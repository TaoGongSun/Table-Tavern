// @vitest-environment happy-dom
// 世界設定開著時從陣容欄匯入（import-refresh-world-editor）：收到匯入通知只重讀清單、勾選名單與帳本，
// 不重新掛載，world.md 與草稿留著；被匯入擴大了可見度的既有條目草稿，可見度以匯入值為準、其餘留玩家的字；
// 重讀完成前既有草稿與帳本開關不寫回過期內容。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../characters/card-model";
import type { EntryChanges, WorldChange } from "./worldbook-model";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));
const dialogs = vi.hoisted(() => ({ confirm: vi.fn(async () => true) }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: dialogs.confirm,
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { WorldEditor } from "./WorldEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function meta(id: string, name: string): CharacterMeta {
  return {
    id,
    name,
    color: "#e07a5f",
    avatar: "🎭",
    tier: "balanced",
    show_image: true,
    archived: false,
    auto_hidden: false,
  };
}

function entry(uid: number, patch: Partial<WorldbookEntry> = {}): WorldbookEntry {
  return {
    uid,
    title: `E${uid}`,
    keys: [],
    content: `原文${uid}`,
    constant: false,
    order: 0,
    disabled: false,
    locked: false,
    visibility: { type: "gm" },
    ...patch,
  };
}

/** 匯入通知：擴大了可見度的條目帶 visibility 組（同 navigation.noteImported） */
const imported = (widened: number[]): EntryChanges[] =>
  widened.length > 0
    ? [{ changed: widened.map((uid) => ({ uid, groups: ["visibility"] as const })), deleted: [] }]
    : [];

describe("WorldEditor after an import", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };
  const taken = vi.fn();
  let cast: CharacterMeta[];
  let book: WorldbookEntry[];
  let generation = 0;

  beforeEach(() => {
    cast = [meta("A", "甲"), meta("B", "乙"), meta("C", "丙")];
    book = [entry(1), entry(2, { visibility: { type: "characters", characters: ["A", "B"] } })];
    generation = 0;
    backend.handlers = {
      read_world_md: () => "World text",
      read_worldbook: () => book,
      list_characters: () => cast,
      mechanism_ledger: () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 }),
      upsert_worldbook_entry: ({ entry: saved, create }) => {
        const uid = (saved as WorldbookEntry).uid;
        if (!create && !book.some((item) => item.uid === uid)) throw "gone";
        return uid;
      },
    };
    backend.calls = [];
    taken.mockReset();
    dialogs.confirm.mockClear();
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function editor(change: WorldChange | null) {
    return (
      <WorldEditor
        title={t("worldAria")}
        world="w1"
        worldName="Alpha"
        onBack={() => {}}
        leaveGuard={leaveGuard}
        convertColor="#e07a5f"
        onEntryConverted={async () => {}}
        onRefactorApplied={async () => {}}
        isTurnRunning={() => false}
        pendingRefactorCard={null}
        onPendingRefactorCardTaken={() => {}}
        worldChange={change}
        onWorldChangeTaken={taken}
      />
    );
  }

  async function mount(change: WorldChange | null = null) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(editor(change)));
  }

  function notice(ids: string[], undo: (EntryChanges | null)[]): WorldChange {
    generation += 1;
    return { worldId: "w1", generation, ids, entryChanges: undo };
  }

  async function notify(ids: string[], undo: (EntryChanges | null)[]) {
    await act(async () => root!.render(editor(notice(ids, undo))));
  }

  /** read_worldbook 改成排隊：每次呼叫留一個待決，測試決定何時、以什麼回應 */
  function queueReads() {
    const queue: { resolve: (value: WorldbookEntry[]) => void; reject: (reason: unknown) => void }[] = [];
    backend.handlers.read_worldbook = () =>
      new Promise((resolve, reject) => queue.push({ resolve, reject }));
    return queue;
  }

  const commands = (name: string) => backend.calls.filter((call) => call.command === name);
  const upserts = () => commands("upsert_worldbook_entry").map((call) => call.args.entry as WorldbookEntry);

  async function typeInto(element: HTMLTextAreaElement | HTMLInputElement, value: string) {
    const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")!.set!;
    await act(async () => {
      setter.call(element, value);
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
  }

  function editButton(uid: number) {
    const row = [...host!.querySelectorAll(".worldbook-row")].find((candidate) =>
      candidate.textContent?.includes(`E${uid}`),
    )!;
    return [...row.querySelectorAll(".worldbook-row-actions button")].find(
      (button) => button.textContent === t("editBtn"),
    ) as HTMLButtonElement;
  }

  async function openEntry(uid: number) {
    await act(async () => editButton(uid).click());
  }

  const worldText = () => host!.querySelector<HTMLTextAreaElement>(".settings-form textarea")!;
  const form = () => host!.querySelector<HTMLFormElement>(".worldbook-form");
  const entryContent = () => form()!.querySelector<HTMLTextAreaElement>("textarea")!;
  const visibility = () =>
    form()!.querySelector<HTMLInputElement>('input[name="worldbook-visibility"]:checked')!.value;
  const checkedCast = () =>
    [...host!.querySelectorAll<HTMLInputElement>(".worldbook-characters input")].map(
      (box) => `${box.parentElement!.textContent}:${box.checked}`,
    );
  const leave = async () => {
    let ok = false;
    await act(async () => {
      ok = await leaveGuard.current!();
    });
    return ok;
  };

  it("rereads the book and cast without remounting; world.md and drafts stay", async () => {
    await mount();
    await typeInto(worldText(), "改過的世界");
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    await typeInto(entryContent(), "新條目草稿");
    cast = [...cast, meta("D", "丁")];
    book = [...book, entry(3, { visibility: { type: "characters", characters: ["D"] } })];
    await notify([], imported([]));
    expect(commands("read_world_md")).toHaveLength(1);
    expect(commands("read_worldbook")).toHaveLength(2);
    expect(commands("list_characters")).toHaveLength(2);
    expect(commands("mechanism_ledger")).toHaveLength(2);
    expect(host!.textContent).toContain("E3");
    expect(worldText().value).toBe("改過的世界");
    expect(entryContent().value).toBe("新條目草稿");
    await act(async () =>
      form()!.querySelector<HTMLInputElement>('input[name="worldbook-visibility"][value="characters"]')!.click(),
    );
    expect(checkedCast()).toContain("丁:false");
  });

  it("a widened entry's draft takes the import's visibility and keeps the player's typing", async () => {
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "玩家內文");
    cast = [...cast, meta("D", "丁")];
    book = [book[0], entry(2, { visibility: { type: "characters", characters: ["A", "B", "D"] } })];
    await notify([], imported([2]));
    expect(entryContent().value).toBe("玩家內文");
    expect(checkedCast()).toEqual(["甲:true", "乙:true", "丙:false", "丁:true"]);
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({
      uid: 2,
      content: "玩家內文",
      visibility: { type: "characters", characters: ["A", "B", "D"] },
    });
  });

  it("the player's own visibility change on a widened entry yields to the import", async () => {
    await mount();
    await openEntry(1);
    await act(async () =>
      form()!.querySelector<HTMLInputElement>('input[name="worldbook-visibility"][value="public"]')!.click(),
    );
    book = [entry(1, { visibility: { type: "characters", characters: ["A"] } }), book[1]];
    await notify([], imported([1]));
    expect(visibility()).toBe("characters");
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("leaves a draft the import did not widen alone", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    await notify([], imported([2]));
    expect(entryContent().value).toBe("玩家內文");
    expect(visibility()).toBe("gm");
    expect(await leave()).toBe(true);
    expect(upserts()[0]).toMatchObject({ uid: 1, content: "玩家內文", visibility: { type: "gm" } });
  });

  it("leaving before the import's reread lands writes the widened visibility, never the stale one", async () => {
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "玩家內文");
    const reads = queueReads();
    await notify([], imported([2]));
    let left: boolean | null = null;
    let leaving: Promise<void> = Promise.resolve();
    await act(async () => {
      leaving = leaveGuard.current!().then((ok) => {
        left = ok;
      });
    });
    expect(upserts()).toHaveLength(0);
    book = [book[0], entry(2, { visibility: { type: "public" } })];
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    // 存完後的重讀
    await act(async () => {
      for (const read of reads) read.resolve(book);
      await leaving;
    });
    expect(left).toBe(true);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ content: "玩家內文", visibility: { type: "public" } });
  });

  it("a failed reread blocks the write; the next successful one applies the import", async () => {
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "玩家內文");
    backend.handlers.read_worldbook = () => {
      throw "讀不到世界書";
    };
    await notify([], imported([2]));
    expect(await leave()).toBe(false);
    expect(upserts()).toHaveLength(0);
    book = [book[0], entry(2, { visibility: { type: "public" } })];
    backend.handlers.read_worldbook = () => book;
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ content: "玩家內文", visibility: { type: "public" } });
  });

  it("the ledger toggle waits for the import's reread and writes the widened entry", async () => {
    backend.handlers.mechanism_ledger = () => ({
      entries: [{ uid: 2, title: "E2", kind: "absorbed", detail: "", sent: true }],
      rejected: 0,
      clamped: 0,
      errors: 0,
      jumps: 0,
    });
    await mount();
    const reads = queueReads();
    await notify([], imported([2]));
    const toggle = host!.querySelector<HTMLInputElement>(".mechanism-ledger-toggle input")!;
    await act(async () => toggle.click());
    expect(upserts()).toHaveLength(0);
    const fresh = entry(2, { visibility: { type: "public" } });
    book = [book[0], fresh];
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toEqual({ ...fresh, disabled: true });
  });
  it("the ledger toggle writes nothing while the import's reread fails", async () => {
    backend.handlers.mechanism_ledger = () => ({
      entries: [{ uid: 2, title: "E2", kind: "absorbed", detail: "", sent: true }],
      rejected: 0,
      clamped: 0,
      errors: 0,
      jumps: 0,
    });
    await mount();
    backend.handlers.read_worldbook = () => {
      throw "讀不到世界書";
    };
    await notify([], imported([2]));
    const toggle = host!.querySelector<HTMLInputElement>(".mechanism-ledger-toggle input")!;
    await act(async () => toggle.click());
    await act(async () => {});
    expect(upserts()).toHaveLength(0);
  });
});
