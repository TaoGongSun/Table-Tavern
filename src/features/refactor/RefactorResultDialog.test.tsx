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

function controller(detail: boolean, cancelled: boolean): RefactorWorkflowController {
  return {
    outcome,
    selection: defaultRefactorSelection(outcome),
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
});
