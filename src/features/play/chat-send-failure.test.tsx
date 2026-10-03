// @vitest-environment happy-dom
// 回合失敗（quota-insufficient-alert）：打字送出只在乾淨路徑自動收回玩家句、放回原文；
// 其他任何情況不收、不放回，原文交給失敗彈窗。換桌換幕世代、逐字稿操作互斥。

import { act, type FormEvent } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, TranscriptEvent } from "../../shared/contracts/backend-contracts";

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

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type Handler = (args: Record<string, unknown>) => unknown;

function gate<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const formEvent = { preventDefault() {} } as FormEvent<HTMLFormElement>;

function playerEvent(text: string, ts = "ts-player"): TranscriptEvent {
  return { ts, speaker_id: "", speaker_name: "", kind: "player", text };
}

describe("chat turn failures", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    invokeMock.mockReset();
  });

  function mount(
    handlers: Record<string, Handler>,
    options: { gm?: boolean; onArrived?: (ids: string[]) => void } = {},
  ) {
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    invokeMock.mockImplementation(async (command, args = {}) => {
      calls.push({ command, args });
      const handler = handlers[command];
      return handler ? handler(args) : null;
    });
    const errors: string[] = [];
    const failures: TurnFailure[] = [];
    const refreshState = vi.fn(async () => {});
    const refreshWorlds = vi.fn(async () => {});
    const config = { preferences: { max_round_speakers: 1 } } as unknown as AppConfig;
    let chat!: ChatController;
    let place = { worldId: "w1", scene: 0 };
    function Harness({ worldId, scene }: { worldId: string; scene: number }) {
      chat = useChatController({
        worldId,
        scene,
        config,
        speaker: options.gm ? "gm" : "fox",
        gmTargeted: options.gm ?? false,
        metaOf: () => ({ name: "狐狸" }) as never,
        playerName: undefined,
        castCount: 1,
        onArrived: options.onArrived ?? (() => {}),
        refreshState,
        refreshWorlds,
        noteChatStarted: () => {},
        markCliConnected: async () => {},
        onError: (message) => {
          if (message) errors.push(message);
        },
        onTurnFailed: (failure) => failures.push(failure),
      });
      return null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    const render = () => act(() => root!.render(<Harness {...place} />));
    render();
    return {
      chat: () => chat,
      calls,
      errors,
      failures,
      refreshState,
      refreshWorlds,
      count: (command: string) => calls.filter((call) => call.command === command).length,
      moveTo(worldId: string, scene = 0) {
        place = { worldId, scene };
        render();
      },
    };
  }

  async function typeAndSend(harness: ReturnType<typeof mount>, text: string) {
    act(() => harness.chat().setInput(text));
    await act(async () => harness.chat().send(formEvent));
  }

  const appendPlayer = (offset = 42): Handler => (args) => ({ event: args.event, offset });

  it("discards the unanswered line and puts the raw text back (character)", async () => {
    const h = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => Promise.reject("AI_HTTP_STATUS_429: slow down"),
      discard_unanswered_player: () => true,
    });
    await typeAndSend(h, "  我推開門 ");
    const discard = h.calls.find((call) => call.command === "discard_unanswered_player")!;
    const appended = h.calls.find((call) => call.command === "append_player_event")!;
    expect(discard.args).toEqual({
      worldId: "w1",
      scene: 0,
      offset: 42,
      ts: (appended.args.event as TranscriptEvent).ts,
      text: "我推開門",
    });
    expect(h.chat().events).toEqual([]);
    expect(h.chat().input).toBe("  我推開門 ");
    expect(h.refreshState).toHaveBeenCalled();
    expect(h.failures).toEqual([{ raw: "AI_HTTP_STATUS_429: slow down" }]);
    expect(h.errors).toEqual([]);
    expect(h.chat().busy).toBe(false);
  });

  it("does the same when the GM narration fails, dropping the streamed half sentence", async () => {
    const h = mount(
      {
        append_player_event: appendPlayer(),
        gm_narrate: (args) => {
          (args.onDelta as { onmessage: (delta: string) => void }).onmessage("半句");
          return Promise.reject("AI_CALL_FAILED: boom");
        },
        discard_unanswered_player: () => true,
      },
      { gm: true },
    );
    await typeAndSend(h, "你好");
    expect(h.count("append_transcript")).toBe(0);
    expect(h.chat().streamText).toBe("");
    expect(h.chat().events).toEqual([]);
    expect(h.chat().input).toBe("你好");
    expect(h.failures).toEqual([{ raw: "AI_CALL_FAILED: boom" }]);
  });

  it("restores the input even if refreshing the table state fails afterwards", async () => {
    const h = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
      discard_unanswered_player: () => true,
    });
    h.refreshState.mockRejectedValueOnce("state broke");
    await typeAndSend(h, "你好");
    expect(h.chat().input).toBe("你好");
    expect(h.chat().events).toEqual([]);
    expect(h.errors).toEqual(["state broke"]);
  });

  it.each([
    ["false", () => false],
    ["an error", () => Promise.reject("io")],
  ])("keeps the line and hands the draft to the dialog when discard returns %s", async (_, discard) => {
    const h = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
      discard_unanswered_player: discard,
    });
    await typeAndSend(h, " 你好 ");
    expect(h.chat().events.map((event) => event.text)).toEqual(["你好"]);
    expect(h.chat().input).toBe("");
    expect(h.failures).toEqual([{ raw: "AI_CALL_FAILED: boom", draft: " 你好 " }]);
  });

  it("never discards when the player line itself failed to append, and re-reads the transcript", async () => {
    const h = mount({
      append_player_event: () => Promise.reject("disk full"),
      read_transcript: () => [playerEvent("舊的一句", "t0")],
    });
    await typeAndSend(h, "你好");
    expect(h.count("discard_unanswered_player")).toBe(0);
    expect(h.count("chat_with_character")).toBe(0);
    expect(h.chat().input).toBe("");
    expect(h.failures).toEqual([{ raw: "disk full", draft: "你好" }]);
    expect(h.chat().events.map((event) => event.text)).toEqual(["舊的一句"]);
  });

  it("reports a failed re-read after the draft is already with the dialog", async () => {
    const h = mount({
      append_player_event: () => Promise.reject("disk full"),
      read_transcript: () => Promise.reject("read broke"),
    });
    await typeAndSend(h, "你好");
    expect(h.failures).toEqual([{ raw: "disk full", draft: "你好" }]);
    expect(h.errors).toEqual(["read broke"]);
  });

  it("leaves a landed GM narration alone when a later step fails", async () => {
    const h = mount(
      {
        append_player_event: appendPlayer(),
        gm_narrate: () => ({ text: "門開了。", raw: null, next: null }),
        append_transcript: (args) => args.event,
        discard_unanswered_player: () => false,
      },
      { gm: true },
    );
    h.refreshWorlds.mockRejectedValueOnce("list broke");
    await typeAndSend(h, "你好");
    expect(h.count("discard_unanswered_player")).toBe(1);
    expect(h.chat().events.map((event) => event.text)).toEqual(["你好", "門開了。"]);
    expect(h.failures).toEqual([{ raw: "list broke", draft: "你好" }]);
  });

  it("a GM reply that never landed shows up before the next player line, as the backend lands it", async () => {
    const disk: TranscriptEvent[] = [];
    const handlers: Record<string, Handler> = {
      append_player_event: (args) => {
        disk.push(args.event as TranscriptEvent);
        return { event: args.event, offset: 1 };
      },
      gm_narrate: () => ({ text: "門開了。", raw: null, next: null }),
      append_transcript: () => Promise.reject("disk full"),
      discard_unanswered_player: () => false,
      read_transcript: () => [...disk],
    };
    const h = mount(handlers, { gm: true });
    await typeAndSend(h, "你好");
    expect(h.count("append_transcript")).toBe(3);
    expect(h.failures).toEqual([{ raw: "disk full", draft: "你好" }]);
    // 下一句送出：後端在玩家句之前代落了上一輪的 GM 回覆
    handlers.append_player_event = (args) => {
      disk.push({ ts: "gm", speaker_id: "", speaker_name: "GM", kind: "narration", text: "門開了。" });
      disk.push(args.event as TranscriptEvent);
      return { event: args.event, offset: 2 };
    };
    handlers.gm_narrate = () => ({ text: "又開了一扇。", raw: null, next: null });
    handlers.append_transcript = (args) => args.event;
    await typeAndSend(h, "再一句");
    const order = h.calls.map((call) => call.command);
    const sent = order.lastIndexOf("append_player_event");
    expect(order.slice(sent, sent + 3)).toEqual(["append_player_event", "read_transcript", "gm_narrate"]);
    expect(h.chat().events.map((event) => event.text)).toEqual(["你好", "門開了。", "再一句", "又開了一扇。"]);
  });

  it("removes only this turn's object even when an identical line is already on screen", async () => {
    const h = mount({
      append_player_event: () => ({ event: playerEvent("再說一次", "same"), offset: 80 }),
      chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
      discard_unanswered_player: () => true,
    });
    const earlier = playerEvent("再說一次", "same");
    act(() => h.chat().hydrate([earlier]));
    await typeAndSend(h, "再說一次");
    expect(h.chat().events).toHaveLength(1);
    expect(h.chat().events[0]).toBe(earlier);
  });

  it("treats a normal abort as no failure, with or without text", async () => {
    const quiet = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => ({ text: "", aborted: true }),
    });
    await typeAndSend(quiet, "你好");
    expect(quiet.count("discard_unanswered_player")).toBe(0);
    expect(quiet.failures).toEqual([]);
    expect(quiet.chat().events.map((event) => event.text)).toEqual(["你好"]);
    act(() => root?.unmount());
    host?.remove();

    const partial = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => ({ text: "晚", aborted: true }),
      append_transcript: (args) => args.event,
    });
    await typeAndSend(partial, "你好");
    expect(partial.count("discard_unanswered_player")).toBe(0);
    expect(partial.chat().events.map((event) => [event.text, event.truncated ?? false])).toEqual([
      ["你好", false],
      ["晚", true],
    ]);
  });

  it("card submissions neither discard nor touch the input box", async () => {
    const h = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
    });
    act(() => h.chat().setInput("草稿"));
    await act(async () => h.chat().submitText("卡片的話"));
    expect(h.count("discard_unanswered_player")).toBe(0);
    expect(h.chat().input).toBe("草稿");
    expect(h.failures).toEqual([{ raw: "AI_CALL_FAILED: boom" }]);
  });

  it("a successful card submission keeps the input box as it was", async () => {
    const h = mount({
      append_player_event: appendPlayer(),
      chat_with_character: () => ({ text: "晚安", aborted: false }),
      append_transcript: (args) => args.event,
    });
    act(() => h.chat().setInput("草稿"));
    await act(async () => h.chat().submitText("卡片的話"));
    expect(h.chat().input).toBe("草稿");
  });

  it("button-driven turns go to the dialog without any discard", async () => {
    const h = mount({
      chat_with_character: () => Promise.reject("AI_CALL_FAILED: a"),
      gm_narrate: () => Promise.reject("AI_CALL_FAILED: b"),
    });
    await act(async () => h.chat().replyFromTarget());
    await act(async () => h.chat().gmNarrate());
    await act(async () => h.chat().gmAdvance());
    expect(h.failures.map((failure) => failure.raw)).toEqual([
      "AI_CALL_FAILED: a",
      "AI_CALL_FAILED: b",
      "AI_CALL_FAILED: b",
    ]);
    expect(h.failures.every((failure) => failure.draft === undefined)).toBe(true);
    expect(h.count("discard_unanswered_player")).toBe(0);
    expect(h.errors).toEqual([]);
  });

  describe("table / scene generation", () => {
    it("reply pending, scene changed (and changed back): no discard", async () => {
      const reply = gate<unknown>();
      const h = mount({
        append_player_event: appendPlayer(),
        chat_with_character: () => reply.promise,
      });
      act(() => h.chat().setInput("你好"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      h.moveTo("w1", 1);
      h.moveTo("w1", 0);
      await act(async () => {
        reply.reject("AI_CALL_FAILED: boom");
        await sending;
      });
      expect(h.count("discard_unanswered_player")).toBe(0);
      expect(h.chat().input).toBe("");
      expect(h.failures).toEqual([{ raw: "AI_CALL_FAILED: boom", draft: "你好" }]);
    });

    it("discard already sent, then table switched: no update on the new table, draft kept", async () => {
      const discard = gate<boolean>();
      const h = mount({
        append_player_event: appendPlayer(),
        chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
        discard_unanswered_player: () => discard.promise,
      });
      act(() => h.chat().setInput("你好"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      await vi.waitFor(() => expect(h.count("discard_unanswered_player")).toBe(1));
      h.moveTo("w2");
      const other = playerEvent("別桌", "t-w2");
      act(() => h.chat().hydrate([other]));
      await act(async () => {
        discard.resolve(true);
        await sending;
      });
      expect(h.count("discard_unanswered_player")).toBe(1);
      expect(h.chat().events).toEqual([other]);
      expect(h.chat().input).toBe("");
      expect(h.refreshState).not.toHaveBeenCalled();
      expect(h.failures).toEqual([{ raw: "AI_CALL_FAILED: boom", draft: "你好" }]);
    });

    it("a re-read that lands after a table switch (even back again) does not overwrite the screen", async () => {
      const read = gate<TranscriptEvent[]>();
      const h = mount({
        append_player_event: () => Promise.reject("disk full"),
        read_transcript: () => read.promise,
      });
      act(() => h.chat().setInput("你好"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      await vi.waitFor(() => expect(h.count("read_transcript")).toBe(1));
      h.moveTo("w2");
      h.moveTo("w1");
      const fresh = playerEvent("換回來後的畫面", "t-fresh");
      act(() => h.chat().hydrate([fresh]));
      await act(async () => {
        read.resolve([playerEvent("舊讀取", "t-old")]);
        await sending;
      });
      expect(h.chat().events).toEqual([fresh]);
    });
  });

  describe("a turn that outlives a table switch", () => {
    it("player append pending, table switched: no AI call for the old table, new table untouched", async () => {
      const append = gate<unknown>();
      const h = mount({
        append_player_event: () => append.promise,
        chat_with_character: () => ({ text: "舊桌成功回覆", aborted: false }),
        append_transcript: (args) => args.event,
      });
      act(() => h.chat().setInput("原桌玩家句"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      h.moveTo("w2");
      const fresh = playerEvent("新桌事件", "fresh");
      act(() => h.chat().hydrate([fresh]));
      await act(async () => {
        append.resolve({ event: playerEvent("原桌玩家句"), offset: 0 });
        await sending;
      });
      expect(h.chat().events).toEqual([fresh]);
      expect(h.count("chat_with_character")).toBe(0);
      expect(h.failures).toEqual([]);
    });

    it("reply already pending when the table changes: lands in the old table only", async () => {
      const reply = gate<unknown>();
      let delta!: (text: string) => void;
      const h = mount({
        append_player_event: appendPlayer(),
        chat_with_character: (args) => {
          delta = (text) => (args.onDelta as { onmessage: (value: string) => void }).onmessage(text);
          return reply.promise;
        },
        append_transcript: (args) => args.event,
      });
      act(() => h.chat().setInput("原桌玩家句"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      await vi.waitFor(() => expect(h.count("chat_with_character")).toBe(1));
      h.moveTo("w2");
      const fresh = playerEvent("新桌事件", "fresh");
      act(() => h.chat().hydrate([fresh]));
      act(() => delta("舊桌串流"));
      expect(h.chat().streamText).toBe("");
      await act(async () => {
        reply.resolve({ text: "舊桌成功回覆", aborted: false });
        await sending;
      });
      expect(h.chat().events).toEqual([fresh]);
      const landed = h.calls.filter((call) => call.command === "append_transcript");
      expect(landed.map((call) => call.args.worldId)).toEqual(["w1"]);
    });

    it("GM narration pending when the table changes: no state refresh, arrivals or relay on the new table", async () => {
      const narration = gate<unknown>();
      const arrived = vi.fn();
      const h = mount(
        {
          gm_narrate: () => narration.promise,
          append_transcript: (args) => args.event,
          chat_with_character: () => ({ text: "接話", aborted: false }),
        },
        { gm: true, onArrived: arrived },
      );
      let advancing!: Promise<void>;
      await act(async () => {
        advancing = h.chat().gmAdvance();
      });
      h.moveTo("w2");
      const fresh = playerEvent("新桌事件", "fresh");
      act(() => h.chat().hydrate([fresh]));
      await act(async () => {
        narration.resolve({ text: "舊桌旁白", raw: null, next: "fox", arrived_characters: ["fox"] });
        await advancing;
      });
      expect(h.chat().events).toEqual([fresh]);
      expect(h.refreshState).not.toHaveBeenCalled();
      expect(arrived).not.toHaveBeenCalled();
      expect(h.count("chat_with_character")).toBe(0);
      const landed = h.calls.filter((call) => call.command === "append_transcript");
      expect(landed.map((call) => (call.args.event as TranscriptEvent).text)).toEqual(["舊桌旁白"]);
    });

    it("table changes while the GM turn is refreshing state: no call event, no relay", async () => {
      const refresh = gate<void>();
      const h = mount(
        {
          gm_narrate: () => ({ text: "舊桌旁白", raw: null, next: "fox" }),
          append_transcript: (args) => args.event,
          chat_with_character: () => ({ text: "接話", aborted: false }),
        },
        { gm: true },
      );
      h.refreshState.mockImplementationOnce(() => refresh.promise);
      let advancing!: Promise<void>;
      await act(async () => {
        advancing = h.chat().gmAdvance();
      });
      await vi.waitFor(() => expect(h.refreshState).toHaveBeenCalled());
      h.moveTo("w2");
      await act(async () => {
        refresh.resolve();
        await advancing;
      });
      const landed = h.calls.filter((call) => call.command === "append_transcript");
      expect(landed.map((call) => (call.args.event as TranscriptEvent).text)).toEqual(["舊桌旁白"]);
      expect(h.count("chat_with_character")).toBe(0);
    });

    it("undo or restore finishing after a switch leaves the new table alone", async () => {
      const pop = gate<boolean>();
      const restore = gate<unknown>();
      const h = mount({
        pop_transcript: () => pop.promise,
        append_transcript: () => restore.promise,
      });
      act(() => h.chat().hydrate([playerEvent("上一句", "t0")]));
      let undoing!: Promise<void>;
      await act(async () => {
        undoing = h.chat().undoLast();
      });
      h.moveTo("w2");
      const fresh = playerEvent("新桌事件", "fresh");
      act(() => h.chat().hydrate([fresh]));
      await act(async () => {
        pop.resolve(true);
        await undoing;
      });
      expect(h.chat().events).toEqual([fresh]);
      expect(h.chat().canRestore).toBe(false);
      expect(h.refreshState).not.toHaveBeenCalled();

      // 回原桌收回一句再復原，復原等待中換桌
      h.moveTo("w1");
      act(() => h.chat().hydrate([playerEvent("上一句", "t0")]));
      const popNow = gate<boolean>();
      h.calls.length = 0;
      invokeMock.mockImplementation(async (command, args = {}) => {
        h.calls.push({ command, args });
        if (command === "pop_transcript") return popNow.promise;
        if (command === "append_transcript") return restore.promise;
        return null;
      });
      let undoAgain!: Promise<void>;
      await act(async () => {
        undoAgain = h.chat().undoLast();
        popNow.resolve(true);
        await undoAgain;
      });
      expect(h.chat().canRestore).toBe(true);
      let restoring!: Promise<void>;
      await act(async () => {
        restoring = h.chat().restoreUndone();
      });
      h.moveTo("w2");
      act(() => h.chat().hydrate([fresh]));
      await act(async () => {
        restore.resolve(null);
        await restoring;
      });
      expect(h.chat().events).toEqual([fresh]);
    });
  });

  describe("one transcript operation at a time", () => {
    it("blocks sending while an undo is still writing", async () => {
      const pop = gate<boolean>();
      const h = mount({
        pop_transcript: () => pop.promise,
        append_player_event: appendPlayer(),
      });
      act(() => h.chat().hydrate([playerEvent("上一句", "t0")]));
      let undoing!: Promise<void>;
      await act(async () => {
        undoing = h.chat().undoLast();
      });
      await typeAndSend(h, "你好");
      expect(h.count("append_player_event")).toBe(0);
      await act(async () => {
        pop.resolve(true);
        await undoing;
      });
    });

    it("blocks undo while a send is running, and a double send only sends once", async () => {
      const append = gate<unknown>();
      const h = mount({
        append_player_event: () => append.promise,
        chat_with_character: () => ({ text: "晚安", aborted: false }),
        append_transcript: (args) => args.event,
        pop_transcript: () => true,
      });
      act(() => h.chat().setInput("你好"));
      let first!: Promise<void>;
      await act(async () => {
        first = h.chat().send(formEvent);
      });
      expect(h.chat().busy).toBe(true);
      expect(h.chat().isBusy()).toBe(true);
      await act(async () => h.chat().send(formEvent));
      await act(async () => h.chat().undoLast());
      expect(h.count("append_player_event")).toBe(1);
      expect(h.count("pop_transcript")).toBe(0);
      await act(async () => {
        append.resolve({ event: playerEvent("你好"), offset: 0 });
        await first;
      });
      expect(h.chat().busy).toBe(false);
    });

    it("blocks sending while a restore is still writing", async () => {
      const restore = gate<unknown>();
      const h = mount({
        pop_transcript: () => true,
        append_transcript: () => restore.promise,
        append_player_event: appendPlayer(),
      });
      act(() => h.chat().hydrate([playerEvent("上一句", "t0")]));
      await act(async () => h.chat().undoLast());
      expect(h.chat().canRestore).toBe(true);
      let restoring!: Promise<void>;
      await act(async () => {
        restoring = h.chat().restoreUndone();
      });
      await typeAndSend(h, "你好");
      expect(h.count("append_player_event")).toBe(0);
      await act(async () => {
        // 後端回傳落檔的那則（帶表的事件版本會換新），畫面放回的是這份
        restore.resolve(playerEvent("上一句", "t0"));
        await restoring;
      });
      expect(h.chat().events.map((event) => event.text)).toEqual(["上一句"]);
    });

    it("blocks a new send while the failed line is being discarded", async () => {
      const discard = gate<boolean>();
      const h = mount({
        append_player_event: appendPlayer(),
        chat_with_character: () => Promise.reject("AI_CALL_FAILED: boom"),
        discard_unanswered_player: () => discard.promise,
      });
      act(() => h.chat().setInput("你好"));
      let sending!: Promise<void>;
      await act(async () => {
        sending = h.chat().send(formEvent);
      });
      await vi.waitFor(() => expect(h.count("discard_unanswered_player")).toBe(1));
      await act(async () => h.chat().submitText("插隊"));
      expect(h.count("append_player_event")).toBe(1);
      await act(async () => {
        discard.resolve(true);
        await sending;
      });
      expect(h.chat().input).toBe("你好");
    });
  });
});
