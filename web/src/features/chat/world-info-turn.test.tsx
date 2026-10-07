// @vitest-environment happy-dom
// 世界書在一桌裡的生命週期：真正派送的那一發才落地觸發狀態、存檔帶走、接著玩讀回，對話退回去時照 ST 回退；
// 換模第二發從回合開頭的狀態掃；outlet 留給下一輪的卡欄位（不進存檔）；畫面已走掉的那一發不落地。
import { IDBFactory } from "fake-indexeddb";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { parseWebSave } from "@desktop/shared/contracts/web-save/web-save";
import { playCardFromValue } from "../cards/play-card";
import { failureFromHttp } from "../openrouter/api-failure";
import { FailoverRuntime } from "../openrouter/failover";
import { MemoryStorage } from "../openrouter/memory-storage";
import { runSmartCall, type CallPlan } from "../openrouter/smart-call";
import { streamChat, type ChatMessage, type StreamResult } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { openSaveStore, type SaveStore } from "../saves/save-store";
import { restoreWebSave } from "../saves/web-save-codec";
import { useChat, type ChatController, type GameSetup } from "./useChat";

vi.mock("../openrouter/openrouter-api", () => ({ fetchFreeDaily: async () => null }));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const CARD = playCardFromValue("json", {
  spec: "chara_card_v2",
  spec_version: "2.0",
  data: {
    name: "瑟拉",
    first_mes: "爐火很旺。",
    character_book: {
      entries: [
        { keys: ["雪"], content: "雪會連下三天。", enabled: true, insertion_order: 10, position: "before_char", extensions: { sticky: 3 } },
        { keys: [], content: "常駐", enabled: true, constant: true, insertion_order: 1, position: "after_char" },
      ],
    },
  },
});

const baseSession = {
  apiKey: "sk-or-v1-test",
  quota: { kind: "unknown" },
  deps: { fetch, apiBase: "http://fake/api/v1" },
  runtime: new FailoverRuntime(),
  pool: {
    refresh: async () => {},
    plan: (holds: (model: string) => boolean): CallPlan => ({ account: "acct", lineup: ["A", "B"], others: [], fits: holds, names: new Map() }),
    contextLength: () => 65_536,
    tokenizer: () => null,
  },
  quotaEvent: () => {},
  refreshQuota: async () => {},
} as unknown as OpenRouterSession;

let chat!: ChatController;
const sent: ChatMessage[][] = [];

