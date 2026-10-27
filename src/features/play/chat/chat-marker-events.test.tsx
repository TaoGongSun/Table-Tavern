// @vitest-environment happy-dom
// 點名與狀態更新改存標頭代碼（backend-zh-data-text）：點名事件本文是空的，落檔、收回、復原整條都要走得通；
// 沒有代碼的空白事件照舊擋。

import { act } from "react";
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

import { useChatController, type ChatController } from "./useChatController";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("chat controller with marker events", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    invokeMock.mockReset();
  });

  function mount(playerName: string | undefined) {
    const config = { preferences: { max_round_speakers: 1 } } as unknown as AppConfig;
    const errors: string[] = [];
    let chat!: ChatController;
    function Harness() {
      chat = useChatController({
        worldId: "w1",
        scene: 0,
        config,
        speaker: "gm",
        gmTargeted: true,
        metaOf: () => undefined,
        playerName,
        castCount: 1,
        onArrived: () => {},
        refreshState: async () => {},
        refreshWorlds: async () => {},
        noteChatStarted: () => {},
        markCliConnected: async () => {},
        onError: (message) => {
          if (message) errors.push(message);
        },
        onTurnFailed: ({ raw }) => errors.push(raw),
      });
      return null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root!.render(<Harness />));
    return { current: () => chat, errors };
  }

  it("lands a marker-only call event; one undo takes the whole GM floor and restore puts it back in order", async () => {
    const written: TranscriptEvent[] = [];
    const restoredSent: TranscriptEvent[][] = [];
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "gm_narrate") {
        return {
          text: "夜深了。",
          raw: null,
          next: "__PLAYER__",
          state_updates: [{ path: "hp", value: "3" }],
        };
      }
      if (command === "append_transcript") {
        const event = { ...(args!.event as TranscriptEvent), id: `e${written.length}` };
        written.push(event);
        return event;
      }
      // 後端一次收整樓：正文、變動摘要、點名一起回來（原序）
      if (command === "pop_transcript_floor") return written.slice(0, 3);
      if (command === "restore_transcript_events") {
        const events = args!.events as TranscriptEvent[];
        restoredSent.push(events);
        return events;
      }
      return null;
    });
    const { current, errors } = mount(undefined);

    await act(async () => current().gmAdvance());
    expect(errors).toEqual([]);
    expect(written.map((event) => event.marker ?? null)).toEqual([
      null,
      { type: "state_update" },
      { type: "gm_call", name: "" },
    ]);
    expect(written[1].text).toBe("hp：3");
    expect(written[2].text).toBe("");

    await act(async () => current().undoLast());
    expect(current().events).toHaveLength(0);
    expect(current().canRestore).toBe(true);

    await act(async () => current().restoreUndone());
    expect(restoredSent).toHaveLength(1);
    expect(restoredSent[0].map((event) => event.marker ?? null)).toEqual([
      null,
      { type: "state_update" },
      { type: "gm_call", name: "" },
    ]);
    expect(current().events.map((event) => event.id)).toEqual(["e0", "e1", "e2"]);
    expect(current().canRestore).toBe(false);
  });

  it("a failed restore keeps the whole floor on the stack and the retry sends it once", async () => {
    const floor: TranscriptEvent[] = [
      { id: "a", ts: "1", speaker_id: "", speaker_name: "GM", kind: "narration", text: "旁白" },
      { id: "b", ts: "2", speaker_id: "", speaker_name: "GM", kind: "system", text: "hp：3", marker: { type: "state_update" } },
    ];
    let fail = true;
    const sent: TranscriptEvent[][] = [];
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "pop_transcript_floor") return floor;
      if (command === "restore_transcript_events") {
        sent.push(args!.events as TranscriptEvent[]);
        if (fail) {
          fail = false;
          throw new Error("io");
        }
        return args!.events;
      }
      return null;
    });
    const { current, errors } = mount(undefined);
    act(() => current().hydrate(floor));
    await act(async () => current().undoLast());
    expect(current().events).toHaveLength(0);
    await act(async () => current().restoreUndone());
    expect(errors.some((message) => message.includes("io"))).toBe(true);
    expect(current().events).toHaveLength(0);
    expect(current().canRestore).toBe(true);
    await act(async () => current().restoreUndone());
    expect(sent).toHaveLength(2);
    expect(sent[1].map((event) => event.id)).toEqual(["a", "b"]);
    expect(current().events.map((event) => event.id)).toEqual(["a", "b"]);
    expect(current().canRestore).toBe(false);
  });

  it("GM 回合的正文與變動紀錄帶同一個 turn_id 與各自的 turn_part 落檔；點名與中止半截照規則帶或不帶", async () => {
    const turns: { part: unknown; turnId: unknown; text: string }[] = [];
    let narrateTurn: unknown = null;
    let aborted = false;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "gm_narrate") {
        narrateTurn = args!.turnId;
        return aborted
          ? { text: "半截", raw: null, next: null, aborted: true }
          : { text: "夜深了。", raw: null, next: "__PLAYER__", state_updates: [{ path: "hp", value: "3" }] };
      }
      if (command === "append_transcript") {
        const event = args!.event as TranscriptEvent;
        turns.push({ part: args!.turnPart, turnId: args!.turnId, text: event.text });
        return event;
      }
      return null;
    });
    const { current } = mount("阿濤");
    await act(async () => current().gmAdvance());
    expect(turns).toEqual([
      { part: "main", turnId: narrateTurn, text: "夜深了。" },
      { part: "state_update", turnId: narrateTurn, text: "hp：3" },
      { part: null, turnId: null, text: "" },
    ]);
    turns.length = 0;
    aborted = true;
    await act(async () => current().gmNarrate());
    expect(turns).toEqual([{ part: "main", turnId: narrateTurn, text: "半截" }]);
  });

  it("正文落檔失敗用同一個冪等鍵重試；狀態沒寫成時照落正文並提示；復原時表以 JSON 文字送回保住鍵順序", async () => {
    const calls: { part: unknown; turnId: unknown; vars: unknown }[] = [];
    let failures = 2;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "gm_narrate") return { text: "夜深了。", raw: null, next: null, state_error: "磁碟滿了" };
      if (command === "append_transcript") {
        const event = args!.event as TranscriptEvent;
        calls.push({ part: args!.turnPart, turnId: args!.turnId, vars: event.message_vars });
        if (args!.turnPart === "main" && failures > 0) {
          failures -= 1;
          throw new Error("io");
        }
        return { ...event, id: "e1", message_vars: { z: 1, a: 2 } };
      }
      if (command === "pop_transcript_floor") return [current().events[0]];
      if (command === "restore_transcript_events") {
        const events = args!.events as TranscriptEvent[];
        calls.push({ part: "restore", turnId: null, vars: events[0].message_vars });
        return events;
      }
      return null;
    });
    const { current, errors } = mount("阿濤");
    await act(async () => current().gmNarrate());
    expect(calls.map((call) => call.part)).toEqual(["main", "main", "main"]);
    expect(new Set(calls.map((call) => call.turnId)).size).toBe(1);
    expect(errors).toContain("磁碟滿了");
    expect(current().events).toHaveLength(1);
    await act(async () => current().undoLast());
    await act(async () => current().restoreUndone());
    expect(calls[calls.length - 1].vars).toBe('{"z":1,"a":2}');
  });

  it("stores an empty player name and still blocks blank unmarked events", async () => {
    const written: TranscriptEvent[] = [];
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "append_transcript" || command === "append_player_event") {
        const event = args!.event as TranscriptEvent;
        written.push(event);
        return command === "append_player_event" ? { event, offset: 0 } : event;
      }
      if (command === "gm_narrate") return { text: "  ", raw: null, next: null };
      return null;
    });
    const { current, errors } = mount(undefined);

    await act(async () => current().submitText("你好"));
    expect(written[0]).toMatchObject({ kind: "player", speaker_name: "", text: "你好" });
    expect(errors.some((message) => message.includes("AI_EMPTY_RESPONSE"))).toBe(true);
    expect(written).toHaveLength(1);
  });
});
