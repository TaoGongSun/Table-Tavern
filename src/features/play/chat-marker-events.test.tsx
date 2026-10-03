// @vitest-environment happy-dom
// 點名與狀態更新改存標頭代碼（backend-zh-data-text）：點名事件本文是空的，落檔、收回、復原整條都要走得通；
// 沒有代碼的空白事件照舊擋。

import { act } from "react";
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

  it("lands a marker-only call event, undoes it and restores it", async () => {
    const written: TranscriptEvent[] = [];
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
        const event = args!.event as TranscriptEvent;
        written.push(event);
        return event;
      }
      if (command === "pop_transcript") return true;
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
    expect(current().events).toHaveLength(2);
    expect(current().canRestore).toBe(true);

    await act(async () => current().restoreUndone());
    expect(written[written.length - 1].marker).toEqual({ type: "gm_call", name: "" });
    const events = current().events;
    expect(events[events.length - 1].marker).toEqual({ type: "gm_call", name: "" });
    expect(current().canRestore).toBe(false);
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