async function mount(game: GameSetup, options: { saves?: SaveStore; runtime?: FailoverRuntime } = {}) {
  const session = { ...baseSession, runtime: options.runtime ?? new FailoverRuntime() } as OpenRouterSession;
  vi.mocked(streamChat).mockReset();
  vi.mocked(streamChat).mockImplementation(async ({ messages }) => {
    sent.push(structuredClone(messages));
    return { kind: "ok", text: "回覆", model: null, truncated: null };
  });
  function Probe() {
    chat = useChat(game, session, options.saves ?? null);
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  return () => act(async () => root.unmount());
}

async function say(text: string) {
  await act(async () => chat.setInput(text));
  await act(async () => chat.send());
}

const lastPrompt = () => sent[sent.length - 1].map((message) => message.content);

describe("world info across a table's life", () => {
  it("sticky lands with the dispatched shot, goes into the save, comes back on resume, and rolls back with the chat", async () => {
    const unmount = await mount({ card: CARD, userName: "旅人", openingIndex: 0 });
    // 開桌就配好穩定 ID，還沒掃過
    expect(chat.exportSave().world_info).toEqual({
      entries: [
        { id: "wi-0", key: "0" },
        { id: "wi-1", key: "1" },
      ],
      timed: { sticky: {}, cooldown: {} },
      last_message_id: null,
      message_effects: {},
    });

    await say("外面下雪了");
    expect(lastPrompt()).toContain("雪會連下三天。");
    expect(lastPrompt()).toContain("常駐");
    const userId = chat.entries[1].id;
    const save = chat.exportSave();
    expect(save.world_info.timed).toEqual({ sticky: { "wi-0": { start: 2, end: 5, protected: false } }, cooldown: {} });
    expect(save.world_info.last_message_id).toBe(userId);
    expect(parseWebSave(JSON.stringify(save)).ok).toBe(true);
    await unmount();

    // 接著玩：沒提到雪，sticky 照樣把條目帶進來（對話 4 則 < end 5）
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    const unmountResumed = await mount({
      card: game.card,
      userName: game.userName,
      openingIndex: game.openingIndex,
      resume: { entries: game.entries, local: game.local, carry: game.carry },
    });
    await say("我喝湯");
    expect(lastPrompt()).toContain("雪會連下三天。");

    // 刪回只剩開場白與第一句之前：對話退到 ≤ start，未受保護的 sticky 撤掉，沒提到雪就不再觸發
    await act(async () => chat.deleteLast());
    await act(async () => chat.deleteLast());
    await act(async () => chat.deleteLast());
    await act(async () => chat.deleteLast());
    expect(chat.entries).toHaveLength(1);
    await say("我坐下");
    expect(lastPrompt()).not.toContain("雪會連下三天。");
    expect(chat.exportSave().world_info.timed).toEqual({ sticky: {}, cooldown: {} });
    await unmountResumed();
  });

  it("a cancelled turn keeps what landed at dispatch, and the next scan rolls it back", async () => {
    const unmount = await mount({ card: CARD, userName: "旅人", openingIndex: 0 });
    // 真的走停止：串流一個字都沒出來就按停止（取消未完成回合）
    let started = () => {};
    const streaming = new Promise<void>((resolve) => (started = resolve));
    vi.mocked(streamChat).mockImplementationOnce(
      ({ messages, signal }) =>
        new Promise<StreamResult>((resolve) => {
          sent.push(structuredClone(messages));
          started();
          signal.addEventListener("abort", () => resolve({ kind: "aborted", text: "" }));
        }),
    );
    await act(async () => chat.setInput("雪"));
    let turn: Promise<void> = Promise.resolve();
    await act(async () => {
      turn = chat.send();
      await streaming;
    });
    await act(async () => {
      chat.stop();
      await turn;
    });
    // 玩家句收回、原句回輸入框；派送那一發的狀態送出前就落地（ST 在送出前寫進 chat_metadata）
    expect(chat.entries).toHaveLength(1);
    expect(chat.input).toBe("雪");
    expect(chat.exportSave().world_info.timed.sticky?.["wi-0"]).toEqual({ start: 2, end: 5, protected: false });
    // 對話退回 1 則：下一次組提示時 start 2 的 sticky 被撤掉
    await say("坐下");
    expect(lastPrompt()).not.toContain("雪會連下三天。");
    expect(chat.exportSave().world_info.timed.sticky).toEqual({});
    await unmount();
  });

  it("the failover shot scans from the turn's starting state, not from what the first shot landed", async () => {
    const card = playCardFromValue("json", {
      name: "瑟拉",
      description: "門外：{{outlet::door}}",
      first_mes: "爐火很旺。",
      character_book: {
        entries: [
          { keys: ["雪"], content: "腳印", enabled: true, insertion_order: 1, extensions: { position: 7, outlet_name: "door", probability: 50, sticky: 2 } },
        ],
      },
    });
    // 這一句的失敗讓換模成立：先讓 A 失敗過一次
    const runtime = new FailoverRuntime();
    const busy: StreamResult = { kind: "failed", failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503}}'), emittedText: false } };
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, { send: async () => busy, dailyRemaining: async () => 10, signal: new AbortController().signal, now: () => 0 });
    const unmount = await mount({ card, userName: "旅人", openingIndex: 0 }, { runtime });
    vi.mocked(streamChat).mockImplementationOnce(async ({ messages }) => {
      sent.push(structuredClone(messages));
      return busy;
    });
    // A 那一發擲過（outlet 有腳印、sticky 落地），B 那一發擲輸
    vi.spyOn(Math, "random").mockReturnValueOnce(0.1).mockReturnValue(0.9);
    await say("下雪了");
    const [first, second] = sent.slice(-2).map((messages) => messages.map((message) => message.content));
    expect(first).toContain("門外：");
    // 第二發的卡欄位看到的是回合開頭的 outlet（空的），不是第一發掃出的「腳印」；也沒有接第一發的 sticky
    expect(second).toContain("門外：");
    expect(chat.exportSave().world_info.timed.sticky).toEqual({});
    await unmount();
  });

  it("the outlet carries to the next turn's card fields in the same tab, but not through a save", async () => {
    const card = playCardFromValue("json", {
      name: "瑟拉",
      description: "Door={{outlet::door}}",
      first_mes: "爐火很旺。",
      character_book: { entries: [{ keys: [], content: "FOOTPRINTS", constant: true, enabled: true, insertion_order: 1, extensions: { position: 7, outlet_name: "door" } }] },
    });
    const unmount = await mount({ card, userName: "旅人", openingIndex: 0 });
    await say("一");
    expect(lastPrompt()).toContain("Door=");
    await say("二");
    expect(lastPrompt()).toContain("Door=FOOTPRINTS");
    const save = chat.exportSave();
    await unmount();
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    const unmountResumed = await mount({ ...game, resume: { entries: game.entries, local: game.local, carry: game.carry } });
    await say("三");
    expect(lastPrompt()).toContain("Door=");
    expect(lastPrompt()).not.toContain("Door=FOOTPRINTS");
    await unmountResumed();
  });

  it("the player's {{outlet}} reads the outlet the previous turn scanned", async () => {
    const card = playCardFromValue("json", {
      name: "瑟拉",
      first_mes: "爐火很旺。",
      character_book: { entries: [{ keys: [], content: "FOOTPRINTS", constant: true, enabled: true, insertion_order: 1, extensions: { position: 7, outlet_name: "door" } }] },
    });
    const unmount = await mount({ card, userName: "旅人", openingIndex: 0 });
    await say("Door={{outlet::door}}");
    expect(chat.entries[1].text).toBe("Door=");
    await say("Door={{outlet::door}}");
    expect(chat.entries[3].text).toBe("Door=FOOTPRINTS");
    expect(lastPrompt()).toContain("Door=FOOTPRINTS");
    await unmount();
  });

  it("a shot still streaming when the page goes away (unmount, reload) never lands its trigger state", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    const factory = new IDBFactory();
    const saves = openSaveStore(factory)!;
    await saves.loadGlobal({});
    const unmount = await mount({ card: CARD, userName: "旅人", openingIndex: 0 }, { saves });
    let release: (result: StreamResult) => void = () => {};
    let started = () => {};
    const streaming = new Promise<void>((resolve) => (started = resolve));
    vi.mocked(streamChat).mockImplementationOnce(
      ({ messages }) =>
        new Promise<StreamResult>((resolve) => {
          sent.push(structuredClone(messages));
          release = resolve;
          started();
        }),
    );
    await act(async () => chat.setInput("外面下雪了"));
    await act(async () => {
      void chat.send();
      await streaming;
    });
    // 回合中：存檔庫裡只有開口前那一格，沒有這一發的 sticky
    const [meta] = await saves.list();
    expect((await saves.get(meta.id))?.world_info.timed).toEqual({ sticky: {}, cooldown: {} });
    // 畫面走掉（換卡、重新整理），回應晚到：不落進逐字稿，觸發狀態也不寫進存檔
    await unmount();
    await act(async () => release({ kind: "ok", text: "晚到的回覆", model: null, truncated: null }));
    await act(async () => new Promise((resolve) => setTimeout(resolve, 20)));
    const stored = (await openSaveStore(factory)!.get(meta.id))!;
    expect(stored.world_info.timed).toEqual({ sticky: {}, cooldown: {} });
    expect(stored.messages.map((message) => message.text)).toEqual(["爐火很旺。"]);
    // 重新整理後接著玩（D28）：退回上次完整回合、那一句放回輸入框、觸發狀態從存檔的樣子起算
    const restored = restoreWebSave(stored);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    const unmountResumed = await mount({ ...game, saveId: meta.id, resume: { entries: game.entries, local: game.local, carry: game.carry } }, { saves });
    expect(chat.input).toBe("外面下雪了");
    expect(chat.exportSave().world_info.timed).toEqual({ sticky: {}, cooldown: {} });
    await unmountResumed();
  });
});
