// @vitest-environment happy-dom
// 載 MVU 的卡在一桌裡的一生：開局照 initvar 初始化（開場白的指令也套上）、每則回覆照 MVU 更新變數並補占位、
// 類巨集送模前代換、存檔帶種子與每則的表、卡片寫入比對版本、接著玩不重新開局。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { runEval } from "@desktop/features/card-interface/mvu/card-mvu-parse-engine";
import { parseWebSave } from "@desktop/shared/contracts/web-save/web-save";
import { playCardFromValue } from "../cards/play-card";
import { failureFromHttp } from "../openrouter/api-failure";
import { MemoryStorage } from "../openrouter/memory-storage";
import { FailoverRuntime } from "../openrouter/failover";
import { runSmartCall, type CallPlan } from "../openrouter/smart-call";
import { streamChat, type ChatMessage, type StreamResult } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { restoreWebSave } from "../saves/web-save-codec";
import { useChat, type ChatController, type GameSetup } from "./useChat";

vi.mock("../openrouter/openrouter-api", () => ({ fetchFreeDaily: async () => null }));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));
// 測試環境沒有 Worker：值解析直接在同一支程式跑（同一份解析引擎）；gate 可以把值解析卡住（開局等待的競態），
// onRun 在每次值解析開始時插一手（模擬卡片在等待期間寫入）
const gate = vi.hoisted(() => ({ wait: null as Promise<void> | null, started: 0, onRun: null as null | (() => void) }));
vi.mock("../mvu/evaluate", () => ({
  createEvaluator: () => ({
    run: async (op: "value" | "patch", text: string) => {
      gate.started += 1;
      gate.onRun?.();
      if (gate.wait) await gate.wait;
      return runEval(op, text);
    },
    dispose: () => {},
  }),
}));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  vi.restoreAllMocks();
  // 失敗的測試也別把卡住的值解析與換掉的全域留給下一條
  vi.unstubAllGlobals();
  gate.wait = null;
  gate.onRun = null;
});

const MVU_SCRIPT = { type: "script", enabled: true, name: "MVU", id: "mvu-1", content: "import 'https://example.invalid/MagVarUpdate/bundle.js';", data: {} };

function mvuCard(initvar: string, extra: Record<string, unknown> = {}) {
  return playCardFromValue("json", {
    spec: "chara_card_v2",
    spec_version: "2.0",
    data: {
      name: "莫拉",
      first_mes: "歡迎，{{user}}。<UpdateVariable>_.set('旅店.燈火', '暗', '亮');//開店</UpdateVariable>",
      extensions: { tavern_helper: { scripts: [MVU_SCRIPT], variables: { 主題: "暗色" } } },
      character_book: {
        name: "旅店書",
        entries: [
          { keys: [], content: initvar, comment: "[initvar]變數初始化勿開", enabled: false, insertion_order: 1 },
          { keys: [], content: "<now>\n{{format_message_variable::stat_data}}\n</now>", comment: "變數列表", constant: true, enabled: true, insertion_order: 2 },
        ],
      },
      ...extra,
    },
  });
}

const INITVAR = "旅店:\n  $meta:\n    extensible: false\n  燈火: 暗\n  主人: '{{user}}'\n金幣: 100\n";

