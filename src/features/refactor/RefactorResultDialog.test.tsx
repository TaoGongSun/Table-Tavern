// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { RefactorResultDialog } from "./RefactorResultDialog";
import { defaultRefactorSelection, type RefactorOutcome } from "./refactor-review";
import type { RefactorWorkflowController } from "./useRefactorWorkflow";

const outcome: RefactorOutcome = {
  characters: [],
  interface: null,
  entries: [],
  mechanisms: [],
  deletable_shared_uids: [],
  dropped: [],
  unabsorbed: [],
  audit: [],
};

function controller(
  detail: boolean,
  cancelled: boolean,
  current: RefactorOutcome = outcome,
): RefactorWorkflowController {
  return {
    outcome: current,
    selection: defaultRefactorSelection(current),
    detail,
    cancelled,
    failures: [{ name: "Alice", reason: "timeout" }],
    busy: false,
    setSelection: vi.fn(),
    setDetail: vi.fn(),
    closeRefactor: vi.fn(),
    restoreDroppedItem: vi.fn(),
    applyRefactor: vi.fn(),
    exportRefactorOutcome: vi.fn(),
  } as unknown as RefactorWorkflowController;
}

describe("refactor result dialog", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  // 取消與部分失敗的說明在摘要與展開兩種檢視都要看得到，玩家才知道少了哪些產出再決定套不套用
  it.each([false, true])("shows cancel and failure notices (detail=%s)", (detail) => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    act(() => root!.render(<RefactorResultDialog refactor={controller(detail, true)} entries={[]} />));

    const alerts = [...document.querySelectorAll('[role="alert"]')].map((node) => node.textContent);
    expect(alerts).toContain(t("refactorCancelledNotice"));
    expect(alerts.some((text) => text?.includes("Alice") && text.includes("timeout"))).toBe(true);
  });

  // 一項都沒勾：展開細看的套用鈕反灰、點了也不套，桌子不動〔作者裁決 2026-10-27〕
  it("disables detail apply when nothing is selected", () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    const refactor = controller(true, false);
    act(() => root!.render(<RefactorResultDialog refactor={refactor} entries={[]} />));

    const apply = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("refactorApplyBtn")),
    );
    expect(apply?.disabled).toBe(true);
    act(() => apply?.click());
    expect(refactor.applyRefactor).not.toHaveBeenCalled();
  });

  // 摘要入口同一條：預設勾選就全空時主鈕反灰、點了不套
  it("disables summary apply when the default selection is empty", () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    const refactor = controller(false, false);
    act(() => root!.render(<RefactorResultDialog refactor={refactor} entries={[]} />));

    const apply = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("refactorApplyAll")),
    );
    expect(apply?.disabled).toBe(true);
    act(() => apply?.click());
    expect(refactor.applyRefactor).not.toHaveBeenCalled();
  });

  // 舞台卡：摘要列出卡名、只勾舞台卡也能一鍵套用；展開後逐張可取消
  const staged: RefactorOutcome = { ...outcome, mode: "characters", stage_cards: ["灰岩要塞"] };

  it("names stage cards in the summary and applies a stage-only selection", () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    const refactor = controller(false, false, staged);
    act(() => root!.render(<RefactorResultDialog refactor={refactor} entries={[]} />));

    expect(host.textContent).toContain(t("refactorSummaryStageNames", { names: "灰岩要塞" }));
    const apply = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes(t("refactorApplyAll")),
    );
    expect(apply?.disabled).toBe(false);
    act(() => apply?.click());
    expect(refactor.applyRefactor).toHaveBeenCalledWith(expect.objectContaining({ stage_indices: [0] }));
  });

  it("lists stage cards as checkboxes in detail view", () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    const refactor = controller(true, false, staged);
    act(() => root!.render(<RefactorResultDialog refactor={refactor} entries={[]} />));

    const box = [...document.querySelectorAll("label")]
      .find((label) => label.textContent?.includes("灰岩要塞"))
      ?.querySelector("input");
    expect(box?.checked).toBe(true);
    act(() => box?.click());
    expect(refactor.setSelection).toHaveBeenCalled();
  });

  // 非 characters 玩法（舊產物、手改檔）帶了舞台卡名也不列：後端同樣不套
  it("hides stage cards outside characters mode", () => {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    const refactor = controller(false, false, { ...staged, mode: "interface" });
    act(() => root!.render(<RefactorResultDialog refactor={refactor} entries={[]} />));
    expect(host.textContent).not.toContain("灰岩要塞");
  });
});
