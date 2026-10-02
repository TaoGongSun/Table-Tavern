// @vitest-environment happy-dom
// 世界書標題列 ⋯、重構執行中的停用範圍、條目列 ⋯ 刪除的接線。

import { act, createRef } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import { WorldbookSection } from "./WorldbookSection";
import type { WorldbookEditorController } from "./useWorldbookEditor";
import { EMPTY_LEDGER } from "./worldbook-model";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const ENTRY: WorldbookEntry = {
  uid: 7,
  title: "Harbor",
  keys: ["harbor"],
  content: "Foggy.",
  constant: false,
  order: 0,
  disabled: false,
  locked: false,
  visibility: { type: "gm" },
};

function controller(entries: WorldbookEntry[]) {
  return {
    entries,
    ledger: EMPTY_LEDGER,
    characters: [],
    message: "",
    draft: null,
    draftOrigin: "",
    setDraft: vi.fn(),
    refreshCharactersForVisibility: vi.fn(),
    toggleLedgerEntry: vi.fn(async () => {}),
    addEntry: vi.fn(),
    editEntry: vi.fn(),
    closeDraft: vi.fn(async () => {}),
    saveEntry: vi.fn(async () => {}),
    deleteEntry: vi.fn(async () => {}),
    reorderEntries: vi.fn(async () => {}),
    dedupeWorldbook: vi.fn(async () => {}),
    exportWorldbook: vi.fn(async () => {}),
    convertEntryToCharacter: vi.fn(async () => {}),
  } as unknown as WorldbookEditorController;
}

describe("WorldbookSection", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function mount(worldbook: WorldbookEditorController, refactorRunning = false) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root?.render(
        <WorldbookSection
          worldbook={worldbook}
          refactorRunning={refactorRunning}
          refactorInputRef={createRef<HTMLInputElement>()}
          onRunRefactor={() => {}}
          onPickRefactorOutcome={() => {}}
          onExportSavedRefactorOutcome={() => {}}
        />,
      );
    });
  }

  const byLabel = (label: string) =>
    host!.querySelector<HTMLButtonElement>(`[aria-label="${label}"]`)!;
  const menuItems = () => [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
  const refactorButton = () => host!.querySelector<HTMLButtonElement>(".worldbook-refactor")!;

  it("puts dedupe, export and the refactor card pair in the more menu", () => {
    mount(controller([ENTRY]));
    act(() => byLabel(t("worldbookMore")).click());
    expect(menuItems().map((item) => item.textContent)).toEqual([
      t("worldbookDedupe"),
      t("worldbookExport"),
      t("refactorImportBtn"),
      t("refactorExportSavedBtn"),
    ]);
    expect(menuItems().some((item) => item.getAttribute("aria-disabled") === "true")).toBe(false);
    expect(refactorButton().disabled).toBe(false);
  });

  it("disables only refactor and the refactor card items while refactoring", () => {
    mount(controller([ENTRY]), true);
    expect(refactorButton().disabled).toBe(true);
    act(() => byLabel(t("worldbookMore")).click());
    const disabled = menuItems()
      .filter((item) => item.getAttribute("aria-disabled") === "true")
      .map((item) => item.textContent);
    expect(disabled).toEqual([t("refactorImportBtn"), t("refactorExportSavedBtn")]);
  });

  it("deletes an entry from its row menu through deleteEntry", () => {
    const worldbook = controller([ENTRY]);
    mount(worldbook);
    act(() => byLabel(t("itemOptionsAria", { name: "Harbor" })).click());
    const items = menuItems();
    expect(items.map((item) => item.textContent)).toEqual([t("worldbookDelete")]);
    expect(items[0].classList.contains("menu-item-danger")).toBe(true);
    act(() => items[0].click());
    expect(worldbook.deleteEntry).toHaveBeenCalledWith(ENTRY);
  });

  it("names untitled entries by uid and gives locked entries no actions", () => {
    mount(
      controller([{ ...ENTRY, title: "" }, { ...ENTRY, uid: 8, title: "Lore", locked: true }]),
    );
    expect(byLabel(t("itemOptionsAria", { name: "7" }))).not.toBeNull();
    const lockedOptions = `[aria-label="${t("itemOptionsAria", { name: "Lore" })}"]`;
    expect(host!.querySelector(lockedOptions)).toBeNull();
    expect(host!.querySelector(".worldbook-locked-view")).not.toBeNull();
  });
});