const session = {
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
let replies: string[] = [];

async function mount(game: GameSetup, options: { runtime?: FailoverRuntime; pool?: OpenRouterSession["pool"]; waitInit?: boolean } = {}) {
  vi.mocked(streamChat).mockReset();
  vi.mocked(streamChat).mockImplementation(async ({ messages }) => {
    sent.push(structuredClone(messages));
    return { kind: "ok", text: replies.shift() ?? "沒有安排的回覆", model: null, truncated: null };
  });
  const custom =
    options.runtime || options.pool
      ? ({ ...session, runtime: options.runtime ?? session.runtime, pool: options.pool ?? session.pool } as OpenRouterSession)
      : session;
  function Probe() {
    chat = useChat(game, custom, null);
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  // 等執行期載好、開局做完（開局掛上表，或回報初始值讀不懂）
  if (options.waitInit !== false) await waitFor(() => chat.entries.some((entry) => entry.vars !== undefined) || chat.mvu.initError !== null);
  return () => act(async () => root.unmount());
}

async function waitFor(done: () => boolean) {
  for (let tries = 0; tries < 200 && !done(); tries++) await act(async () => new Promise((resolve) => setTimeout(resolve, 10)));
  if (!done()) throw new Error("等不到開局");
}

async function say(text: string) {
  await act(async () => chat.setInput(text));
  await act(async () => chat.send());
}

const statOf = (index: number) => (chat.entries[index].vars as { stat_data: Record<string, unknown> }).stat_data;
const targetRev = (floor: number) => chat.mvu.frontend?.targets[floor]?.rev ?? null;
const layerRev = (key: string) => chat.mvu.frontend?.layers[key]?.rev ?? null;
const lastPrompt = () => sent[sent.length - 1].map((message) => message.content);

describe("an MVU card on a table", () => {
  it("opens with the initvar table (macros, metadata cleaned) and the opening's own commands applied", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    const opening = chat.entries[0];
    expect(opening.vars).toMatchObject({
      stat_data: { 旅店: { 燈火: "亮", 主人: "旅人" }, 金幣: 100 },
      initialized_lorebooks: { 旅店書: [] },
      schema: { type: "object" },
    });
    expect(JSON.stringify(opening.vars)).not.toContain("$meta");
    // 開場白不補占位
    expect(opening.text).not.toContain("<StatusPlaceHolderImpl/>");
    await unmount();
  });

  it("each reply updates its own table from the last one, gets the placeholder, and the next prompt reads it", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    replies = ["你買了一杯酒。<UpdateVariable>\n_.set('金幣', 100, 80);//買酒\n</UpdateVariable>", "夜已經深了。"];
    await say("來杯酒");
    // 類巨集照送模當下最後一則帶表的訊息（開場）代換成 YAML
    expect(lastPrompt()).toContain("<now>\n旅店:\n  燈火: 亮\n  主人: 旅人\n金幣: 100\n</now>");
    expect(statOf(2)).toEqual({ 旅店: { 燈火: "亮", 主人: "旅人" }, 金幣: 80 });
    expect((chat.entries[2].vars as { display_data: Record<string, unknown> }).display_data.金幣).toBe("100->80 (買酒)");
    expect(chat.entries[2].text.endsWith("\n\n<StatusPlaceHolderImpl/>")).toBe(true);
    expect(chat.entries[2].raw).toBe("你買了一杯酒。<UpdateVariable>\n_.set('金幣', 100, 80);//買酒\n</UpdateVariable>");
    await say("再坐一下");
    const second = lastPrompt();
    expect(second).toContain("<now>\n旅店:\n  燈火: 亮\n  主人: 旅人\n金幣: 80\n</now>");
    // MVU filterPrompts：歷史裡的占位不送
    expect(second.some((content) => content.includes("<StatusPlaceHolderImpl/>"))).toBe(false);
    // 沒有指令的回覆也照樣接上一張表
    expect(statOf(4)).toEqual(statOf(2));
    await unmount();
  });

  it("the save carries the seed, macros, every table and the TavernHelper layers, and resuming does not re-initialize", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    replies = ["你買了一杯酒。<UpdateVariable>_.set('金幣', 100, 80);</UpdateVariable>"];
    await say("來杯酒");
    const save = chat.exportSave();
    const parsed = parseWebSave(JSON.stringify(save));
    expect(parsed.ok).toBe(true);
    expect(save.mvu?.macros).toEqual({ user: "旅人", char: "莫拉" });
    expect(save.mvu?.seed).toEqual(save.messages[0].message_vars);
    expect(save.mvu?.layers.character).toEqual({ 主題: "暗色" });
    expect((save.messages[2].message_vars as { stat_data: { 金幣: number } }).stat_data.金幣).toBe(80);
    await unmount();

    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    const unmountResumed = await mount({ ...game, resume: { entries: game.entries, local: game.local, carry: game.carry } });
    expect(chat.entries.map((entry) => entry.vars)).toEqual(save.messages.map((message) => message.message_vars));
    replies = ["再一杯。<UpdateVariable>_.set('金幣', 80, 60);</UpdateVariable>"];
    await say("再來");
    expect(statOf(4).金幣).toBe(60);
    expect(chat.exportSave().mvu?.seed).toEqual(save.mvu?.seed);
    await unmountResumed();
  });

  it("regenerating processes the new reply from the table before it", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    replies = ["<UpdateVariable>_.set('金幣', 100, 1);</UpdateVariable>花光了", "<UpdateVariable>_.set('金幣', 100, 99);</UpdateVariable>省著花"];
    await say("買東西");
    expect(statOf(2).金幣).toBe(1);
    await act(async () => chat.regenerate());
    expect(statOf(2).金幣).toBe(99);
    await unmount();
  });

  it("card writes land only on the version they were based on; a stale write gets the current table back", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    const opening = chat.entries[0];
    const replyTo = vi.fn();
    const table = { ...(opening.vars as object), stat_data: { 金幣: 5 } };
    const openingRev = targetRev(0);
    await act(async () =>
      chat.mvu.write({ requestId: "w1", target: opening.id, payload: JSON.stringify(table), base: openingRev, generation: 0, scene: 0 }, replyTo),
    );
    const newRev = targetRev(0);
    expect(newRev).not.toBe(openingRev);
    expect(replyTo).toHaveBeenLastCalledWith({ kind: "mvu-settle", results: [{ requestId: "w1", ok: true, rev: newRev }] });
    expect(statOf(0)).toEqual({ 金幣: 5 });
    await act(async () =>
      chat.mvu.write({ requestId: "w2", target: opening.id, payload: "{}", base: openingRev, generation: 0, scene: 0 }, replyTo),
    );
    expect(replyTo).toHaveBeenLastCalledWith({
      kind: "mvu-settle",
      results: [{ requestId: "w2", ok: false, error: "stale" }],
      authority: { key: opening.id, table, rev: newRev },
    });
    // chat 層就是 ST 的聊天變數：{{getvar}} 讀得到
    await act(async () =>
      chat.mvu.write({ requestId: "w3", target: "chat", payload: '{"心情":"好"}', base: layerRev("chat"), generation: 0, scene: 0 }, replyTo),
    );
    expect(chat.setup.variables.local.values).toEqual({ 心情: "好" });
    // 別張卡的角色層、壞形狀
    await act(async () =>
      chat.mvu.write({ requestId: "w4", target: "character:other", payload: "{}", base: null, generation: 0, scene: 0 }, replyTo),
    );
    expect(replyTo).toHaveBeenLastCalledWith(expect.objectContaining({ results: [{ requestId: "w4", ok: false, error: "bad-target" }] }));
    replyTo.mockClear();
    await act(async () => chat.mvu.write({ requestId: "w5", target: "chat", payload: 3 }, replyTo));
    expect(replyTo).not.toHaveBeenCalled();
    await unmount();
  });

  it("an initvar entry that cannot be parsed leaves the table without card variables and says which entry", async () => {
    const unmount = await mount({ card: mvuCard("金幣: [1, *沒有這個錨點"), userName: "旅人", openingIndex: 0 });
    expect(chat.mvu.initError).toBe("[initvar]變數初始化勿開");
    expect(chat.entries[0].vars).toBeUndefined();
    replies = ["<UpdateVariable>_.set('金幣', 1, 2);</UpdateVariable>回覆"];
    await say("嗨");
    expect(chat.entries[2].vars).toBeUndefined();
    expect(chat.entries[2].text).not.toContain("<StatusPlaceHolderImpl/>");
    await unmount();
  });
});

