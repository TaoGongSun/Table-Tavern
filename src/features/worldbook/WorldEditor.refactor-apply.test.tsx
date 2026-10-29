// @vitest-environment happy-dom
// 世界設定裡套用重構卡、清回重跑、帳本開關、去重（refactor-apply-open-draft）：開著的既有條目草稿不會在之後的
// 自動存把這些操作的結果蓋回去——套用照後端回報定基（停用跟上、被刪就關）、清回與去重先存並關草稿、帳本開關的
// enabled 跟上。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { PendingRefactorCard } from "../refactor/refactor-card";
import type { EntryChanges } from "./worldbook-model";

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
  Channel: class {
    onmessage: (delta: string) => void = () => {};
  },
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

const OUTCOME_CARD = {
  card: {
    format: "table-tavern-refactor-card",
    version: 1,
    outcome: { characters: [{ name: "阿福", source_uids: ["2"] }] },
  },
  assets: [],
  token: null,
};

describe("WorldEditor after a refactor apply or reset", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const leaveGuard: { current: (() => Promise<boolean>) | null } = { current: null };
  let book: WorldbookEntry[];
  const refactorApplied = vi.fn(async (_live: () => boolean) => {});
  const refreshReceipts = vi.fn(async () => {});

  beforeEach(() => {
    book = [entry(1), entry(2), entry(3)];
    backend.handlers = {
      read_world_md: () => "World text",
      read_worldbook: () => book,
      list_characters: () => [],
      mechanism_ledger: () => ({ entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 }),
      refactor_card_open: () => OUTCOME_CARD,
      upsert_worldbook_entry: ({ entry: saved, create }) => {
        const next = saved as WorldbookEntry;
        if (!create && !book.some((item) => item.uid === next.uid)) throw "gone";
        book = book.map((item) => (item.uid === next.uid ? { ...next } : item));
        return next.uid;
      },
    };
    backend.calls = [];
    dialogs.confirm.mockClear();
    refactorApplied.mockClear();
    refreshReceipts.mockClear();
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function editor(pending: PendingRefactorCard | null = null) {
    return (
      <WorldEditor
        title={t("worldAria")}
        world="w1"
        worldName="Alpha"
        onBack={() => {}}
        leaveGuard={leaveGuard}
        convertColor="#e07a5f"
        onEntryConverted={async () => {}}
        onRefactorApplied={refactorApplied}
        onRefreshReceipts={refreshReceipts}
        isTurnRunning={() => false}
        pendingRefactorCard={pending}
        onPendingRefactorCardTaken={() => {}}
        worldChange={null}
        onWorldChangeTaken={() => {}}
      />
    );
  }

  async function mount() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => root!.render(editor()));
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
  const openEntry = async (uid: number) => act(async () => editButton(uid).click());
  const form = () => host!.querySelector<HTMLFormElement>(".worldbook-form");
  const entryContent = () => form()!.querySelector<HTMLTextAreaElement>("textarea")!;
  const enabledBox = () =>
    [...form()!.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')].find((box) =>
      box.parentElement?.textContent?.includes(t("worldbookEnabled")),
    )!;
  const button = (label: string) =>
    [...document.querySelectorAll<HTMLButtonElement>("button")].find((candidate) =>
      candidate.textContent?.startsWith(label),
    )!;
  const leave = async () => {
    let ok = false;
    await act(async () => {
      ok = await leaveGuard.current!();
    });
    return ok;
  };

  /** 陣容欄交來重構卡、按全部套用；後端把 uid 1 停用、刪掉 uid 2，回報 changes */
  async function applyRefactor(changes: EntryChanges | null) {
    backend.handlers.refactor_apply = () => {
      book = [{ ...book[0], disabled: true }, ...book.slice(2)];
      return { new_characters: 1, entry_changes: changes };
    };
    const pending = { worldId: "w1", file: new File(["{}"], "card.png"), generation: 1 };
    await act(async () => root!.render(editor(pending)));
    await vi.waitFor(() => expect(button(t("refactorApplyAll"))).toBeDefined());
    await act(async () => button(t("refactorApplyAll")).click());
    await vi.waitFor(() => expect(commands("refactor_apply")).toHaveLength(1));
    await act(async () => {});
  }
  const REPORTED: EntryChanges = { changed: [{ uid: 1, groups: ["enabled"] }], deleted: [2] };

  it("a dirty draft of a disabled entry keeps the player's text and saves it disabled", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    await applyRefactor(REPORTED);
    expect(enabledBox().checked).toBe(false);
    await openEntry(3);
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ uid: 1, content: "玩家內文", disabled: true });
  });

  it("a clean draft of a disabled entry shows it disabled, stays clean, and leaves without a write", async () => {
    await mount();
    await openEntry(1);
    await applyRefactor(REPORTED);
    expect(enabledBox().checked).toBe(false);
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("a dirty draft of a deleted entry closes without asking and writes nothing", async () => {
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "改過");
    dialogs.confirm.mockClear();
    await applyRefactor(REPORTED);
    expect(form()).toBeNull();
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
    expect(dialogs.confirm).not.toHaveBeenCalled();
  });

  it("a draft of an entry the apply did not touch stays as typed", async () => {
    await mount();
    await openEntry(3);
    await typeInto(entryContent(), "沒被動到");
    await applyRefactor(REPORTED);
    expect(entryContent().value).toBe("沒被動到");
    await openEntry(1);
    expect(upserts()[0]).toMatchObject({ uid: 3, content: "沒被動到", disabled: false });
    expect(commands("read_world_md")).toHaveLength(1);
  });

  it("a new-entry draft stays through the apply", async () => {
    await mount();
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    await typeInto(entryContent(), "新條目草稿");
    await applyRefactor(REPORTED);
    expect(entryContent().value).toBe("新條目草稿");
    expect(upserts()).toHaveLength(0);
  });

  it("switching entries before the post-apply reread lands waits and writes the disabled entry", async () => {
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    const reads: ((value: WorldbookEntry[]) => void)[] = [];
    backend.handlers.read_worldbook = () => new Promise((resolve) => reads.push(resolve));
    await applyRefactor(REPORTED);
    await act(async () => editButton(3).click());
    expect(upserts()).toHaveLength(0);
    await act(async () => {
      for (const read of reads.splice(0)) read(book);
    });
    await act(async () => {
      for (const read of reads.splice(0)) read(book);
    });
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ uid: 1, content: "玩家內文", disabled: true });
  });

  it("an apply that could not report its changes closes the open existing draft", async () => {
    await mount();
    await openEntry(3);
    await typeInto(entryContent(), "改過");
    await applyRefactor(null);
    expect(form()).toBeNull();
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(0);
  });

  it("a rerun saves and closes a dirty existing draft before resetting", async () => {
    backend.handlers.refactor_rerun_status = () => "ready";
    backend.handlers.refactor_reset_to_source = () => {
      book = [entry(1, { content: "原卡" })];
      return "committed";
    };
    // 清回之後的重構擋在不支援的卡上，不跑 AI
    backend.handlers.card_interfaces = () => [{ unsupported: "drm", scripts: [] }];
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "重構後改的");
    await act(async () => button(t("refactorBtn")).click());
    await vi.waitFor(() => expect(commands("card_interfaces")).toHaveLength(1));
    const order = backend.calls.map((call) => call.command);
    expect(order.indexOf("upsert_worldbook_entry")).toBeLessThan(order.indexOf("refactor_reset_to_source"));
    expect(upserts()).toHaveLength(1);
    expect(form()).toBeNull();
    expect(await leave()).toBe(true);
    expect(upserts()).toHaveLength(1);
  });

  it("a clean existing draft is closed before the reset is sent and never written", async () => {
    let closedWhileResetPending = false;
    backend.handlers.refactor_rerun_status = () => "ready";
    // 清回還沒回應時表單就要關掉（關草稿在送出前；畫面晚一次 render，用等的不賭計時器先後）
    backend.handlers.refactor_reset_to_source = async () => {
      await vi.waitFor(() => expect(form()).toBeNull(), { timeout: 200 }).then(
        () => (closedWhileResetPending = true),
        () => {},
      );
      return "committed";
    };
    backend.handlers.card_interfaces = () => [{ unsupported: "drm", scripts: [] }];
    await mount();
    await openEntry(1);
    await act(async () => button(t("refactorBtn")).click());
    await vi.waitFor(() => expect(commands("card_interfaces")).toHaveLength(1));
    expect(closedWhileResetPending).toBe(true);
    expect(form()).toBeNull();
    expect(upserts()).toHaveLength(0);
  });

  it("a typed new-entry draft survives the rerun and is never written", async () => {
    backend.handlers.refactor_rerun_status = () => "ready";
    backend.handlers.refactor_reset_to_source = () => "committed";
    backend.handlers.card_interfaces = () => [{ unsupported: "drm", scripts: [] }];
    await mount();
    await act(async () => host!.querySelector<HTMLButtonElement>(".worldbook-add")!.click());
    await typeInto(entryContent(), "新條目草稿");
    await act(async () => button(t("refactorBtn")).click());
    await vi.waitFor(() => expect(commands("card_interfaces")).toHaveLength(1));
    expect(commands("refactor_reset_to_source")).toHaveLength(1);
    expect(entryContent().value).toBe("新條目草稿");
    expect(upserts()).toHaveLength(0);
  });

  it("the ledger toggle carries the new enabled into an open dirty draft of the same entry", async () => {
    backend.handlers.mechanism_ledger = () => ({
      entries: [{ uid: 1, title: "E1", kind: "absorbed", detail: "", sent: true }],
      rejected: 0,
      clamped: 0,
      errors: 0,
      jumps: 0,
    });
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    const toggle = host!.querySelector<HTMLInputElement>(".mechanism-ledger-toggle input")!;
    await act(async () => toggle.click());
    expect(upserts()).toHaveLength(1);
    await openEntry(3);
    expect(upserts()).toHaveLength(2);
    expect(upserts()[1]).toMatchObject({ uid: 1, content: "玩家內文", disabled: true });
  });

  it("dedupe is not sent when saving the open existing draft fails", async () => {
    backend.handlers.dedupe_worldbook = () => 0;
    backend.handlers.upsert_worldbook_entry = () => {
      throw "寫不進去";
    };
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    const more = host!.querySelector<HTMLButtonElement>(`button[aria-label="${t("worldbookMore")}"]`)!;
    await act(async () => more.click());
    await act(async () => button(t("worldbookDedupe")).click());
    await act(async () => {});
    expect(upserts()).toHaveLength(1);
    expect(commands("dedupe_worldbook")).toHaveLength(0);
    expect(entryContent().value).toBe("玩家內文");
  });

  it("dedupe saves and closes the open existing draft before running", async () => {
    backend.handlers.dedupe_worldbook = () => 0;
    await mount();
    await openEntry(1);
    await typeInto(entryContent(), "玩家內文");
    const more = host!.querySelector<HTMLButtonElement>(`button[aria-label="${t("worldbookMore")}"]`)!;
    await act(async () => more.click());
    await act(async () => button(t("worldbookDedupe")).click());
    await vi.waitFor(() => expect(commands("dedupe_worldbook")).toHaveLength(1));
    const order = backend.calls.map((call) => call.command);
    expect(order.indexOf("upsert_worldbook_entry")).toBeLessThan(order.indexOf("dedupe_worldbook"));
    expect(upserts()[0]).toMatchObject({ uid: 1, content: "玩家內文" });
    expect(form()).toBeNull();
  });

  /** 套用中途失敗、沒能整個退回：後端已改了世界書（mutate），回部分套用的錯 */
  async function failApply(mutate: () => void) {
    backend.handlers.refactor_apply = () => {
      mutate();
      return Promise.reject('TTMSG:{"code":"refactor_apply_partial","error":"寫卡失敗"}');
    };
    const pending = { worldId: "w1", file: new File(["{}"], "card.png"), generation: 1 };
    await act(async () => root!.render(editor(pending)));
    await vi.waitFor(() => expect(button(t("refactorApplyAll"))).toBeDefined());
    await act(async () => button(t("refactorApplyAll")).click());
    await vi.waitFor(() => expect(refreshReceipts).toHaveBeenCalledTimes(1));
    await act(async () => {});
  }

  it("a failed apply saves the open dirty draft before refreshing, then rereads the receipts", async () => {
    await mount();
    await openEntry(3);
    await typeInto(entryContent(), "玩家內文");
    await failApply(() => {
      book = book.filter((item) => item.uid !== 2);
    });
    expect(upserts()).toHaveLength(1);
    expect(upserts()[0]).toMatchObject({ uid: 3, content: "玩家內文" });
    expect(refactorApplied).toHaveBeenCalledTimes(1);
    expect(form()).toBeNull();
  });

  it("a failed apply that deleted the drafted entry keeps the draft open with the unsaved text", async () => {
    backend.handlers.upsert_worldbook_entry = ({ entry: saved, create }) => {
      const next = saved as WorldbookEntry;
      if (!create && !book.some((item) => item.uid === next.uid)) {
        throw 'TTMSG:{"code":"worldbook_entry_gone"}';
      }
      book = book.map((item) => (item.uid === next.uid ? { ...next } : item));
      return next.uid;
    };
    await mount();
    await openEntry(2);
    await typeInto(entryContent(), "玩家內文");
    await failApply(() => {
      book = book.filter((item) => item.uid !== 2);
    });
    expect(upserts()).toHaveLength(1);
    expect(form()).not.toBeNull();
    expect(entryContent().value).toBe("玩家內文");
    expect(refactorApplied).not.toHaveBeenCalled();
    expect(refreshReceipts).toHaveBeenCalledTimes(1);

    // 按存檔：後端拒寫，表單照留、文字還在，訊息說條目已不在〔作者裁決 2026-10-29〕
    await act(async () => form()!.requestSubmit());
    await act(async () => {});
    expect(upserts()).toHaveLength(2);
    expect(form()).not.toBeNull();
    expect(entryContent().value).toBe("玩家內文");
    expect(host!.textContent).toContain(backendText('TTMSG:{"code":"worldbook_entry_gone"}'));

    // 切到別條照舊關掉這份草稿（已知限制）
    await openEntry(3);
    expect(entryContent().value).toBe("原文3");
  });
});
