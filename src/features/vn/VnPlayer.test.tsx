// @vitest-environment happy-dom
// VN 播放器元件測試：假後端回格，假聊天控制器只記呼叫；打字機以 reduced-motion 直接全出。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type {
  TranscriptEvent,
  SceneLabel,
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
  branches: [] as unknown[],
  /** 分支查詢：等這個 promise 才回；fail＝回錯 */
  branchesGate: null as Promise<void> | null,
  branchesFail: false,
  progress: null as unknown,
  loaded: null as unknown,
  args: {} as Record<string, Record<string, unknown>>,
  chapterHead: 0,
}));

/** 照後端共用選項邊界標選項點：其後第一則玩家句＝已選，之前又出選項＝被取代（不標） */
function marked(events: VnEventCells[]): VnEventCells[] {
  return events.map((event, index) => {
    if (!event.cells.some((cell) => cell.type === "choices")) return event;
    for (const later of events.slice(index + 1)) {
      if (later.kind === "player") {
        const first = later.cells[0];
        return { ...event, choice: { answered: first?.type === "narration" ? first.text : "" } };
      }
      if (later.cells.some((cell) => cell.type === "choices")) return event;
    }
    return { ...event, choice: { answered: null } };
  });
}
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown>) => {
    backend.calls.push(command);
    backend.args[command] = args;
    if (command === "read_vn_cells") {
      if (backend.slowRead) await new Promise((resolve) => setTimeout(resolve, 0));
      return marked(structuredClone(backend.cells));
    }
    if (command === "parse_vn_text") return backend.parse(args.text as string);
    if (command === "vn_reroll_choices") return backend.reroll();
    if (command === "vn_backgrounds_status") return [];
    if (command === "vn_endings") return backend.endings;
    if (command === "vn_progress_read") return backend.progress;
    if (command === "vn_save_load") return backend.loaded;
    if (command === "vn_choice_branches") {
      if (backend.branchesGate) await backend.branchesGate;
      if (backend.branchesFail) throw new Error("boom");
      return backend.branches;
    }
    if (command === "vn_new_playthrough") return `new:${String(args.nameTemplate)}`;
    if (command === "vn_replay_goto") return args.scene;
    if (command === "vn_replay_chapter") return backend.chapterHead;
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn(async () => true) }));

import { VnPlayer } from "./VnPlayer";
import { cellHash, type AutoWait, type TextSpeed } from "./vn-cells";

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
let textSpeed: TextSpeed = "normal";
const onTextSpeed = vi.fn((_speed: TextSpeed) => {});
const onAutoWait = vi.fn((_wait: AutoWait) => {});
let replayTable = false;
let labels: Record<string, SceneLabel> = {};
let titleOpen = false;
const onTitle = vi.fn((open: boolean) => {
  titleOpen = open;
});
let sceneNo = 0;
let onAdvanceScene = vi.fn(async (_chapterEnd: boolean): Promise<VnAdvance> => ({ stage: "entered" }));
let onReenter = vi.fn(async () => true);
const onReplayChapter = vi.fn(async (_chapter: number) => {});
const onEnterTable = vi.fn((_id: string) => {});
let onBranch = vi.fn(async (run: () => Promise<unknown>): Promise<VnAdvance> => {
  await run();
  return { stage: "entered" };
});

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
        onBranch={onBranch}
        onEnterTable={onEnterTable}
        flagHints={flagHints}
        onFlagHints={() => {}}
        textSpeed={textSpeed}
        autoWait="normal"
        onTextSpeed={onTextSpeed}
        onAutoWait={onAutoWait}
        title={titleOpen}
        onTitle={onTitle}
        replay={replayTable}
        sceneLabels={labels}
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
/** 合成按鍵：預設從 body 冒泡到 window；回傳事件看有沒有被 preventDefault */
const press = async (key: string, init: KeyboardEventInit = {}, target: EventTarget = document.body, type = "keydown") => {
  const event = new KeyboardEvent(type, { key, bubbles: true, cancelable: true, ...init });
  await act(async () => void target.dispatchEvent(event));
  await settle();
  return event;
};
const menuItems = () => [...host.querySelectorAll(".vn-game-menu button")].map((b) => b.textContent);

