// @vitest-environment happy-dom
// 世界設定開著時撤銷上次匯入（undo-import-editor-guard）：不重新掛載，world.md 與新條目草稿留著；
// 開著的既有條目草稿依後端報的改動定基——被撤銷刪掉就關、被撤銷改到的欄組以撤銷為準，其餘留玩家的字；
// 定基完成前既有草稿不寫回，已刪條目不會被自動存寫回來。

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

const GONE = 'TTMSG:{"code":"worldbook_entry_gone"}';

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

const changes = (patch: Partial<EntryChanges> = {}): EntryChanges => ({
  changed: [],
  deleted: [],
  ...patch,
});

describe("WorldEditor after undoing the last import", () => {
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
        if (!create && !book.some((item) => item.uid === uid)) throw GONE;
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
        onRefreshReceipts={async () => {}}
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
  const entryTitle = () => form()!.querySelector<HTMLInputElement>("input")!;
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

  it("stays mounted: world.md and a new-entry draft survive, and the book is reread", async () => {
    await mount();
    await typeInto(worldText(), "改過的世界");
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    await typeInto(entryContent(), "新條目草稿");
    await notify([], [changes({ deleted: [1] })]);
    expect(commands("read_world_md")).toHaveLength(1);
    expect(commands("read_worldbook")).toHaveLength(2);
    expect(worldText().value).toBe("改過的世界");
    expect(entryContent().value).toBe("新條目草稿");
    expect(taken).toHaveBeenCalledWith("w1", 1);
  });

  it("an untouched draft follows the undo and stays clean", async () => {
    await mount();
    await openEntry(1);
    book = [entry(1, { visibility: { type: "public" } }), book[1]];
    await notify([], [changes({ changed: [{ uid: 1, groups: ["visibility"] }] })]);
    expect(visibility()).toBe("public");
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("takes the undo's value on reported groups and keeps the player's typing elsewhere", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryTitle(), "玩家標題");
    await typeInto(entryContent(), "玩家內文");
    // 撤銷改了內文與可見度；關鍵字是別處改的、沒報上來，草稿照留
    book = [
      entry(1, { content: "撤銷還原", visibility: { type: "public" }, keys: ["別處改的"] }),
      book[1],
    ];
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content", "visibility"] }] })]);
    expect(entryContent().value).toBe("撤銷還原");
    expect(entryTitle().value).toBe("玩家標題");
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({
      title: "玩家標題",
      content: "撤銷還原",
      keys: [],
      visibility: { type: "public" },
    });
  });

  it("closes a draft whose entry the undo deleted, even an edited one, without asking", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    book = [book[1]];
    await notify([], [changes({ deleted: [1] })]);
    expect(form()).toBeNull();
    expect(await leave()).toBe(true);
    expect(dialogs.confirm).not.toHaveBeenCalled();
    expect(upserts()).toHaveLength(0);
  });

  it("closes a reported draft whose entry the reread no longer has", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    book = [book[1]];
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]);
    expect(form()).toBeNull();
  });

  it("prunes the undone card from a ticked list when the undo only scrubbed it", async () => {
    await mount();
    await openEntry(2);
    const boxC = [...host!.querySelectorAll<HTMLInputElement>(".worldbook-characters input")].find(
      (box) => box.parentElement!.textContent === "丙",
    )!;
    await act(async () => boxC.click());
    cast = cast.filter((character) => character.id !== "B");
    book = [book[0], entry(2, { visibility: { type: "characters", characters: ["A"] } })];
    await notify(["B"], [changes()]);
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: ["A", "C"] });
  });

  it("takes the whole visibility group when the undo really changed it", async () => {
    await mount();
    await openEntry(2);
    const boxC = [...host!.querySelectorAll<HTMLInputElement>(".worldbook-characters input")].find(
      (box) => box.parentElement!.textContent === "丙",
    )!;
    await act(async () => boxC.click());
    book = [book[0], entry(2, { visibility: { type: "characters", characters: ["A"] } })];
    await notify([], [changes({ changed: [{ uid: 2, groups: ["visibility"] }] })]);
    expect(checkedCast()).toEqual(["甲:true", "乙:false", "丙:false"]);
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("does not rebase on a later card deletion when the undo came before mounting", async () => {
    await mount(notice([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]));
    expect(taken).toHaveBeenCalledWith("w1", 1);
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    book = [entry(1, { content: "別的寫入" }), book[1]];
    await notify(["B"], []);
    expect(entryContent().value).toBe("玩家內文");
  });

  it("still rebases when a newer reread overtakes the notice's reread", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryTitle(), "玩家標題");
    const reads = queueReads();
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]);
    await notify([], []);
    const fresh = [entry(1, { content: "撤銷還原" }), book[1]];
    await act(async () => reads[1].resolve(fresh));
    await act(async () => reads[0].resolve([entry(1, { content: "舊回應" }), book[1]]));
    expect(entryContent().value).toBe("撤銷還原");
    expect(entryTitle().value).toBe("玩家標題");
  });

  it("leaving waits for the pending rebase, even when its own reread is overtaken", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    const reads = queueReads();
    await notify([], [changes({ deleted: [1] })]);
    let left: boolean | null = null;
    let leaving: Promise<void> = Promise.resolve();
    await act(async () => {
      leaving = leaveGuard.current!().then((ok) => {
        left = ok;
      });
    });
    // 離開跟著進行中的重讀等，不另發
    expect(reads).toHaveLength(1);
    await notify([], []);
    // 進行中那次被更新的一次頂掉（舊回應丟掉）：等到真正套用才放行
    await act(async () => reads[0].resolve(book));
    expect(left).toBeNull();
    book = [book[1]];
    await act(async () => reads[1].resolve(book));
    await act(async () => leaving);
    expect(left).toBe(true);
    expect(upserts()).toHaveLength(0);
    expect(form()).toBeNull();
  });

  it("switching entries waits for the rebase and never writes the deleted entry back", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    const reads = queueReads();
    await notify([], [changes({ deleted: [1] })]);
    await act(async () => editButton(2).click());
    book = [book[1]];
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    expect(upserts()).toHaveLength(0);
    expect(entryTitle().value).toBe("E2");
  });

  it("refuses to leave without writing when the reread fails", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryTitle(), "玩家標題");
    await typeInto(entryContent(), "打到一半");
    backend.handlers.read_worldbook = () => {
      throw "讀不到世界書";
    };
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]);
    expect(await leave()).toBe(false);
    expect(upserts()).toHaveLength(0);
    expect(entryContent().value).toBe("打到一半");
    backend.handlers.read_worldbook = () => [entry(1, { content: "撤銷還原" }), book[1]];
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ title: "玩家標題", content: "撤銷還原" });
  });

  it("closes only the draft the backend refused as gone", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    book = [book[1]];
    expect(await leave()).toBe(true);
    expect(form()).toBeNull();
    expect(host!.textContent).toContain(t("be_worldbook_entry_gone"));
  });

  it("an unknown undo closes an existing-entry draft but keeps a new one and world.md", async () => {
    await mount();
    await typeInto(worldText(), "改過的世界");
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    await notify([], [null]);
    expect(form()).toBeNull();
    expect(worldText().value).toBe("改過的世界");
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    await typeInto(entryContent(), "新條目草稿");
    await notify([], [null]);
    expect(entryContent().value).toBe("新條目草稿");
  });

  const submitEntry = () => act(async () => form()!.requestSubmit());

  it("saving a clean draft waits for the rebase and writes the undo's whole entry", async () => {
    await mount();
    await openEntry(1);
    const reads = queueReads();
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]);
    let saving: Promise<void> = Promise.resolve();
    await act(async () => {
      saving = submitEntry();
    });
    expect(upserts()).toHaveLength(0);
    const fresh = entry(1, { content: "撤銷還原" });
    book = [fresh, book[1]];
    await act(async () => reads[0].resolve(book));
    await act(async () => {
      for (const read of reads.slice(1)) read.resolve(book);
      await saving;
    });
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toEqual(fresh);
  });

  it("the ledger toggle waits for the rebase reread and flips the undone entry", async () => {
    backend.handlers.mechanism_ledger = () => ({
      entries: [{ uid: 1, title: "E1", kind: "absorbed", detail: "", sent: true }],
      rejected: 0,
      clamped: 0,
      errors: 0,
      jumps: 0,
    });
    await mount();
    const reads = queueReads();
    await notify([], [changes({ changed: [{ uid: 1, groups: ["content"] }] })]);
    const toggle = host!.querySelector<HTMLInputElement>(".mechanism-ledger-toggle input")!;
    let toggling: Promise<void> = Promise.resolve();
    await act(async () => {
      toggle.click();
    });
    expect(upserts()).toHaveLength(0);
    const fresh = entry(1, { content: "撤銷還原" });
    book = [fresh, book[1]];
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    await act(async () => {
      for (const read of reads) read.resolve(book);
      await toggling;
    });
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toEqual({ ...fresh, disabled: true });
  });

  it("converting waits for the rebase and skips an entry the undo deleted", async () => {
    await mount();
    await openEntry(1);
    const reads = queueReads();
    await notify([], [changes({ deleted: [1] })]);
    const convert = [...form()!.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("convertEntryToCard")),
    )!;
    await act(async () => convert.click());
    expect(commands("worldbook_entry_to_character")).toHaveLength(0);
    book = [book[1]];
    await act(async () => {
      for (const read of reads) read.resolve(book);
    });
    expect(commands("worldbook_entry_to_character")).toHaveLength(0);
    expect(form()).toBeNull();
  });

  it("keeps the deleted notice when switching entries hits a gone entry", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "打到一半");
    book = [book[1]];
    await act(async () => editButton(2).click());
    expect(entryTitle().value).toBe("E2");
    expect(host!.textContent).toContain(t("be_worldbook_entry_gone"));
  });
});