describe("an MVU table under races and edits", () => {
  const ok = (text: string): StreamResult => ({ kind: "ok", text, model: null, truncated: null });

  it("a card write made while the request is out is the base the reply's commands apply to", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    let finish!: (result: StreamResult) => void;
    vi.mocked(streamChat).mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    await act(async () => chat.setInput("買酒"));
    let running!: Promise<void>;
    await act(async () => {
      running = chat.send();
    });
    await waitFor(() => finish !== undefined);
    const opening = chat.entries[0];
    const written = { ...(opening.vars as object), stat_data: { ...statOf(0), 金幣: 5 } };
    await act(async () =>
      chat.mvu.write({ requestId: "w", target: opening.id, payload: JSON.stringify(written), base: targetRev(0), generation: 0, scene: 0 }, () => {}),
    );
    await act(async () => {
      finish(ok("<UpdateVariable>\n_.add('金幣', -20);\n</UpdateVariable>買了酒"));
      await running;
    });
    expect(statOf(0).金幣).toBe(5);
    expect(statOf(2).金幣).toBe(-15);
    await unmount();
  });

  it("a failover's second shot keeps a card write made during the first shot", async () => {
    const runtime = new FailoverRuntime();
    const busy: StreamResult = { kind: "failed", failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503}}'), emittedText: false } };
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, { send: async () => busy, dailyRemaining: async () => 10, signal: new AbortController().signal, now: () => 0 });
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 }, { runtime });
    const opening = chat.entries[0];
    const written = { ...(opening.vars as object), stat_data: { ...statOf(0), 金幣: 7 } };
    const base = targetRev(0);
    vi.mocked(streamChat).mockImplementationOnce(async () => {
      chat.mvu.write({ requestId: "w", target: opening.id, payload: JSON.stringify(written), base, generation: 0, scene: 0 }, () => {});
      return busy;
    });
    replies = ["<UpdateVariable>\n_.add('金幣', 1);\n</UpdateVariable>第二發回覆"];
    await say("買東西");
    expect(statOf(0).金幣).toBe(7);
    expect(statOf(2).金幣).toBe(8);
    await unmount();
  });

  it("a card write made while the reply's values are being parsed makes the reply recompute from the new table", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    let release!: () => void;
    gate.started = 0;
    gate.wait = new Promise((resolve) => (release = resolve));
    replies = ["<UpdateVariable>\n_.add('金幣', -20);\n</UpdateVariable>買了酒"];
    await act(async () => chat.setInput("買酒"));
    let running!: Promise<void>;
    await act(async () => {
      running = chat.send();
    });
    // 回覆已回來、MVU 正在等值解析：這時卡片把前一張表的金幣改成 5
    await waitFor(() => gate.started > 0);
    const opening = chat.entries[0];
    const written = { ...(opening.vars as object), stat_data: { ...statOf(0), 金幣: 5 } };
    await act(async () =>
      chat.mvu.write({ requestId: "w", target: opening.id, payload: JSON.stringify(written), base: targetRev(0), generation: 0, scene: 0 }, () => {}),
    );
    gate.wait = null;
    await act(async () => {
      release();
      await running;
    });
    expect(statOf(0).金幣).toBe(5);
    expect(statOf(2).金幣).toBe(-15);
    await unmount();
  });

  it("a failover's second shot keeps chat and global writes made during the first shot, and still commits its own setvar", async () => {
    const runtime = new FailoverRuntime();
    const busy: StreamResult = { kind: "failed", failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503}}'), emittedText: false } };
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, { send: async () => busy, dailyRemaining: async () => 10, signal: new AbortController().signal, now: () => 0 });
    const unmount = await mount(
      { card: mvuCard(INITVAR, { description: "{{setvar::flag::fresh}}" }), userName: "旅人", openingIndex: 0 },
      { runtime },
    );
    vi.mocked(streamChat).mockImplementationOnce(async () => {
      // 第一發已提交 setvar，卡片手上的版本過期：照沙盒拿回的現行版本再寫一次
      const write = (target: string, payload: object, base = layerRev(target)) =>
        chat.mvu.write({ requestId: target, target, payload: JSON.stringify(payload), base, generation: 0, scene: 0 }, (message) => {
          const authority = message.authority as { rev: string | null } | undefined;
          if (authority) write(target, payload, authority.rev);
        });
      write("chat", { ...chat.setup.variables.local.values, written: 7 });
      write("global", { ...chat.setup.variables.global.values, shared: "卡片寫的" });
      return busy;
    });
    replies = ["第二發回覆已經完成。"];
    await say("買東西");
    expect(streamChat).toHaveBeenCalledTimes(2);
    expect(chat.setup.variables.local.values).toMatchObject({ written: 7, flag: "fresh" });
    expect(chat.setup.variables.global.values.shared).toBe("卡片寫的");
    delete chat.setup.variables.global.values.shared;
    await unmount();
  });

  it("chat and global writes made before the first shot is committed still survive a failover's second shot", async () => {
    const runtime = new FailoverRuntime();
    const busy: StreamResult = { kind: "failed", failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503}}'), emittedText: false } };
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, { send: async () => busy, dailyRemaining: async () => 10, signal: new AbortController().signal, now: () => 0 });
    // 回合已開始（變數副本已取）、還在更新模型清單時，卡片寫入 chat 與 global
    const refreshing = {
      ...session.pool,
      refresh: async () => {
        const write = (target: string, payload: object) =>
          chat.mvu.write({ requestId: target, target, payload: JSON.stringify(payload), base: layerRev(target), generation: 0, scene: 0 }, () => {});
        write("chat", { ...chat.setup.variables.local.values, written: 7 });
        write("global", { ...chat.setup.variables.global.values, early: "卡片先寫的" });
      },
    } as unknown as OpenRouterSession["pool"];
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 }, { runtime, pool: refreshing });
    let firstShot: unknown[] = [];
    vi.mocked(streamChat).mockImplementationOnce(async () => {
      firstShot = [chat.setup.variables.local.values.written, chat.setup.variables.global.values.early];
      return busy;
    });
    replies = ["第二發回覆已經完成。"];
    await say("買東西");
    expect(streamChat).toHaveBeenCalledTimes(2);
    expect(firstShot).toEqual([7, "卡片先寫的"]);
    expect([chat.setup.variables.local.values.written, chat.setup.variables.global.values.early]).toEqual([7, "卡片先寫的"]);
    delete chat.setup.variables.global.values.early;
    await unmount();
  });

  it("leaving the page while the reply's values are being parsed keeps the draft for the reload", async () => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    let release!: () => void;
    gate.started = 0;
    gate.wait = new Promise((resolve) => (release = resolve));
    replies = ["<UpdateVariable>\n_.add('金幣', -20);\n</UpdateVariable>買了酒"];
    await act(async () => chat.setInput("買酒"));
    let running!: Promise<void>;
    await act(async () => {
      running = chat.send();
    });
    await waitFor(() => gate.started > 0);
    const draftKey = () => Array.from({ length: sessionStorage.length }, (_, index) => sessionStorage.key(index)).find((key) => key?.startsWith("tt-web:pending-input:"));
    expect(draftKey()).toBeDefined();
    await act(async () => {
      window.dispatchEvent(new Event("beforeunload"));
    });
    gate.wait = null;
    await act(async () => {
      release();
      await running;
    });
    expect(sessionStorage.getItem(draftKey()!)).toBe("買酒");
    // 作廢的回合不落地
    expect(chat.entries.some((entry) => entry.text.includes("買了酒"))).toBe(false);
    await unmount();
  });

  it("a reply whose value parsing keeps getting interrupted lands no stale table, and the next reply builds on the latest one", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    let writes = 0;
    // 每次值解析一開始，卡片就把開場的金幣改掉（版本過期就拿現行版本再寫）
    gate.onRun = () => {
      writes += 1;
      const opening = chat.entries[0];
      const payload = JSON.stringify({ ...(opening.vars as object), stat_data: { ...statOf(0), 金幣: 4 + writes } });
      const write = (base = targetRev(0)) =>
        chat.mvu.write({ requestId: `r${writes}`, target: opening.id, payload, base, generation: 0, scene: 0 }, (message) => {
          const authority = message.authority as { rev: string | null } | undefined;
          if (authority) write(authority.rev);
        });
      write();
    };
    replies = ["<UpdateVariable>\n_.add('金幣', -20);\n</UpdateVariable>買了酒"];
    await say("買酒");
    gate.onRun = null;
    expect(writes).toBe(3);
    expect(statOf(0).金幣).toBe(7);
    expect(chat.entries[2].vars).toBeUndefined();
    expect(chat.entries[2].text).not.toContain("<StatusPlaceHolderImpl/>");
    expect(parseWebSave(JSON.stringify(chat.exportSave())).ok).toBe(true);
    replies = ["<UpdateVariable>\n_.add('金幣', -1);\n</UpdateVariable>下一輪"];
    await say("再來");
    expect(statOf(4).金幣).toBe(6);
    await unmount();
  });

  it("when the card and this turn's setvar touch the same chat and global key, the card's value wins on both shots", async () => {
    const runtime = new FailoverRuntime();
    const busy: StreamResult = { kind: "failed", failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503}}'), emittedText: false } };
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, { send: async () => busy, dailyRemaining: async () => 10, signal: new AbortController().signal, now: () => 0 });
    const refreshing = {
      ...session.pool,
      refresh: async () => {
        const write = (target: string, payload: object) =>
          chat.mvu.write({ requestId: target, target, payload: JSON.stringify(payload), base: layerRev(target), generation: 0, scene: 0 }, () => {});
        write("chat", { flag: "card" });
        write("global", { ...chat.setup.variables.global.values, shared: "card" });
      },
    } as unknown as OpenRouterSession["pool"];
    const card = mvuCard(INITVAR, { description: "{{setvar::flag::model}}{{setglobalvar::shared::model}}" });
    const unmount = await mount({ card, userName: "旅人", openingIndex: 0 }, { runtime, pool: refreshing });
    let firstShot: unknown[] = [];
    vi.mocked(streamChat).mockImplementationOnce(async () => {
      firstShot = [chat.setup.variables.local.values.flag, chat.setup.variables.global.values.shared];
      return busy;
    });
    replies = ["第二發回覆已經完成。"];
    await say("買東西");
    expect(streamChat).toHaveBeenCalledTimes(2);
    expect(firstShot).toEqual(["card", "card"]);
    expect([chat.setup.variables.local.values.flag, chat.setup.variables.global.values.shared]).toEqual(["card", "card"]);
    delete chat.setup.variables.global.values.shared;
    await unmount();
  });

  it("TavernHelper macros in the prompt read this shot's variable copy (a setvar earlier in the same prompt counts)", async () => {
    const unmount = await mount({
      card: mvuCard(INITVAR, { description: "FLAG={{setvar::flag::fresh}}{{get_chat_variable::flag}}" }),
      userName: "旅人",
      openingIndex: 0,
    });
    replies = ["好的，記下來了。"];
    await say("嗨");
    expect(lastPrompt().some((content) => content.includes("FLAG=fresh"))).toBe(true);
    expect(chat.setup.variables.local.values.flag).toBe("fresh");
    await unmount();
  });

  it("on screen the macros read the latest table, like TavernHelper's render pass (it passes no message id)", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    replies = ["<UpdateVariable>\n_.set('金幣', 100, 80);\n</UpdateVariable>買了酒"];
    await say("來杯酒");
    expect(statOf(0).金幣).toBe(100);
    expect(chat.mvu.display("{{get_message_variable::stat_data.金幣}}", chat.entries)).toBe("80");
    await unmount();
  });

  it("editing the last reply keeps its table; deleting it makes the table before it the base again", async () => {
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 });
    replies = ["<UpdateVariable>\n_.set('金幣', 100, 80);\n</UpdateVariable>買了酒"];
    await say("來杯酒");
    await act(async () => chat.editLast("<UpdateVariable>\n_.set('金幣', 100, 90);\n</UpdateVariable>改了價錢"));
    // MVU 不重算編輯過的那則：表照舊
    expect(statOf(2).金幣).toBe(80);
    await act(async () => chat.deleteLast());
    await act(async () => chat.deleteLast());
    replies = ["<UpdateVariable>\n_.add('金幣', -1);\n</UpdateVariable>重新來過"];
    await say("再來一次");
    expect(statOf(2).金幣).toBe(99);
    await unmount();
  });

  it("a send while the opening is still initializing waits for it; stop during the wait sends nothing", async () => {
    let release!: () => void;
    gate.wait = new Promise((resolve) => (release = resolve));
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 }, { waitInit: false });
    const before = sent.length;
    await act(async () => chat.setInput("等等我"));
    let running!: Promise<void>;
    await act(async () => {
      running = chat.send();
    });
    await act(async () => chat.stop());
    gate.wait = null;
    await act(async () => {
      release();
      await running;
    });
    expect(sent.length).toBe(before);
    expect(chat.input).toBe("等等我");
    expect(chat.entries).toHaveLength(1);
    // 開局照樣做完；之後送得出去，類巨集讀得到第 0 則的表
    await waitFor(() => chat.entries[0].vars !== undefined);
    replies = ["好的，我在等你。"];
    await say("好了");
    expect(lastPrompt().some((content) => content.includes("金幣: 100"))).toBe(true);
    await unmount();
  });

  it("unmounting while the opening is still initializing lands nothing and throws nothing", async () => {
    let release!: () => void;
    gate.wait = new Promise((resolve) => (release = resolve));
    const unmount = await mount({ card: mvuCard(INITVAR), userName: "旅人", openingIndex: 0 }, { waitInit: false });
    await act(async () => chat.setInput("等等我"));
    let running!: Promise<void>;
    await act(async () => {
      running = chat.send();
    });
    await unmount();
    gate.wait = null;
    release();
    await running;
    expect(streamChat).not.toHaveBeenCalled();
  });

  it("an initialization failure saves no seed and no tables, and the save still passes the contract", async () => {
    const unmount = await mount({ card: mvuCard("金幣: [1, *沒有這個錨點"), userName: "旅人", openingIndex: 0 });
    expect(chat.mvu.initError).not.toBeNull();
    const save = chat.exportSave();
    expect(save.mvu?.seed).toBeNull();
    expect(save.messages.every((message) => message.message_vars === undefined)).toBe(true);
    expect(parseWebSave(JSON.stringify(save)).ok).toBe(true);
    await unmount();
  });
});
