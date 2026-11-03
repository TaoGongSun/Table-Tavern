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
import type { VnAdvance } from "../play/scene/useSceneActions";

const backend = vi.hoisted(() => ({
  cells: [] as VnEventCells[],
  parse: (_text: string): { cells: VnCell[]; tail_tentative: boolean } | null => null,
  reroll: (): Promise<string[]> => Promise.resolve([]),
  calls: [] as string[],
  /** 讓整幕重抓晚一拍回來，跟串流解析落在同一批更新 */
  slowRead: false,
  endings: null as unknown,
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
    if (command === "vn_backgrounds_status") return [];
    if (command === "vn_endings") return backend.endings;
    if (command === "vn_new_playthrough") return `new:${String(args.nameTemplate)}`;
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

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

let flagHints = true;
let sceneNo = 0;
let onAdvanceScene = vi.fn(async (_chapterEnd: boolean): Promise<VnAdvance> => ({ stage: "entered" }));
let onReenter = vi.fn(async () => true);
const onReplayChapter = vi.fn(async (_chapter: number) => {});
const onEnterTable = vi.fn((_id: string) => {});

async function render(patch: Partial<ChatController> = {}) {
  chat = { ...chat, ...patch };
  await act(async () => {
    root.render(
      <VnPlayer
        worldId="w1"
        scene={sceneNo}
        tableName="Harbor"
        chat={chat}
        images={{}}
        error=""
        busy={chat.busy}
        onError={() => {}}
        onAdvanceScene={onAdvanceScene}
        onReenter={onReenter}
        onShowChat={() => {}}
        onGoLobby={() => {}}
        onReplayChapter={onReplayChapter}
        onEnterTable={onEnterTable}
        flagHints={flagHints}
        onFlagHints={() => {}}
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
  onAdvanceScene = vi.fn(async () => ({ stage: "entered" }) as VnAdvance);
  onReenter = vi.fn(async () => true);
  sceneNo = 0;
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
  it("shows an event's flag hints on its first cell only, and not when turned off", async () => {
    backend.cells = [
      {
        ...gm("g1", [n("first"), n("second")]),
        hints: [
          { kind: "flag", label: "Trust", mark: "+1" },
          { kind: "reveal", title: "Princess" },
        ],
      },
    ];
    const hints = () => [...host.querySelectorAll(".vn-flag-hints li")].map((li) => li.textContent);
    flagHints = false;
    await render({ events: [event("g1")] });
    expect(text()).toBe("first");
    expect(hints()).toEqual([]);
    flagHints = true;
    await render({ events: [event("g1")] });
    expect(hints()).toEqual(["◆ Trust +1", `◆ ${t("vnFlagReveal", { title: "Princess" })}`]);
    await advance();
    expect(hints()).toEqual([]);
  });

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
      cells: [{ type: "bg", location: "Track", time: null, slug: "s", period: null }, n("first"), n("second"), n("third")],
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

  it("advancing onto the chapter cell ends the chapter once, then opens the next chapter's first beat", async () => {
    backend.cells = [{ ...gm("g1", [n("we arrive"), { type: "chapter" }]), chapter: 2 }];
    let entered = (_: VnAdvance) => {};
    onAdvanceScene = vi.fn(() => new Promise<VnAdvance>((resolve) => (entered = resolve)));
    await render({ events: [event("g1")] });
    expect(text()).toBe("we arrive");
    expect(onAdvanceScene).not.toHaveBeenCalled();
    await advance();
    expect(onAdvanceScene).toHaveBeenCalledWith(true);
    expect(host.querySelector(".vn-chapter-heading")?.textContent).toBe(t("vnChapterDone", { n: 2 }));
    await advance();
    expect(onAdvanceScene).toHaveBeenCalledTimes(1);
    await act(async () => entered({ stage: "entered" }));
    await settle();
    // 首拍等播放器換到新幕才開：同一輪的聊天控制器還綁著舊幕
    expect(chat.gmAdvance).not.toHaveBeenCalled();
    sceneNo = 1;
    backend.cells = [{ ...gm("s1", [n("recap")]), chapter: 3 }];
    await render({ events: [event("s1")] });
    expect(chat.gmAdvance).toHaveBeenCalledTimes(1);
    expect(host.querySelector(".vn-chapter-card")).toBeNull();
  });

  it("entering or skipping onto a chapter cell waits for the next-chapter button", async () => {
    backend.cells = [gm("g0", [n("a")]), gm("g1", [{ type: "chapter" }])];
    await render({ events: [event("g0"), event("g1")] });
    expect(onAdvanceScene).not.toHaveBeenCalled();
    const next = [...host.querySelectorAll<HTMLButtonElement>(".vn-continue")];
    expect(next.map((b) => b.textContent)).toEqual([t("vnNextChapter")]);
    await act(async () => next[0].click());
    await settle();
    expect(onAdvanceScene).toHaveBeenCalledWith(true);
  });

  it("a failed advance retries the advance; a failed re-entry retries only the re-entry", async () => {
    backend.cells = [gm("g1", [n("end"), { type: "chapter" }])];
    onAdvanceScene = vi
      .fn<(chapterEnd: boolean) => Promise<VnAdvance>>()
      .mockResolvedValueOnce({ stage: "failed", message: "boom" })
      .mockResolvedValueOnce({ stage: "committed" });
    onReenter = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    await render({ events: [event("g1")] });
    await advance();
    expect(host.querySelector(".vn-chapter-card [role=alert]")?.textContent).toBe("boom");
    await act(async () => button(t("vnChapterRetry"))!.click());
    await settle();
    expect(onAdvanceScene).toHaveBeenCalledTimes(2);
    expect(host.querySelector(".vn-chapter-card")?.textContent).toContain(t("vnReenterFailed"));
    await act(async () => button(t("vnChapterRetry"))!.click());
    await settle();
    await act(async () => button(t("vnChapterRetry"))!.click());
    await settle();
    expect(onAdvanceScene).toHaveBeenCalledTimes(2);
    expect(onReenter).toHaveBeenCalledTimes(2);
    sceneNo = 1;
    await render();
    expect(chat.gmAdvance).toHaveBeenCalledTimes(1);
  });

  it("the capacity error's advance button only changes the act", async () => {
    backend.cells = [gm("g1", [n("hello")])];
    await act(async () => {
      root.render(
        <VnPlayer
          worldId="w1"
          scene={0}
          tableName="Harbor"
          chat={{ ...chat, events: [event("g1")] }}
          images={{}}
          error={'TTMSG:{"code":"scene_capacity_full"}'}
          busy={false}
          onError={() => {}}
          onAdvanceScene={onAdvanceScene}
          onReenter={onReenter}
          onShowChat={() => {}}
          onGoLobby={() => {}}
          onReplayChapter={onReplayChapter}
          onEnterTable={onEnterTable}
          flagHints
          onFlagHints={() => {}}
        />,
      );
    });
    await settle();
    await act(async () => button(t("sceneAdvance"))!.click());
    await settle();
    expect(onAdvanceScene).toHaveBeenCalledWith(false);
  });

  it("a new scene (replay, fork, advance) starts at its entry, not at the old scene's end", async () => {
    backend.cells = [gm("g1", [n("one"), n("two")])];
    await render({ events: [event("g1")] });
    await advance();
    await advance();
    expect(text()).toBe("two");
    backend.cells = [gm("o1", [n("opening"), n("more")])];
    sceneNo = 1;
    await render({ events: [event("o1")] });
    expect(text().length).toBeLessThanOrEqual("opening".length);
    expect("opening".startsWith(text())).toBe(true);
  });

  it("reaching the ending cell shows the end screen with the ending title and reached count", async () => {
    backend.cells = [gm("g1", [n("farewell"), { type: "ending", id: "true_end" }])];
    backend.endings = {
      endings: [
        { id: "true_end", title: "Dawn", reached: true },
        { id: "default", title: "", reached: false },
      ],
      current: "true_end",
    };
    await render({ events: [event("g1")] });
    expect(host.querySelector(".vn-ending-screen")).toBeNull();
    await advance();
    expect(host.querySelector(".vn-ending-title")?.textContent).toBe("Dawn");
    expect(host.querySelector(".vn-ending-screen")?.textContent).toContain(
      t("vnEndingsReached", { x: 1, y: 2 }),
    );
    expect(button(t("vnNextChapter"))!.disabled).toBe(true);
    expect(button(t("vnContinue"))).toBeUndefined();

    // 結局一覽：未達成顯示 ???
    await act(async () => button(t("vnEndingsList"))!.click());
    await settle();
    const list = [...host.querySelectorAll(".vn-story-panel li")].map((item) => item.textContent);
    expect(list).toEqual(["Dawn", "???"]);
    await act(async () => button(t("vnChapterClose"))!.click());
    // 從某一章重玩：第 1 章到目前這章
    await act(async () => button(t("vnReplayChapter"))!.click());
    await act(async () => button(t("vnChapterStart", { n: 1 }))!.click());
    expect(onReplayChapter).toHaveBeenCalledWith(1);
    // 開新周目：確認後帶在地化樣板建桌，進新桌
    await act(async () => button(t("vnNewPlaythrough"))!.click());
    await settle();
    expect(onEnterTable).toHaveBeenCalledWith(
      `new:${t("vnPlaythroughName", { name: "{name}", n: "{n}" })}`,
    );
  });
});
