// @vitest-environment happy-dom
// multichar-next-speaker-silent：對 GM 一般送出只花一次旁白，點名記下來給「讓 X 接話」鈕（C）；
// 「送出並接話」（B）旁白後被點名的角色接一句就停。正文一落檔，之後的失敗都保留玩家句、彈窗不帶原文。

import { act, createElement, type FormEvent } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, TranscriptEvent } from "../../../shared/contracts/backend-contracts";

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const invokeMock = vi.hoisted(() => vi.fn<Invoke>());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: class {
    onmessage: ((value: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import { useChatController, type ChatController, type TurnFailure } from "./useChatController";
import { callableAt, calledSince, lastStoryId } from "../next-speaker-call";
import { PlayView } from "../PlayView";
import { t } from "../../../i18n";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type Handler = (args: Record<string, unknown>) => unknown;
const formEvent = { preventDefault() {} } as FormEvent<HTMLFormElement>;
const PLAYER = "__PLAYER__";

function gate<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const gmCalls = (events: TranscriptEvent[]) =>
  events.filter((event) => (event.marker as { type?: string } | undefined)?.type === "gm_call");

describe("next speaker call", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    invokeMock.mockReset();
  });

  function mount(overrides: Record<string, Handler> = {}, next: string | null = "fox") {
    let id = 0;
    // 假磁碟：失敗收尾重讀逐字稿時讀得到
    const disk: TranscriptEvent[] = [];
    const land = (event: TranscriptEvent) => (disk.push(event), event);
    const handlers: Record<string, Handler> = {
      append_player_event: (args) => ({ event: land({ ...(args.event as TranscriptEvent), id: `p${++id}` }), offset: 1 }),
      gm_narrate: () => ({ text: "門開了。", raw: "門開了。\n下一位：狐狸", next }),
      chat_with_character: () => ({ text: "嗨", raw: null, aborted: false }),
      append_transcript: (args) => land({ ...(args.event as TranscriptEvent), id: `e${++id}` }),
      // 收回一樓：尾端附屬行（變動摘要、點名列）連同前面那一樓一起收（同後端 floor.rs，略過登場塊）
      pop_transcript_floor: () => {
        let start = disk.length;
        const accessory = (event: TranscriptEvent) =>
          event.marker?.type === "state_update" || event.marker?.type === "gm_call";
        while (start > 0 && accessory(disk[start - 1])) start -= 1;
        if (start > 0 && disk[start - 1].kind !== "system") start -= 1;
        return disk.splice(start);
      },
      read_transcript: () => [...disk],
      discard_unanswered_player: () => false,
      ...overrides,
    };
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    invokeMock.mockImplementation(async (command, args = {}) => {
      calls.push({ command, args });
      const handler = handlers[command];
      return handler ? handler(args) : null;
    });
    const failures: TurnFailure[] = [];
    const refreshState = vi.fn(async () => {});
    const config = { preferences: { max_round_speakers: 3 } } as unknown as AppConfig;
    let chat!: ChatController;
    let place = { worldId: "w1", scene: 0 };
    function Harness({ worldId, scene }: { worldId: string; scene: number }) {
      chat = useChatController({
        worldId,
        scene,
        config,
        speaker: "gm",
        gmTargeted: true,
        metaOf: (cid) => (cid === "fox" ? ({ name: "狐狸" } as never) : undefined),
        playerName: undefined,
        castCount: 1,
        onArrived: () => {},
        refreshState,
        refreshWorlds: async () => {},
        noteChatStarted: () => {},
        markCliConnected: async () => {},
        onError: () => {},
        onTurnFailed: (failure) => failures.push(failure),
      });
      return null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    const render = () => act(() => root!.render(<Harness {...place} />));
    render();
    const h = {
      chat: () => chat,
      handlers,
      failures,
      refreshState,
      count: (command: string) => calls.filter((call) => call.command === command).length,
      texts: () => chat.events.map((event) => event.text || `[${(event.marker as { name: string }).name}]`),
      callable: (cast: string[] = ["fox"]) => callableAt(chat.events, chat.nominated, cast),
      async send(text: string) {
        act(() => chat.setInput(text));
        await act(async () => chat.send(formEvent));
      },
      async relay(text: string) {
        act(() => chat.setInput(text));
        await act(async () => chat.sendRelay());
      },
      moveTo(worldId: string) {
        place = { worldId, scene: 0 };
        render();
      },
    };
    return h;
  }

  it("一般送出對 GM：只一次旁白、不接話、不寫點名列，鈕掛在旁白下", async () => {
    const h = mount();
    await h.send("我推門");
    expect(h.count("gm_narrate")).toBe(1);
    expect(h.count("chat_with_character")).toBe(0);
    expect(gmCalls(h.chat().events)).toHaveLength(0);
    expect(h.chat().nominated).toEqual({ narrationId: "e2", characterId: "fox" });
    expect(h.callable()).toBe(1);
  });

  it("C：寫點名列＋角色接一句、鈕消失；收回對白鈕回來，重試不疊點名列", async () => {
    const h = mount();
    await h.send("我推門");
    await act(async () => h.chat().callNominated());
    expect(h.count("gm_narrate")).toBe(1);
    expect(h.count("chat_with_character")).toBe(1);
    expect(h.texts()).toEqual(["我推門", "門開了。", "[狐狸]", "嗨"]);
    expect(h.callable()).toBeNull();
    await act(async () => h.chat().undoLast());
    expect(h.callable()).toBe(1);
    await act(async () => h.chat().callNominated());
    expect(h.count("chat_with_character")).toBe(2);
    expect(gmCalls(h.chat().events)).toHaveLength(1);
    // 接話失敗後連按重試：點名列仍只有一則
    h.handlers.chat_with_character = () => Promise.reject("boom");
    await act(async () => h.chat().undoLast());
    await act(async () => h.chat().callNominated());
    await act(async () => h.chat().callNominated());
    expect(gmCalls(h.chat().events)).toHaveLength(1);
    expect(h.callable()).toBe(1);
  });

  it("B：旁白＋點名列＋角色一句就停", async () => {
    const h = mount();
    await h.relay("我推門");
    expect(h.count("gm_narrate")).toBe(1);
    expect(h.count("chat_with_character")).toBe(1);
    expect(h.texts()).toEqual(["我推門", "門開了。", "[狐狸]", "嗨"]);
    expect(h.chat().input).toBe("");
  });

  it("點名到玩家：一般送出無鈕無列；B 寫請玩家發言列、不接話", async () => {
    const h = mount({}, PLAYER);
    await h.send("我推門");
    expect(h.chat().nominated).toBeNull();
    expect(gmCalls(h.chat().events)).toHaveLength(0);
    await h.relay("再推");
    expect(gmCalls(h.chat().events)).toEqual([expect.objectContaining({ marker: { type: "gm_call", name: "" } })]);
    expect(h.count("chat_with_character")).toBe(0);
  });

  it("B 空白輸入不進回合", async () => {
    const h = mount();
    await h.relay("   ");
    expect(h.count("append_player_event")).toBe(0);
    expect(h.count("gm_narrate")).toBe(0);
  });

  it("失效：玩家再發言、收回旁白、角色不在主區、換桌", async () => {
    const h = mount();
    await h.send("一");
    expect(h.callable([])).toBeNull();
    await act(async () => h.chat().undoLast());
    expect(h.callable()).toBeNull();
    await h.send("二");
    expect(h.callable()).not.toBeNull();
    h.handlers.gm_narrate = () => ({ text: "沒人回。", raw: null, next: null });
    await h.send("三");
    expect(h.callable()).toBeNull();
    h.handlers.gm_narrate = () => ({ text: "門開了。", raw: null, next: "fox" });
    await h.send("四");
    expect(h.callable()).not.toBeNull();
    h.moveTo("w2");
    expect(h.chat().nominated).toBeNull();
  });

  it("B 旁白後按停止不接話；之後點 C 照常接話", async () => {
    const hold = gate<void>();
    const h = mount({
      gm_narrate: () => ({ text: "門開了。", raw: null, next: "fox", state_updates: [{ path: "hp", value: "3" }] }),
    });
    const base = h.handlers.append_transcript;
    h.handlers.append_transcript = async (args) => {
      if (args.turnPart === "state_update") await hold.promise;
      return base(args);
    };
    act(() => h.chat().setInput("我推門"));
    let running!: Promise<void>;
    await act(async () => {
      running = h.chat().sendRelay();
    });
    await act(async () => h.chat().stopResponse());
    await act(async () => {
      hold.resolve();
      await running;
    });
    expect(h.count("chat_with_character")).toBe(0);
    await act(async () => h.chat().callNominated());
    expect(h.count("chat_with_character")).toBe(1);
  });

  it("正文落檔後各步失敗：玩家句留著、不收回、彈窗不帶原文", async () => {
    const steps: { name: string; arm: (h: ReturnType<typeof mount>) => void; relay: boolean }[] = [
      {
        name: "state_update",
        relay: false,
        arm: (h) => {
          h.handlers.gm_narrate = () => ({ text: "門開了。", raw: null, next: "fox", state_updates: [{ path: "hp", value: "3" }] });
          const base = h.handlers.append_transcript;
          h.handlers.append_transcript = (args) => (args.turnPart === "state_update" ? Promise.reject("disk") : base(args));
        },
      },
      { name: "refreshState", relay: false, arm: (h) => h.refreshState.mockRejectedValueOnce("state") },
      {
        name: "gm_call",
        relay: true,
        arm: (h) => {
          const base = h.handlers.append_transcript;
          h.handlers.append_transcript = (args) =>
            (args.event as TranscriptEvent).marker ? Promise.reject("call") : base(args);
        },
      },
      { name: "reply", relay: true, arm: (h) => (h.handlers.chat_with_character = () => Promise.reject("reply")) },
    ];
    for (const step of steps) {
      const h = mount();
      step.arm(h);
      if (step.relay) await h.relay("我推門");
      else await h.send("我推門");
      expect(h.count("discard_unanswered_player"), step.name).toBe(0);
      expect(h.chat().events[0]?.text, step.name).toBe("我推門");
      expect(h.failures, step.name).toHaveLength(1);
      expect(h.failures[0].draft, step.name).toBeUndefined();
      expect(h.chat().input, step.name).toBe("");
      if (step.name === "reply") expect(h.callable()).toBe(1);
      act(() => root?.unmount());
      host?.remove();
    }
  });

  it("旁白沒落檔就失敗：照舊收回玩家句、原文放回輸入框", async () => {
    const h = mount({ discard_unanswered_player: () => true, gm_narrate: () => Promise.reject("net") });
    await h.relay("我推門");
    expect(h.count("discard_unanswered_player")).toBe(1);
    expect(h.chat().input).toBe("我推門");
  });

  it("GM 推進照舊接力（共用點名接話）", async () => {
    let round = 0;
    const h = mount({
      gm_narrate: () => ({ text: `旁白${++round}`, raw: null, next: round < 2 ? "fox" : PLAYER }),
    });
    await act(async () => h.chat().gmAdvance());
    expect(h.texts()).toEqual(["旁白1", "[狐狸]", "嗨", "旁白2", "[]"]);
    expect(h.chat().nominated).toBeNull();
  });
});

describe("next-speaker-call 純推導", () => {
  const ev = (id: string, kind: TranscriptEvent["kind"], marker?: TranscriptEvent["marker"]): TranscriptEvent => ({
    id,
    ts: id,
    speaker_id: "",
    speaker_name: "",
    kind,
    text: kind === "system" ? "" : id,
    ...(marker ? { marker } : {}),
  });
  it("系統列不擋鈕、點名列只看那則旁白之後", () => {
    const events = [ev("a", "narration"), ev("b", "player"), ev("n", "narration"), ev("s", "system", { type: "state_update" })];
    expect(lastStoryId(events)).toBe("n");
    expect(callableAt(events, { narrationId: "n", characterId: "fox" }, ["fox"])).toBe(2);
    expect(callableAt(events, { narrationId: "a", characterId: "fox" }, ["fox"])).toBeNull();
    const called = [ev("x", "system", { type: "gm_call", name: "狐狸" }), ...events];
    expect(calledSince(called, "n", "狐狸")).toBe(false);
    expect(calledSince([...events, ev("c", "system", { type: "gm_call", name: "狐狸" })], "n", "狐狸")).toBe(true);
  });
});

describe("PlayView 的 B／C 鈕", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });
  const noop = () => {};
  const base = {
    onboarding: null,
    sceneLabel: "第 1 幕",
    storyKey: "w1\u00000",
    events: [
      { id: "p", ts: "1", speaker_id: "", speaker_name: "", kind: "player", text: "我推門" },
      { id: "n", ts: "2", speaker_id: "", speaker_name: "GM", kind: "narration", text: "門開了。" },
      { id: "s", ts: "3", speaker_id: "", speaker_name: "GM", kind: "system", text: "hp：3", marker: { type: "state_update" } },
    ] as TranscriptEvent[],
    metaOf: (id: string) => (id === "fox" ? ({ name: "狐狸" } as never) : undefined),
    generating: null,
    generatingMeta: undefined,
    streamText: "",
    busy: false,
    canRestore: false,
    onRestoreUndone: noop,
    canUndoScene: false,
    onRegenerateSummary: noop,
    onRevertScene: noop,
    speaker: "gm",
    gmTargeted: true,
    gmOnly: false,
    targetName: "GM",
    targetColor: "#000",
    targetImage: null,
    targetEmoji: "🎭",
    onClearTarget: noop,
    input: "你好",
    onInputChange: noop,
    castEmpty: false,
    onSubmit: noop,
    canStop: false,
    onStop: noop,
    requestReplyLabel: "",
    onUndoLast: noop,
    onRequestReply: noop,
    onGmNarrate: noop,
    onGmAdvance: noop,
    relayable: true,
    nominated: { narrationId: "n", characterId: "fox" },
    castIds: ["fox"],
  };
  type Props = Parameters<typeof PlayView>[0];
  function draw(over: Partial<Props> = {}) {
    if (!host) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    act(() => root!.render(createElement(PlayView, { ...(base as unknown as Props), ...over })));
    return host!;
  }
  const relayButton = (node: HTMLElement) => node.querySelector<HTMLButtonElement>("button.composer-relay");
  const callLabel = t("callNominated", { name: "狐狸" });

  it("C 鈕在旁白正下方；生成中不畫", () => {
    const node = draw();
    const call = [...node.querySelectorAll("button")].find((button) => button.textContent === callLabel);
    expect(call).toBeTruthy();
    expect(call!.closest(".next-call")!.previousElementSibling!.classList.contains("message-narration")).toBe(true);
    const busy = draw({ busy: true });
    expect([...busy.querySelectorAll("button")].some((button) => button.textContent === callLabel)).toBe(false);
  });

  it("B 鈕只在對 GM、會點名、非只走 GM、有角色時出現；空白輸入停用；停止時讓位", () => {
    expect(relayButton(draw())).toBeTruthy();
    expect(relayButton(draw())!.disabled).toBe(false);
    expect(relayButton(draw({ input: "  " }))!.disabled).toBe(true);
    expect(relayButton(draw({ relayable: false }))).toBeNull();
    expect(relayButton(draw({ gmTargeted: false, speaker: "fox" }))).toBeNull();
    expect(relayButton(draw({ gmOnly: true }))).toBeNull();
    expect(relayButton(draw({ castEmpty: true }))).toBeNull();
    expect(relayButton(draw({ canStop: true }))).toBeNull();
  });
});
