// @vitest-environment happy-dom
// 世界設定開著時側欄刪角色（world-editor-delete-character-guard）：編輯器不關、草稿留著，
// 收到通知只重讀並修剪草稿名單裡確定已刪的 id；別的 id（玩家卡、解析不了的卡）照留。
// 寫回已刪 id 的最後防線在後端存條目指令，這裡驗前端送出的內容與畫面狀態。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../characters/card-model";
import type { CastRemoval } from "./worldbook-model";

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
  calls: [] as { command: string; args: Record<string, unknown> }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
    backend.calls.push({ command, args });
    const handler = backend.handlers[command];
    return handler ? handler(args) : null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
  message: vi.fn(async () => {}),
  save: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));

import { WorldEditor } from "./WorldEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const PLAYER = "01PLAYERCARD00000000000000";

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

function entry(uid: number, characters: string[]): WorldbookEntry {
  return {
    uid,
    title: `E${uid}`,
    keys: [],
    content: "原文",
    constant: false,
    order: 0,
    disabled: false,
    locked: false,
    visibility: { type: "characters", characters },
  };
}

describe("WorldEditor after a character is deleted", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };
  const taken = vi.fn();
  let cast: CharacterMeta[];
  let book: WorldbookEntry[];

  beforeEach(() => {
    cast = [meta("A", "甲"), meta("B", "乙")];
    book = [entry(1, ["A", "B"]), entry(2, ["B"]), entry(3, [PLAYER, "B"])];
    backend.handlers = {
      read_world_md: () => "World text",
      read_worldbook: () => book,
      list_characters: () => cast,
      mechanism_ledger: () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 }),
      upsert_worldbook_entry: () => 1,
    };
    backend.calls = [];
    taken.mockReset();
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function editor(removal: CastRemoval | null, world = "w1") {
    return (
      <WorldEditor
        title={t("worldAria")}
        world={world}
        worldName="Alpha"
        onBack={() => {}}
        leaveGuard={leaveGuard}
        convertColor="#e07a5f"
        onEntryConverted={async () => {}}
        onRefactorApplied={async () => {}}
        isTurnRunning={() => false}
        pendingRefactorCard={null}
        onPendingRefactorCardTaken={() => {}}
        castRemoval={removal}
        onCastRemovalTaken={taken}
      />
    );
  }

  async function mount(removal: CastRemoval | null = null) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(editor(removal)));
  }

  async function notify(removal: CastRemoval, world = "w1") {
    await act(async () => root!.render(editor(removal, world)));
  }

  /** 刪掉 B：後端已清（名單拿掉 B）＝重讀拿到的是清理後的資料 */
  function backendDeletes(id: string) {
    cast = cast.filter((character) => character.id !== id);
    book = book.map((item) =>
      item.visibility.type === "characters" && item.visibility.characters.includes(id)
        ? (() => {
            const kept = item.visibility.characters.filter((other) => other !== id);
            return {
              ...item,
              visibility: kept.length > 0 ? { type: "characters" as const, characters: kept } : { type: "gm" as const },
            };
          })()
        : item,
    );
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

  async function openEntry(uid: number) {
    const rows = [...host!.querySelectorAll(".worldbook-row-actions")];
    const edit = [...rows[book.findIndex((item) => item.uid === uid)].querySelectorAll("button")].find(
      (button) => button.textContent === t("editBtn"),
    )!;
    await act(async () => edit.click());
  }

  const worldText = () => host!.querySelector<HTMLTextAreaElement>(".settings-form textarea")!;
  const entryContent = () => host!.querySelector<HTMLTextAreaElement>(".worldbook-form textarea")!;
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

  it("stays open with world.md and the new-entry draft, and rereads", async () => {
    await mount();
    await typeInto(worldText(), "改過的世界");
    const add = host!.querySelector<HTMLButtonElement>(".worldbook-add")!;
    await act(async () => add.click());
    await typeInto(entryContent(), "新條目草稿");
    backend.calls = [];
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    expect(worldText().value).toBe("改過的世界");
    expect(entryContent().value).toBe("新條目草稿");
    expect(commands("read_worldbook")).toHaveLength(1);
    expect(commands("list_characters")).toHaveLength(1);
    expect(commands("mechanism_ledger")).toHaveLength(1);
    expect(taken).toHaveBeenCalledWith("w1", 1);
  });

  it("leaves an untouched entry draft clean and does not save it", async () => {
    await mount();
    await openEntry(1);
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    expect(checkedCast()).toEqual(["甲:true"]);
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("autosaves an edited draft without the deleted id, handing an emptied list to the GM", async () => {
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "改過的內文");
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    expect(entryContent().value).toBe("改過的內文");
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0].visibility).toEqual({ type: "gm" });
    expect(upserts()[0].content).toBe("改過的內文");
  });

  it("becomes clean when the only change was unticking the deleted character", async () => {
    await mount();
    await openEntry(1);
    const boxB = [...host!.querySelectorAll<HTMLInputElement>(".worldbook-characters input")].find(
      (box) => box.parentElement!.textContent === "乙",
    )!;
    await act(async () => boxB.click());
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("keeps ids that were not deleted, such as the player card", async () => {
    await mount();
    await openEntry(3);
    await typeInto(entryContent(), "改過");
    backendDeletes("A");
    await notify({ worldId: "w1", generation: 1, ids: ["A"] });
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: [PLAYER, "B"] });
  });

  it("keeps typing done while the reread is in flight and drops a stale reply", async () => {
    await mount();
    await openEntry(1);
    let release: () => void = () => {};
    const stale = [...book, { ...entry(9, ["B"]), title: "舊回應" }];
    backend.handlers.read_worldbook = () =>
      new Promise((resolve) => {
        release = () => resolve(stale);
      });
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    await typeInto(entryContent(), "重讀途中打的字");
    // 第二次通知的重讀先回來，第一次的舊回應晚到要丟掉
    const fresh = book;
    backend.handlers.read_worldbook = () => fresh;
    await notify({ worldId: "w1", generation: 2, ids: ["B"] });
    await act(async () => release());
    expect(entryContent().value).toBe("重讀途中打的字");
    expect(host!.textContent).not.toContain("舊回應");
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: ["A"] });
  });

  it("still prunes the draft when the reread fails", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "改過");
    backend.handlers.read_worldbook = () => {
      throw "讀不到世界書";
    };
    backend.handlers.list_characters = () => {
      throw "讀不到名單";
    };
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    expect(host!.textContent).toContain("讀不到");
    expect(entryContent().value).toBe("改過");
    // 自動存完會重讀清單：讓它讀得到，只驗送出的內容
    backend.handlers.read_worldbook = () => book;
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: ["A"] });
  });

  it("handles two deletions and ignores another table's notice", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "改過");
    await notify({ worldId: "w2", generation: 1, ids: ["A", "B"] });
    expect(taken).not.toHaveBeenCalled();
    await notify({ worldId: "w1", generation: 2, ids: ["B"] });
    await notify({ worldId: "w1", generation: 3, ids: ["A"] });
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "gm" });
  });

  it("ignores a notice that was already there when it mounted", async () => {
    await mount({ worldId: "w1", generation: 5, ids: ["B"] });
    expect(commands("read_worldbook")).toHaveLength(1);
    expect(taken).not.toHaveBeenCalled();
    await openEntry(1);
    await typeInto(entryContent(), "改過");
    expect(await leave()).toBe(true);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: ["A", "B"] });
  });

  const ledgerWith = (uid: number, title: string) => ({
    entries: [{ uid, title, kind: "absorbed", detail: "", sent: true }],
    rejected: 0,
    clamped: 0,
    errors: 0,
    jumps: 0,
  });

  it("toggles a ledger entry from the reread book, without the deleted id", async () => {
    backend.handlers.mechanism_ledger = () => ledgerWith(1, "E1");
    await mount();
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    const toggle = host!.querySelector<HTMLInputElement>(".mechanism-ledger-toggle input")!;
    await act(async () => toggle.click());
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0].visibility).toEqual({ type: "characters", characters: ["A"] });
  });

  it("drops stale cast and ledger replies", async () => {
    await mount();
    const pending: { cast?: () => void; ledger?: () => void } = {};
    const staleCast = cast;
    backend.handlers.list_characters = () =>
      new Promise((resolve) => (pending.cast = () => resolve(staleCast)));
    backend.handlers.mechanism_ledger = () =>
      new Promise((resolve) => (pending.ledger = () => resolve(ledgerWith(9, "舊帳本"))));
    backendDeletes("B");
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    const freshCast = cast;
    backend.handlers.list_characters = () => freshCast;
    backend.handlers.mechanism_ledger = () => ledgerWith(1, "新帳本");
    await notify({ worldId: "w1", generation: 2, ids: ["B"] });
    await act(async () => {
      pending.cast?.();
      pending.ledger?.();
    });
    expect(host!.textContent).toContain("新帳本");
    expect(host!.textContent).not.toContain("舊帳本");
    await openEntry(1);
    expect(checkedCast()).toEqual(["甲:true"]);
  });

  it("drops a stale failed reread instead of showing its error", async () => {
    await mount();
    const pending: { book?: () => void; cast?: () => void } = {};
    backend.handlers.read_worldbook = () =>
      new Promise((_, reject) => (pending.book = () => reject("舊的世界書錯誤")));
    backend.handlers.list_characters = () =>
      new Promise((_, reject) => (pending.cast = () => reject("舊的名單錯誤")));
    await notify({ worldId: "w1", generation: 1, ids: ["B"] });
    backend.handlers.read_worldbook = () => book;
    backend.handlers.list_characters = () => cast;
    await notify({ worldId: "w1", generation: 2, ids: ["B"] });
    await act(async () => {
      pending.book?.();
      pending.cast?.();
    });
    expect(host!.textContent).not.toContain("舊的");
  });
});