beforeEach(() => {
  backend.cells = [];
  backend.parse = () => null;
  backend.calls = [];
  backend.slowRead = false;
  onAdvanceScene = vi.fn(async () => ({ stage: "entered" }) as VnAdvance);
  onReenter = vi.fn(async () => true);
  onBranch = vi.fn(async (run: () => Promise<unknown>): Promise<VnAdvance> => {
    await run();
    return { stage: "entered" };
  });
  backend.branches = [];
  backend.branchesGate = null;
  backend.branchesFail = false;
  backend.progress = null;
  backend.loaded = null;
  backend.args = {};
  titleOpen = false;
  textSpeed = "normal";
  onTitle.mockClear();
  sceneNo = 0;
  replayTable = false;
  labels = {};
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

  it("enters an unread scene at its first cell and a choice goes to submitText exactly once", async () => {
    backend.cells = [gm("g0", [n("old")]), gm("g1", [n("hello"), choices("Go left", "Go right")])];
    let finish = () => {};
    chat.submitText = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    await render({ events: [event("g0"), event("g1")] });
    expect(text()).toBe("old");
    await advance();
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

  it("a choice already answered by the player shows only the past panel, and continue answers the player", async () => {
    backend.cells = [gm("g1", [n("hello"), choices("A", "B")]), player("p1", "A")];
    await render({ events: [event("g1"), event("p1")] });
    await advance();
    expect(host.querySelector(".vn-choices:not(.vn-choices-past)")).toBeNull();
    expect(host.querySelector(".vn-choice-current")?.textContent).toContain("A");
    // 「繼續」在對話框右下角取代 ▼
    expect(button(t("vnContinue"))!.closest(".vn-dialogue")).not.toBeNull();
    expect(host.querySelector(".vn-ctc")).toBeNull();
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

  it("entering or skipping onto a chapter cell waits for a click, with only the usual hint", async () => {
    backend.cells = [gm("g0", [n("a")]), gm("g1", [{ type: "chapter" }])];
    backend.progress = { version: 1, read: [], positions: { "0": "g1#0" }, saves: [] };
    await render({ events: [event("g0"), event("g1")] });
    expect(onAdvanceScene).not.toHaveBeenCalled();
    expect(button(t("vnNextChapter"))).toBeUndefined();
    expect(host.querySelector(".vn-dialogue .vn-ctc")).not.toBeNull();
    await advance();
    expect(onAdvanceScene).toHaveBeenCalledWith(true);
  });

  it("the chapter card holds typing behind it and never flashes the final beat before the new act loads", async () => {
    window.matchMedia = (() => ({ matches: false })) as unknown as typeof window.matchMedia;
    backend.cells = [{ ...gm("g1", [n("we"), { type: "chapter" }]), chapter: 2 }];
    let entered = (_: VnAdvance) => {};
    onAdvanceScene = vi.fn(() => new Promise<VnAdvance>((resolve) => (entered = resolve)));
    chat.gmAdvance = vi.fn(() => new Promise<void>(() => {}));
    await render({ events: [event("g1")] });
    await advance();
    await advance();
    expect(onAdvanceScene).toHaveBeenCalledWith(true);
    await act(async () => entered({ stage: "entered" }));
    await settle();
    // 還在舊幕：cells 的章號等於結束的章，不能判成尾聲
    expect(host.textContent).not.toContain(t("vnFinalBeat"));
    sceneNo = 1;
    backend.cells = [{ ...gm("s1", [n("a long first line of the new act")]), chapter: 3 }];
    await render({ events: [event("s1")] });
    expect(host.querySelector(".vn-chapter-heading")?.textContent).toBe(t("vnChapterStart", { n: 3 }));
    await act(async () => new Promise((resolve) => setTimeout(resolve, 200)));
    expect(text()).toBe("");
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
          onBranch={onBranch}
          onEnterTable={onEnterTable}
          flagHints
          onFlagHints={() => {}}
          textSpeed="normal"
          autoWait="normal"
          onTextSpeed={() => {}}
          onAutoWait={() => {}}
          title={false}
          onTitle={() => {}}
          replay={false}
          sceneLabels={{}}
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
      t("vnEndingsReached", { x: 1, y: 1 }),
    );
    expect(button(t("vnContinue"))).toBeUndefined();
    // 完結畫面上 Esc 仍開得了遊戲選單（切聊天視圖收回結局），下一章停用
    await press("Escape");
    expect(button(t("vnNextChapter"))!.disabled).toBe(true);
    await press("Escape");
    expect(host.querySelector(".vn-game-menu")).toBeNull();

    // 結局一覽：未達成顯示 ???
    await act(async () => button(t("vnEndingsList"))!.click());
    await settle();
    const list = [...host.querySelectorAll(".vn-story-panel li")].map((item) => item.textContent);
    expect(list).toEqual(["Dawn"]);
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

  it("rolls back cell by cell for display only, and stops at the scene's first cell", async () => {
    backend.cells = [gm("g1", [n("one"), n("two")])];
    await render({ events: [event("g1")] });
    await advance();
    expect(text()).toBe("two");
    await act(async () => button(t("vnBack"))!.click());
    expect(text()).toBe("one");
    expect(button(t("vnBack"))!.disabled).toBe(true);
    expect(backend.calls.filter((call) => call.startsWith("vn_fork") || call.startsWith("vn_switch"))).toEqual([]);
  });

  it("rolling back from a choice panel skips the line the dialogue box already shows", async () => {
    backend.cells = [gm("g1", [n("one"), n("two"), choices("A", "B")])];
    await render({ events: [event("g1")] });
    await advance();
    await advance();
    expect(choiceButtons()).toHaveLength(2);
    expect(text()).toBe("two");
    await act(async () => button(t("vnBack"))!.click());
    expect(text()).toBe("one");
  });

  it("an open choice followed by a character line shows its panel after the line", async () => {
    backend.cells = [
      gm("g1", [n("q"), choices("A", "B")]),
      { event_id: "c1", kind: "dialogue", cells: [n("who?")] },
    ];
    await render({ events: [event("g1"), event("c1")] });
    expect(text()).toBe("q");
    expect(choiceButtons()).toEqual([]);
    await advance();
    expect(text()).toBe("who?");
    expect(choiceButtons().map((b) => b.textContent)).toEqual(["A", "B"]);
    expect(button(t("vnReroll"))).toBeUndefined();
  });

  describe("past choice panel", () => {
    const past = () => [
      gm("g1", [n("q"), choices("A", "B", "C")]),
      player("p1", "A"),
      gm("g2", [n("after A")]),
    ];
    async function openPast() {
      backend.cells = past();
      backend.branches = [{ option: "B", tip: 3, ended: true }];
      backend.progress = { version: 1, read: [], positions: { "0": "g2#0" }, saves: [] };
      await render({ events: [event("g1"), event("p1"), event("g2")] });
      await act(async () => button(t("vnBack"))!.click());
      await settle();
    }
    const panel = () => [...host.querySelectorAll<HTMLButtonElement>(".vn-choices-past .vn-choice")];

    it("marks the current route and played branches; the current route reads on", async () => {
      await openPast();
      expect(panel().map((b) => b.textContent)).toEqual([
        `A${t("vnChoiceCurrent")}`,
        `B${t("vnChoicePlayed")} · ${t("vnChoiceEnded")}`,
        "C",
      ]);
      await act(async () => panel()[0].click());
      expect(text()).toBe("after A");
      expect(onBranch).not.toHaveBeenCalled();
    });

    it("other routes stay locked until this choice point's branches are loaded", async () => {
      let release = () => {};
      backend.branchesGate = new Promise<void>((resolve) => (release = resolve));
      await openPast();
      expect(panel().map((b) => b.disabled)).toEqual([false, true, true]);
      await act(async () => panel()[1].click());
      expect(onBranch).not.toHaveBeenCalled();
      await act(async () => release());
      await settle();
      expect(panel().map((b) => b.disabled)).toEqual([false, false, false]);
      expect(panel()[1].textContent).toContain(t("vnChoicePlayed"));
    });

    it("a failed branch lookup keeps other routes locked and offers a retry", async () => {
      backend.branchesFail = true;
      await openPast();
      expect(panel().map((b) => b.disabled)).toEqual([false, true, true]);
      expect(host.querySelector(".vn-choices-past .vn-choice-error")?.textContent).toContain(t("vnChoiceBranchesFailed"));
      backend.branchesFail = false;
      await act(async () => button(t("vnBranchReenterRetry"))!.click());
      await settle();
      expect(panel().map((b) => b.disabled)).toEqual([false, false, false]);
    });

    it("a played branch switches to its tip", async () => {
      await openPast();
      await act(async () => panel()[1].click());
      await settle();
      expect(onBranch).toHaveBeenCalledTimes(1);
      expect(backend.calls).toContain("vn_switch_branch");
      expect(chat.submitText).not.toHaveBeenCalled();
    });

    it("an unplayed option forks, then sends only after the player is on the new scene", async () => {
      await openPast();
      await act(async () => panel()[2].click());
      await settle();
      expect(backend.calls).toContain("vn_fork_choice");
      expect(chat.submitText).not.toHaveBeenCalled();
      backend.cells = [gm("g9", [n("q"), choices("A", "B", "C")])];
      sceneNo = 4;
      await render({ events: [event("g9")] });
      expect(chat.submitText).toHaveBeenCalledTimes(1);
      expect(chat.submitText).toHaveBeenCalledWith("C");
    });

    it("a committed fork whose re-entry failed retries only the re-entry", async () => {
      onBranch = vi.fn(async (run: () => Promise<unknown>): Promise<VnAdvance> => {
        await run();
        return { stage: "committed" };
      });
      await openPast();
      await act(async () => panel()[2].click());
      await settle();
      expect(host.querySelector(".vn-choice-error")?.textContent).toContain(t("vnBranchReenterFailed"));
      await act(async () => button(t("vnBranchReenterRetry"))!.click());
      await settle();
      expect(onReenter).toHaveBeenCalledTimes(1);
      expect(onBranch).toHaveBeenCalledTimes(1);
      expect(chat.submitText).not.toHaveBeenCalled();
      backend.cells = [gm("g9", [n("q"), choices("A", "B", "C")])];
      sceneNo = 4;
      await render({ events: [event("g9")] });
      expect(chat.submitText).toHaveBeenCalledWith("C");
    });
  });

  describe("reading progress, saves and the title screen", () => {
    const progress = (read: VnCell[], positions: Record<string, string> = {}) => ({
      version: 1,
      read: read.map(cellHash),
      positions,
      saves: [],
    });

    it("skip runs over read cells and stops before the first unread one", async () => {
      backend.cells = [gm("g1", [n("a"), n("b"), n("c")])];
      backend.progress = progress([n("a"), n("b")]);
      await render({ events: [event("g1")] });
      expect(text()).toBe("a");
      await act(async () => button(t("vnSkip"))!.click());
      await settle();
      await act(async () => new Promise((resolve) => setTimeout(resolve, 200)));
      await settle();
      expect(text()).toBe("b");
      expect(button(t("vnSkip"))!.getAttribute("aria-pressed")).toBe("false");
    });

    it("turning skip on while an unread cell types stops skip and keeps typing it", async () => {
      window.matchMedia = (() => ({ matches: false })) as unknown as typeof window.matchMedia;
      backend.cells = [gm("g1", [n("a long unread line"), n("b")])];
      backend.progress = progress([]);
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnSkip"))!.click());
      await settle();
      expect(button(t("vnSkip"))!.getAttribute("aria-pressed")).toBe("false");
      expect(text().length).toBeLessThan("a long unread line".length);
    });

    it("the instant text speed shows the whole line at once and the settings panel reports picks", async () => {
      window.matchMedia = (() => ({ matches: false })) as unknown as typeof window.matchMedia;
      backend.cells = [gm("g1", [n("a long unread line"), n("b")])];
      textSpeed = "instant";
      titleOpen = true;
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnTitleSettings"))!.click());
      const [speed, wait] = [...host.querySelectorAll<HTMLSelectElement>(".vn-setting select")];
      expect(speed.value).toBe("instant");
      await act(async () => {
        speed.value = "fast";
        speed.dispatchEvent(new Event("change", { bubbles: true }));
        wait.value = "long";
        wait.dispatchEvent(new Event("change", { bubbles: true }));
      });
      expect(onTextSpeed).toHaveBeenLastCalledWith("fast");
      expect(onAutoWait).toHaveBeenLastCalledWith("long");
      titleOpen = false;
      await render({ events: [event("g1")] });
      expect(text()).toBe("a long unread line");
    });

    it("enters at the last read position of the scene", async () => {
      backend.cells = [gm("g0", [n("old")]), gm("g1", [n("a"), n("b")])];
      backend.progress = progress([], { "0": "g0#0" });
      await render({ events: [event("g0"), event("g1")] });
      expect(text()).toBe("old");
    });

    it("saves the current cell and loads a save through the branch flow", async () => {
      backend.cells = [gm("g1", [n("a"), n("b")])];
      backend.progress = { ...progress([]), saves: [{ id: "s1", name: "mine", scene: 0, event_id: "g1", cell: 1, chapter: 1, ts: "t", preview: "b" }] };
      backend.loaded = { scene: 0, event_id: "g1", cell: 1 };
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnSave"))!.click());
      await act(async () => button(t("vnSaveCreate"))!.click());
      await settle();
      expect(backend.args["vn_save_create"]).toMatchObject({ eventId: "g1", cell: 0 });
      await act(async () => button(t("vnSaveLoad"))!.click());
      await settle();
      expect(onBranch).toHaveBeenCalledTimes(1);
      expect(backend.calls).toContain("vn_save_load");
      expect(text()).toBe("b");
    });

    it("the title screen continues, starts an unplayed table without replaying, and the ending returns to it", async () => {
      onReplayChapter.mockClear();
      backend.cells = [gm("g1", [n("hello")])];
      titleOpen = true;
      await render({ events: [event("g1")] });
      expect(host.querySelector(".vn-title-screen")).not.toBeNull();
      await act(async () => button(t("vnTitleStart"))!.click());
      await settle();
      expect(onTitle).toHaveBeenLastCalledWith(false);
      expect(onReplayChapter).not.toHaveBeenCalled();
      await act(async () => button(t("vnMenu"))!.click());
      await act(async () => button(t("vnBackToTitle"))!.click());
      expect(onTitle).toHaveBeenLastCalledWith(true);
    });

    const wait = (ms: number) => act(async () => new Promise((resolve) => setTimeout(resolve, ms)));

    it("the title screen holds reading, auto and the chapter end until it closes", async () => {
      backend.cells = [gm("g1", [n("a"), { type: "chapter" }])];
      backend.progress = progress([]);
      titleOpen = true;
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnAuto"))!.click());
      await wait(1600);
      expect(text()).toBe("a");
      expect(onAdvanceScene).not.toHaveBeenCalled();
      expect(backend.args["vn_progress_write"]).toBeUndefined();
      titleOpen = false;
      await render();
      await wait(1600);
      expect(backend.args["vn_progress_write"]?.readAdd).toContain(cellHash(n("a")));
      expect(onAdvanceScene).toHaveBeenCalledWith(true);
    }, 10000);

    it("skip waits behind the title and resumes when it closes", async () => {
      backend.cells = [gm("g1", [n("a"), n("b"), n("c")])];
      backend.progress = progress([n("a"), n("b"), n("c")]);
      titleOpen = true;
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnSkip"))!.click());
      await wait(300);
      expect(text()).toBe("a");
      expect(button(t("vnSkip"))!.getAttribute("aria-pressed")).toBe("true");
      titleOpen = false;
      await render();
      await wait(200);
      await wait(200);
      expect(text()).toBe("c");
    });

    it("an open panel or the log holds auto until it closes", async () => {
      backend.cells = [gm("g1", [n("a"), n("b"), n("c")])];
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnAuto"))!.click());
      await act(async () => button(t("vnSave"))!.click());
      await wait(1400);
      expect(text()).toBe("a");
      await act(async () => button(t("vnChapterClose"))!.click());
      await wait(1400);
      expect(text()).toBe("b");
      await act(async () => button(t("vnLog"))!.click());
      await wait(1400);
      expect(text()).toBe("b");
    }, 10000);
  });

  describe("replay table (imported work)", () => {
    const label = (patch: Partial<SceneLabel>): SceneLabel => ({ base: 0, version: 1, parent: null, ...patch });
    const wait = (ms: number) => act(async () => new Promise((resolve) => setTimeout(resolve, ms)));
    beforeEach(() => {
      replayTable = true;
    });

    it("drops generation entries: no next chapter, chat view, new playthrough or export", async () => {
      backend.cells = [gm("g1", [n("farewell"), { type: "ending", id: "true_end" }])];
      backend.endings = { endings: [{ id: "true_end", title: "Dawn", reached: true }], current: "true_end" };
      await render({ events: [event("g1")] });
      expect(button(t("vnNextChapter"))).toBeUndefined();
      expect(button(t("vnShowChat"))).toBeUndefined();
      await act(async () => button(t("vnMenu"))!.click());
      expect(menuItems()).toEqual([
        t("vnBackToTitle"),
        t("vnSaves"),
        t("vnTitleSettings"),
        t("vnEndingsList"),
        t("vnGallery"),
        t("vnWorkChapterSelect"),
        t("vnHideUi"),
        t("backToLobby"),
      ]);
      await act(async () => button(t("vnChapterClose"))!.click());
      await advance();
      expect(host.querySelector(".vn-ending-title")?.textContent).toBe("Dawn");
      expect(button(t("vnNewPlaythrough"))).toBeUndefined();
      expect(button(t("vnWorkExport"))).toBeUndefined();
      // 章節選擇：章頭就是目前幕＝直接跳回幕首，不分岔
      await act(async () => button(t("vnWorkChapterSelect"))!.click());
      await act(async () => button(t("vnChapterStart", { n: 1 }))!.click());
      await settle();
      expect(backend.args["vn_replay_chapter"]).toMatchObject({ chapter: 1 });
      expect(onReplayChapter).not.toHaveBeenCalled();
      expect(text()).toBe("farewell");
    });

    it("a chapter end goes to the child scene's first cell without generating", async () => {
      backend.cells = [{ ...gm("g1", [n("we arrive"), { type: "chapter" }]), chapter: 1 }];
      labels = { "1": label({ parent: 0 }), "2": label({ parent: 0, forked: true, source: 0 }) };
      await render({ events: [event("g1")] });
      await advance();
      expect(host.querySelector(".vn-chapter-heading")?.textContent).toBe(t("vnChapterDone", { n: 1 }));
      await wait(1100);
      await settle();
      expect(onAdvanceScene).not.toHaveBeenCalled();
      expect(backend.args["vn_replay_goto"]).toEqual({ worldId: "w1", scene: 1 });
      sceneNo = 1;
      backend.cells = [{ ...gm("s1", [n("first"), n("second")]), chapter: 2 }];
      backend.progress = { version: 1, read: [], positions: { "1": "s1#1" }, saves: [] };
      await render({ events: [event("s1")] });
      expect(host.querySelector(".vn-chapter-heading")?.textContent).toBe(t("vnChapterStart", { n: 2 }));
      await wait(1300);
      await settle();
      expect(host.querySelector(".vn-chapter-card")).toBeNull();
      expect(chat.gmAdvance).not.toHaveBeenCalled();
      expect(text()).toBe("first");
    });

    it("a dead end shows the unfinished path, and going back lands on the previous scene's last choice", async () => {
      backend.cells = [gm("s1", [n("alone"), { type: "chapter" }])];
      labels = { "1": label({ parent: 0 }) };
      sceneNo = 1;
      await render({ events: [event("s1")] });
      await advance();
      expect(host.querySelector(".vn-unfinished")?.textContent).toContain(t("vnWorkUnfinished"));
      expect(onBranch).not.toHaveBeenCalled();
      await act(async () => button(t("vnWorkBackToChoice"))!.click());
      await settle();
      expect(backend.args["vn_replay_goto"]).toEqual({ worldId: "w1", scene: 0 });
      sceneNo = 0;
      backend.cells = [gm("g1", [n("q"), choices("A", "B")]), player("p1", "A"), gm("g2", [n("after")])];
      await render({ events: [event("g1"), event("p1"), event("g2")] });
      expect(host.querySelector(".vn-choices-past")).not.toBeNull();
      // 重播桌也標目前路線
      expect(host.querySelector(".vn-choice-current")?.textContent).toContain("A");
    });

    it("the last act's chapter cell is passed over: one click from the line reaches the ending", async () => {
      backend.cells = [gm("g1", [n("farewell"), { type: "chapter" }, { type: "ending", id: "true_end" }])];
      backend.endings = { endings: [{ id: "true_end", title: "Dawn", reached: true }], current: "true_end" };
      await render({ events: [event("g1")] });
      await advance();
      expect(host.querySelector(".vn-ending-title")?.textContent).toBe("Dawn");
    });

    it("an answered choice that ends the scene shows the unfinished note inside the same panel", async () => {
      backend.cells = [gm("g1", [n("q"), choices("A", "B")]), player("p1", "A")];
      await render({ events: [event("g1"), event("p1")] });
      await advance();
      expect(host.querySelectorAll(".vn-choices")).toHaveLength(1);
      expect(host.querySelector(".vn-choices-past")?.textContent).toContain(t("vnWorkUnfinished"));
      expect(button(t("vnWorkBackToChoice"))).toBeUndefined();
    });

    it("an open choice jumps to the author's branch and greys out the paths never taken", async () => {
      backend.cells = [gm("g1", [n("q"), choices("A", "B", "C")])];
      backend.branches = [{ option: "B", tip: 2, ended: true, scene: 2, answer_event_id: "p9" }];
      await render({ events: [event("g1")] });
      await advance();
      const panel = choiceButtons();
      expect(panel.map((b) => b.disabled)).toEqual([true, false, true]);
      expect(panel[0].textContent).toContain(t("vnWorkNotWalked"));
      expect(button(t("vnReroll"))).toBeUndefined();
      await act(async () => panel[1].click());
      await settle();
      expect(backend.args["vn_replay_goto"]).toEqual({ worldId: "w1", scene: 2 });
      expect(chat.submitText).not.toHaveBeenCalled();
      sceneNo = 2;
      backend.cells = [gm("g1", [n("q"), choices("A", "B", "C")]), player("p9", "B"), gm("g9", [n("reply")])];
      await render({ events: [event("g1"), event("p9"), event("g9")] });
      expect(text()).toBe("reply");
    });

    it("an open choice nobody took says the path is unfinished and can go back a scene", async () => {
      backend.cells = [gm("g1", [n("q"), choices("A", "B")])];
      labels = { "1": label({ parent: 0 }) };
      sceneNo = 1;
      await render({ events: [event("g1")] });
      await advance();
      expect(choiceButtons().slice(0, 2).every((b) => b.disabled)).toBe(true);
      expect(host.querySelector(".vn-choices-past")?.textContent).toContain(t("vnWorkUnfinished"));
      await act(async () => button(t("vnWorkBackToChoice"))!.click());
      await settle();
      expect(backend.args["vn_replay_goto"]).toEqual({ worldId: "w1", scene: 0 });
    });

    it("the title's start goes back to scene 0's first cell without asking", async () => {
      backend.cells = [gm("s3", [n("late"), n("later")])];
      sceneNo = 3;
      titleOpen = true;
      await render({ events: [event("s3")] });
      await act(async () => button(t("vnTitleStart"))!.click());
      await settle();
      expect(backend.args["vn_replay_goto"]).toEqual({ worldId: "w1", scene: 0 });
      expect(onReplayChapter).not.toHaveBeenCalled();
      sceneNo = 0;
      backend.cells = [gm("o1", [n("opening"), n("more")])];
      backend.progress = { version: 1, read: [], positions: { "0": "o1#1" }, saves: [] };
      await render({ events: [event("o1")] });
      expect(text()).toBe("opening");
    });
  });
  describe("Ren'Py-style input", () => {
    const three = () => [gm("g1", [n("a"), n("b"), n("c")])];
    const pressed = (label: string) => button(label)!.getAttribute("aria-pressed");
    const stage = () => host.querySelector<HTMLElement>(".vn-stage")!;
    const click = async (el: Element, type = "click") => {
      const event = new MouseEvent(type, { bubbles: true, cancelable: true });
      await act(async () => void el.dispatchEvent(event));
      await settle();
      return event;
    };

    it("maps advance, back, auto, hide and the menu keys", async () => {
      backend.cells = three();
      await render({ events: [event("g1")] });
      await press(" ");
      expect(text()).toBe("b");
      await press("ArrowLeft");
      expect(text()).toBe("a");
      await press("Enter");
      await press("ArrowRight");
      expect(text()).toBe("c");
      await press("Backspace");
      expect(text()).toBe("b");
      await press("a");
      expect(pressed(t("vnAuto"))).toBe("true");
      await press("a");
      await press("h");
      expect(host.querySelector(".vn-ui-hidden")).not.toBeNull();
      await press(" ");
      expect(host.querySelector(".vn-ui-hidden")).toBeNull();
      expect(text()).toBe("b");
      await press("Escape");
      expect(host.querySelector(".vn-game-menu")).not.toBeNull();
      await press(" ");
      expect(text()).toBe("b");
      await press("Escape");
      expect(host.querySelector(".vn-game-menu")).toBeNull();
      // app 層的 modal 對話窗開著：按鍵不進播放器
      const modal = document.body.appendChild(document.createElement("dialog"));
      modal.setAttribute("open", "");
      await press(" ");
      await press("Escape");
      modal.remove();
      expect(text()).toBe("b");
      expect(host.querySelector(".vn-game-menu")).toBeNull();
    });

    it("the focused dialogue box consumes Enter and Space so the native click cannot advance again; other buttons keep theirs", async () => {
      backend.cells = three();
      await render({ events: [event("g1")] });
      const dialogue = host.querySelector(".vn-dialogue")!;
      expect((await press("Enter", {}, dialogue)).defaultPrevented).toBe(true);
      expect(text()).toBe("b");
      expect((await press(" ", {}, button(t("vnAuto"))!)).defaultPrevented).toBe(false);
      expect(text()).toBe("b");
    });

    it("a focused continue button cannot be pressed natively behind the open menu", async () => {
      backend.cells = [gm("g1", [n("hello")])];
      await render({ events: [event("g1")] });
      const cont = button(t("vnContinue"))!;
      cont.focus();
      await press("Escape");
      expect(host.querySelector(".vn-game-menu")).not.toBeNull();
      expect(host.querySelector(".vn-bottom")!.hasAttribute("inert")).toBe(true);
      expect(document.activeElement).not.toBe(cont);
      // 原生啟動落在當下的焦點上：焦點已不在「繼續」，Enter 不會按到它
      const enter = await press("Enter", {}, document.activeElement ?? document.body);
      if (!enter.defaultPrevented && document.activeElement === cont) cont.click();
      expect(chat.gmAdvance).not.toHaveBeenCalled();
    });

    it("holding Ctrl skips and letting go or leaving the window restores the previous state", async () => {
      backend.cells = three();
      backend.progress = { version: 1, read: [n("a"), n("b"), n("c")].map(cellHash), positions: {}, saves: [] };
      await render({ events: [event("g1")] });
      await press("Control");
      await press("Control", { repeat: true });
      expect(pressed(t("vnSkip"))).toBe("true");
      await press("Control", {}, document.body, "keyup");
      expect(pressed(t("vnSkip"))).toBe("false");
      await press("Control");
      await act(async () => void window.dispatchEvent(new Event("blur")));
      await settle();
      expect(pressed(t("vnSkip"))).toBe("false");
    });

    it("Esc closes a panel even from a focused field; other keys do nothing while it is open", async () => {
      backend.cells = three();
      await render({ events: [event("g1")] });
      await act(async () => button(t("vnTitleSettings"))!.click());
      await press(" ");
      expect(text()).toBe("a");
      await press("Escape", {}, host.querySelector(".vn-setting select")!);
      expect(host.querySelector(".vn-story-panel")).toBeNull();
      await act(async () => button(t("vnLog"))!.click());
      await press("Escape");
      expect(host.querySelector(".vn-backlog")).toBeNull();
      expect(host.querySelector(".vn-game-menu")).toBeNull();
    });

    it("clicking or wheeling the stage advances, a right click opens the menu, an open choice ignores the stage", async () => {
      backend.cells = [gm("g1", [n("a"), n("b"), choices("A", "B")])];
      await render({ events: [event("g1")] });
      await click(stage());
      expect(text()).toBe("b");
      await act(async () => void stage().dispatchEvent(new WheelEvent("wheel", { deltaY: -1, bubbles: true })));
      expect(text()).toBe("a");
      await new Promise((resolve) => setTimeout(resolve, 260));
      await act(async () => void stage().dispatchEvent(new WheelEvent("wheel", { deltaY: 1, bubbles: true })));
      await settle();
      expect(text()).toBe("b");
      await advance();
      expect(choiceButtons()).toHaveLength(2);
      await click(stage());
      expect(choiceButtons()).toHaveLength(2);
      expect((await click(stage(), "contextmenu")).defaultPrevented).toBe(true);
      expect(host.querySelector(".vn-game-menu")).not.toBeNull();
    });

    it("clicking anywhere on a chapter cell reached by skip starts the chapter end", async () => {
      backend.cells = [gm("g0", [n("a")]), gm("g1", [{ type: "chapter" }])];
      backend.progress = { version: 1, read: [], positions: { "0": "g1#0" }, saves: [] };
      await render({ events: [event("g0"), event("g1")] });
      expect(onAdvanceScene).not.toHaveBeenCalled();
      await click(stage());
      expect(onAdvanceScene).toHaveBeenCalledWith(true);
    });

    it("the title leads with start until there is progress, offers load with saves, and keeps showing under its panels", async () => {
      backend.cells = three();
      titleOpen = true;
      const menu = () => [...host.querySelectorAll(".vn-title-menu button")].map((b) => b.textContent);
      await render({ events: [event("g1")] });
      expect(menu()).toEqual([t("vnTitleStart"), t("vnEndingsList"), t("vnGallery"), t("vnTitleSettings"), t("backToLobby")]);
      expect(host.querySelector(".vn-title-menu .btn-primary")?.textContent).toBe(t("vnTitleStart"));
      await act(async () => host.querySelectorAll<HTMLButtonElement>(".vn-title-menu button")[2].click());
      expect(host.querySelector(".vn-title-screen")).not.toBeNull();
      expect(host.querySelector(".vn-story-panel")).not.toBeNull();
      act(() => root.unmount());
      root = createRoot(host);
      backend.progress = { version: 1, read: [], positions: {}, saves: [{ id: "s1", name: "x", scene: 0, event_id: "g1", cell: 0, chapter: 1, ts: "t", preview: "" }] };
      await render({ events: [event("g1")] });
      expect(menu()).toContain(t("vnTitleLoad"));
      expect(menu()).not.toContain(t("vnTitleContinue"));
      act(() => root.unmount());
      root = createRoot(host);
      backend.progress = { version: 1, read: [], positions: { "0": "g1#1" }, saves: [] };
      await render({ events: [event("g1")] });
      expect(menu().slice(0, 3)).toEqual([t("vnTitleContinue"), t("vnTitleStart"), t("vnTitleLoad")]);
      expect(host.querySelector(".vn-title-menu .btn-primary")?.textContent).toBe(t("vnTitleContinue"));
    });

    it("the log opens at its newest line", async () => {
      backend.cells = three();
      const height = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollHeight");
      Object.defineProperty(HTMLElement.prototype, "scrollHeight", { configurable: true, get: () => 900 });
      try {
        await render({ events: [event("g1")] });
        await act(async () => button(t("vnLog"))!.click());
        expect(host.querySelector<HTMLElement>(".vn-backlog")!.scrollTop).toBe(900);
      } finally {
        if (height) Object.defineProperty(HTMLElement.prototype, "scrollHeight", height);
        else delete (HTMLElement.prototype as { scrollHeight?: number }).scrollHeight;
      }
    });
  });
});
