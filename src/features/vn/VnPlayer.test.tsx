// @vitest-environment happy-dom
// VN 播放器元件測試：假後端回格，假聊天控制器只記呼叫；打字機以 reduced-motion 直接全出。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type {
  TranscriptEvent,
  VnCell,
  VnEventCells,
} from "../../shared/contracts/backend-contracts";
import type { ChatController } from "../play/chat/useChatController";

const backend = vi.hoisted(() => ({
  cells: [] as VnEventCells[],
  parse: (_text: string): { cells: VnCell[]; tail_tentative: boolean } | null => null,
  reroll: (): Promise<string[]> => Promise.resolve([]),
  calls: [] as string[],
  /** 讓整幕重抓晚一拍回來，跟串流解析落在同一批更新 */
  slowRead: false,
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown>) => {
    backend.calls.push(command);
    if (command === "read_vn_cells") {
      if (backend.slowRead) await new Promise((resolve) => setTimeout(resolve, 0));
      return structuredClone(backend.cells);
    }
    if (command === "parse_vn_text") return backend.parse(args.text as string);
    if (command === "vn_reroll_choices") return backend.reroll();
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import { VnPlayer } from "./VnPlayer";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const n = (text: string): VnCell => ({ type: "narration", text });
const choices = (...options: string[]): VnCell => ({ type: "choices", options });
const gm = (id: string, cells: VnCell[]): VnEventCells => ({ event_id: id, kind: "narration", cells });
const player = (id: string, text: string): VnEventCells => ({
  event_id: id,
  kind: "player",
  cells: [n(text)],
});
const event = (id: string): TranscriptEvent => ({
  ts: "",
  speaker_id: "",
  speaker_name: "GM",
  kind: "narration",
  text: id,
  id,
});

let root: Root;
let host: HTMLElement;
let chat: ChatController;

function makeChat(): ChatController {
  return {
    events: [],
    generating: null,
    streamText: "",
    busy: false,
    canStop: false,
    stopResponse: vi.fn(),
    submitText: vi.fn(async () => {}),
    gmAdvance: vi.fn(async () => {}),
    reload: vi.fn(async () => {}),
  } as unknown as ChatController;
}

async function settle() {
  for (let i = 0; i < 5; i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

async function render(patch: Partial<ChatController> = {}) {
  chat = { ...chat, ...patch };
  await act(async () => {
    root.render(
      <VnPlayer
        worldId="w1"
        scene={0}
        tableName="Harbor"
        chat={chat}
        images={{}}
        notice={null}
        busy={chat.busy}
        onError={() => {}}
        onShowChat={() => {}}
        onGoLobby={() => {}}
      />,
    );
  });
  await settle();
}

const text = () => host.querySelector(".vn-dialogue .vn-text")?.textContent ?? "";
const button = (label: string) =>
  [...host.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent === label);
const choiceButtons = () => [...host.querySelectorAll<HTMLButtonElement>(".vn-choice")];
const advance = async () => {
  await act(async () => host.querySelector<HTMLButtonElement>(".vn-dialogue")!.click());
  await settle();
};

beforeEach(() => {
  backend.cells = [];
  backend.parse = () => null;
  backend.calls = [];
  backend.slowRead = false;
  window.matchMedia = (() => ({ matches: true })) as unknown as typeof window.matchMedia;
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  chat = makeChat();
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("VnPlayer", () => {
  it("plays from the last event and a choice goes to submitText exactly once", async () => {
    backend.cells = [gm("g0", [n("old")]), gm("g1", [n("hello"), choices("Go left", "Go right")])];
    let finish = () => {};
    chat.submitText = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    await render({ events: [event("g0"), event("g1")] });
    expect(text()).toBe("hello");
    await advance();
    expect(choiceButtons().map((b) => b.textContent)).toEqual(["Go left", "Go right"]);
    await act(async () => {
      choiceButtons()[1].click();
      choiceButtons()[1].click();
    });
    expect(chat.submitText).toHaveBeenCalledTimes(1);
    expect(chat.submitText).toHaveBeenCalledWith("Go right");
    expect(choiceButtons().every((b) => b.disabled)).toBe(true);
    await act(async () => finish());
  });

  it("a choice already answered by the player does not show the panel again", async () => {
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")]), player("p1", "A")];
    await render({ events: [event("g1"), event("p1")] });
    await advance();
    expect(choiceButtons()).toHaveLength(0);
    await act(async () => button(t("vnContinue"))!.click());
    expect(chat.gmAdvance).toHaveBeenCalledTimes(1);
  });

  it("reroll shows new options of the same event only after the refetch, and keeps the lock till then", async () => {
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")])];
    let release = () => {};
    backend.reroll = () => {
      backend.cells = [gm("g1", [n("hello"), choices("C", "D")])];
      return Promise.resolve(["C", "D"]);
    };
    chat.reload = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    await render({ events: [event("g1")] });
    await advance();
    await act(async () => button(t("vnReroll"))!.click());
    await settle();
    expect(button(t("vnReroll"))!.disabled).toBe(true);
    await act(async () => release());
    await settle();
    expect(choiceButtons().map((b) => b.textContent)).toEqual(["C", "D"]);
    expect(button(t("vnReroll"))!.disabled).toBe(false);
    expect(backend.calls.filter((c) => c === "vn_reroll_choices")).toHaveLength(1);
  });

  it.each([
    ["BUSY", "vnRerollBusy"],
    ["STALE", "vnRerollStale"],
    ["FAILED", "vnRerollFailed"],
    ["UNAVAILABLE", "vnRerollUnavailable"],
  ] as const)("reroll error %s shows its message and keeps the old options", async (code, key) => {
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")])];
    backend.reroll = () => Promise.reject(`VN_REROLL_${code}: detail`);
    await render({ events: [event("g1")] });
    await advance();
    await act(async () => button(t("vnReroll"))!.click());
    await settle();
    expect(host.querySelector(".vn-choice-error")?.textContent).toBe(t(key));
    expect(choiceButtons().map((b) => b.textContent)).toEqual(["A", "B"]);
  });

  it("streamed cells hand over to the landed event without duplicates", async () => {
    backend.cells = [gm("g1", [n("hello")])];
    await render({ events: [event("g1")] });
    await act(async () => button(t("vnContinue"))!.click());
    expect(chat.gmAdvance).toHaveBeenCalledTimes(1);
    backend.parse = () => ({ cells: [n("one"), n("two")], tail_tentative: false });
    const generating = { id: "", kind: "narration" as const };
    await render({ generating, streamText: "one\ntwo\n" });
    expect(text()).toBe("one");
    backend.cells = [gm("g1", [n("hello")]), gm("g2", [n("one"), n("two")])];
    await render({ events: [event("g1"), event("g2")] });
    await render({ generating: null });
    expect(text()).toBe("one");
    await advance();
    expect(text()).toBe("two");
    await advance();
    expect(text()).toBe("two");
    expect(button(t("vnContinue"))).toBeDefined();
  });

  it("waits with an ellipsis while the next cell is still generating", async () => {
    backend.cells = [gm("g1", [n("hello")])];
    await render({ events: [event("g1")], generating: { id: "", kind: "narration" } });
    expect(host.querySelector(".vn-waiting")).not.toBeNull();
    expect(button(t("vnContinue"))).toBeUndefined();
  });

  it("an empty table starts the first turn from its first cell, not the newest one", async () => {
    await render();
    await act(async () => button(t("vnContinue"))!.click());
    backend.parse = () => ({
      cells: [{ type: "bg", location: "Track", time: null }, n("first"), n("second"), n("third")],
      tail_tentative: false,
    });
    await render({ generating: { id: "", kind: "narration" }, streamText: "first\nsecond\nthird\n" });
    expect(text()).toBe("first");
  });

  it("after a choice the reply plays from its first cell even when it lands with the player line", async () => {
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")])];
    await render({ events: [event("g1")] });
    await advance();
    await act(async () => choiceButtons()[0].click());
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")]), player("p1", "A")];
    backend.slowRead = true;
    backend.parse = () => ({ cells: [n("coat"), n("shadows"), n("whistle")], tail_tentative: false });
    await render({
      events: [event("g1"), event("p1")],
      generating: { id: "", kind: "narration" },
      streamText: "coat\n\nshadows\n\nwhistle",
    });
    expect(text()).toBe("coat");
  });
});
