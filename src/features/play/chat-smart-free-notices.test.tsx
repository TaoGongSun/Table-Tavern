// @vitest-environment happy-dom
// 穩定免費換模提示（stable-free-failover §2.5）：聊天室只接受「本次檢視期間自己送出的 turn」的事件，
// eventId 去重，換幕／離桌／卸載清空；提示不進逐字稿；重送成功不收回玩家句，最終失敗才收回。

import { act, type FormEvent } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { AppConfig, TranscriptEvent } from "../../shared/contracts/backend-contracts";

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type Listener = (event: { payload: unknown }) => void;
const invokeMock = vi.hoisted(() => vi.fn<Invoke>());
const bus = vi.hoisted(() => ({ listeners: new Map<string, Set<(event: { payload: unknown }) => void>>() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: class {
    onmessage: ((value: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: Listener) => {
    const set = bus.listeners.get(name) ?? new Set();
    set.add(handler);
    bus.listeners.set(name, set);
    return () => set.delete(handler);
  }),
}));

import { PlayView } from "./PlayView";
import { useChatController, type ChatController, type TurnFailure } from "./useChatController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type Handler = (args: Record<string, unknown>) => unknown;
const formEvent = { preventDefault() {} } as FormEvent<HTMLFormElement>;
const emit = (name: string, payload: unknown) =>
  act(() => bus.listeners.get(name)?.forEach((handler) => handler({ payload })));

const failover = (eventId: string, turnId: string | null, world: string | null = "w1") => ({
  eventId,
  world,
  turnId,
  from: "甲",
  to: "乙",
  retried: true,
});
const FAILOVER_TEXT = () => t("smartFreeFailover", { from: "甲", to: "乙" });

describe("chat smart-free notices", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    invokeMock.mockReset();
    bus.listeners.clear();
  });

  function mount(handlers: Record<string, Handler>) {
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    invokeMock.mockImplementation(async (command, args = {}) => {
      calls.push({ command, args });
      const handler = handlers[command];
      return handler ? handler(args) : null;
    });
    const failures: TurnFailure[] = [];
    const config = { preferences: { max_round_speakers: 1 } } as unknown as AppConfig;
    let chat!: ChatController;
    let place = { worldId: "w1", scene: 0 };
    function Harness({ worldId, scene }: { worldId: string; scene: number }) {
      chat = useChatController({
        worldId,
        scene,
        config,
        speaker: "gm",
        gmTargeted: true,
        metaOf: () => undefined,
        playerName: undefined,
        castCount: 1,
        onArrived: () => {},
        refreshState: async () => {},
        refreshWorlds: async () => {},
        noteChatStarted: () => {},
        markCliConnected: async () => {},
        onError: () => {},
        onTurnFailed: (failure) => failures.push(failure),
      });
      return (
        <PlayView
          onboarding={null}
          sceneLabel="第 1 幕"
          storyKey={`${worldId}\u0000${scene}`}
          events={chat.events}
          notices={chat.notices}
          metaOf={() => undefined}
          generating={null}
          generatingMeta={undefined}
          streamText=""
          busy={false}
          canRestore={false}
          onRestoreUndone={() => {}}
          canUndoScene={false}
          onRegenerateSummary={() => {}}
          onRevertScene={() => {}}
          speaker=""
          gmTargeted={false}
          targetName=""
          targetColor="#888888"
          targetImage={null}
          targetEmoji="🎭"
          onClearTarget={() => {}}
          input=""
          onInputChange={() => {}}
          castEmpty={false}
          onSubmit={() => {}}
          canStop={false}
          onStop={() => {}}
          requestReplyLabel=""
          onUndoLast={() => {}}
          onRequestReply={() => {}}
          onGmNarrate={() => {}}
          onGmAdvance={() => {}}
        />
      );
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    const render = () => act(() => root!.render(<Harness {...place} />));
    render();
    return {
      chat: () => chat,
      calls,
      failures,
      count: (command: string) => calls.filter((call) => call.command === command).length,
      lines: () => Array.from(host!.querySelectorAll(".message-system")).map((node) => node.textContent),
      moveTo(worldId: string, scene = 0) {
        place = { worldId, scene };
        render();
      },
    };
  }

  /** gm_narrate 進行中把事件灌進去：turn id 取自這一輪實際送給後端的那個 */
  function narrateWith(fire: (turnId: string) => void, outcome: () => unknown = () => ({ text: "門開了。", raw: null, next: null })) {
    return async (args: Record<string, unknown>) => {
      // 不等渲染：玩家句已 setEvents 但還沒 render 時事件就到，提示仍要落在玩家句之後
      fire(args.turnId as string);
      return outcome();
    };
  }

  const base: Record<string, Handler> = {
    append_player_event: (args) => ({ event: args.event, offset: 1 }),
    append_transcript: (args) => args.event,
    discard_unanswered_player: () => true,
  };

  async function send(h: ReturnType<typeof mount>, text = "你好") {
    act(() => h.chat().setInput(text));
    await act(async () => h.chat().send(formEvent));
  }

  it("shows a failover line for its own turn, once per eventId, and keeps it out of the transcript", async () => {
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => {
        emit("smart-free-failover", failover("e1", turnId));
        emit("smart-free-failover", failover("e1", turnId));
        emit("smart-free-model-switched", { eventId: "e2", world: "w1", turnId, model: "丙" });
      }),
    });
    await send(h);
    expect(h.lines()).toEqual([FAILOVER_TEXT(), t("smartFreeSwitched", { model: "丙" })]);
    // 提示不是逐字稿事件：沒落檔、沒進 events
    expect(h.chat().events.map((event) => event.text)).toEqual(["你好", "門開了。"]);
    const order = Array.from(host!.querySelectorAll(".messages > .message")).map((node) => node.textContent ?? "");
    const at = (text: string) => order.findIndex((line) => line.includes(text));
    expect(at("你好")).toBeLessThan(at(FAILOVER_TEXT()));
    const written = h.calls.filter((call) => call.command === "append_transcript").map((call) => (call.args.event as TranscriptEvent).text);
    expect(written.some((text) => text.includes("甲") || text.includes("乙"))).toBe(false);
  });

  it("remembers every eventId for the whole viewing: replaying the first one after 250 others adds no line", async () => {
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => {
        for (let i = 0; i < 250; i += 1) emit("smart-free-failover", failover(`e${i}`, turnId));
        emit("smart-free-failover", failover("e0", turnId));
      }),
    });
    await send(h);
    expect(h.lines()).toHaveLength(250);
    expect(new Set(h.lines()).size).toBe(1);
  });

  const order = () =>
    Array.from(host!.querySelectorAll(".messages > .message")).map((node) => node.textContent ?? "");
  const idx = (lines: string[], text: string) => lines.findIndex((line) => line.includes(text));

  it("order stays player line → notice → reply: event while the reply is pending, then after it lands", async () => {
    let turnId = "";
    let release!: (value: unknown) => void;
    const h = mount({
      ...base,
      gm_narrate: (args) => {
        turnId = args.turnId as string;
        return new Promise((resolve) => {
          release = resolve;
        });
      },
    });
    act(() => h.chat().setInput("你好"));
    let sending!: Promise<void>;
    await act(async () => {
      sending = h.chat().send(formEvent);
      for (let i = 0; i < 20; i += 1) await Promise.resolve();
    });
    expect(turnId).not.toBe("");
    emit("smart-free-failover", failover("e1", turnId));
    let lines = order();
    expect(lines.map((line) => line.trim())).toEqual(expect.arrayContaining([FAILOVER_TEXT()]));
    expect(idx(lines, "你好")).toBeLessThan(idx(lines, FAILOVER_TEXT()));
    await act(async () => {
      release({ text: "門開了。", raw: null, next: null });
      await sending;
    });
    // 回覆落地之後才到的同輪事件：仍排在玩家句與回覆之間
    emit("smart-free-failover", failover("e2", turnId));
    lines = order();
    expect(h.lines()).toHaveLength(2);
    expect(idx(lines, "你好")).toBeLessThan(idx(lines, FAILOVER_TEXT()));
    expect(lines.map((line, i) => (line === FAILOVER_TEXT() ? i : -1)).filter((i) => i >= 0).every((i) => i < idx(lines, "門開了。"))).toBe(true);
  });

  it("drops events from other turns, other worlds, and non-chat calls (turnId null)", async () => {
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => {
        emit("smart-free-failover", failover("x1", "someone-elses-turn"));
        emit("smart-free-failover", failover("x2", null));
        emit("smart-free-failover", failover("x3", turnId, "other-world"));
        emit("smart-free-failover", failover("x4", turnId, null));
      }),
    });
    await send(h);
    expect(h.lines()).toEqual([]);
  });

  it("clears on scene change, and an old turn's late event is dropped afterwards", async () => {
    let oldTurn = "";
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => {
        oldTurn = turnId;
        emit("smart-free-failover", failover("e1", turnId));
      }),
    });
    await send(h);
    expect(h.lines()).toEqual([FAILOVER_TEXT()]);
    h.moveTo("w1", 1);
    expect(h.lines()).toEqual([]);
    emit("smart-free-failover", failover("late", oldTurn));
    expect(h.lines()).toEqual([]);
    // 換回原幕也不復活：turn 集合已清空
    h.moveTo("w1", 0);
    emit("smart-free-failover", failover("late2", oldTurn));
    expect(h.lines()).toEqual([]);
  });

  it("leaving the table (and coming back) drops old events; other tables never show them", async () => {
    let oldTurn = "";
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => {
        oldTurn = turnId;
      }),
    });
    await send(h);
    h.moveTo("");
    h.moveTo("w1");
    emit("smart-free-failover", failover("back", oldTurn));
    expect(h.lines()).toEqual([]);
    h.moveTo("w2");
    emit("smart-free-failover", failover("w2", oldTurn, "w2"));
    expect(h.lines()).toEqual([]);
  });

  it("unmounting stops listening", async () => {
    mount(base);
    const count = () => [...bus.listeners.values()].reduce((sum, set) => sum + set.size, 0);
    await act(async () => {
      await Promise.resolve();
    });
    expect(count()).toBeGreaterThan(0);
    act(() => root!.unmount());
    root = null;
    expect(count()).toBe(0);
  });

  it("a retry that succeeds never discards the player line (one player sentence in the transcript)", async () => {
    const h = mount({
      ...base,
      gm_narrate: narrateWith((turnId) => emit("smart-free-failover", failover("e1", turnId))),
    });
    await send(h);
    expect(h.count("discard_unanswered_player")).toBe(0);
    expect(h.count("append_player_event")).toBe(1);
    expect(h.failures).toEqual([]);
    expect(h.chat().input).toBe("");
    expect(h.chat().events.filter((event) => event.kind === "player")).toHaveLength(1);
  });

  it("a final failure still discards the unanswered line once, hands the raw busy error to the dialog, and keeps the notice", async () => {
    const busy = "AI_FREE_MODEL_BUSY: AI_HTTP_STATUS_503: status=503 body=";
    const h = mount({
      ...base,
      gm_narrate: narrateWith(
        (turnId) => emit("smart-free-failover", failover("e1", turnId)),
        () => Promise.reject(busy),
      ),
    });
    await send(h, "我推開門");
    expect(h.count("discard_unanswered_player")).toBe(1);
    expect(h.chat().events).toEqual([]);
    expect(h.chat().input).toBe("我推開門");
    expect(h.failures).toEqual([{ raw: busy }]);
    expect(h.lines()).toEqual([FAILOVER_TEXT()]);
  });

  it("a failure after half the text streamed keeps the existing contract: nothing landed, half text dropped", async () => {
    const h = mount({
      ...base,
      gm_narrate: (args) => {
        (args.onDelta as { onmessage: (delta: string) => void }).onmessage("半句");
        emit("smart-free-failover", failover("e1", args.turnId as string));
        return Promise.reject("AI_FREE_MODEL_BUSY: AI_HTTP_STATUS_503: status=503 body=");
      },
    });
    await send(h);
    expect(h.count("append_transcript")).toBe(0);
    expect(h.chat().streamText).toBe("");
    expect(h.chat().events).toEqual([]);
    expect(h.failures).toHaveLength(1);
  });

  it("an aborted turn is not a failure and does not discard", async () => {
    const h = mount({
      ...base,
      gm_narrate: narrateWith(
        (turnId) => emit("smart-free-failover", failover("e1", turnId)),
        () => ({ text: "半截", raw: null, next: null, aborted: true }),
      ),
    });
    await send(h);
    expect(h.count("discard_unanswered_player")).toBe(0);
    expect(h.failures).toEqual([]);
  });
});
