// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { RefactorRunDialogs } from "./RefactorRunDialogs";
import type { RefactorModeAsk, RefactorWorkflowController } from "./useRefactorWorkflow";

function controller(
  failure: RefactorModeAsk["failure"],
  recommend: RefactorModeAsk["recommend"] = null,
): RefactorWorkflowController {
  return {
    progress: null,
    modeAsk: { recommend, evidence: "證據句", expanded: recommend === null, picked: "interface", ticket: null, failure },
    setModeAsk: vi.fn(),
    cancelAiRefactor: vi.fn(),
    startRefactorRun: vi.fn(),
  } as unknown as RefactorWorkflowController;
}

describe("重構二選一框的初判失敗說明", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  function render(failure: RefactorModeAsk["failure"], transport?: string, recommend: RefactorModeAsk["recommend"] = null) {
    host = document.createElement("div");
    document.body.append(host);
    root = createRoot(host);
    act(() => root!.render(<RefactorRunDialogs refactor={controller(failure, recommend)} transport={transport} />));
    return [...document.querySelectorAll('[role="alert"]')].map((node) => node.textContent ?? "");
  }

  it("送出後失敗：失敗句＋錯誤人話＋原文，不顯判官句", () => {
    const raw = "AI_HTTP_STATUS_429: status=429 body=busy";
    const alerts = render({ kind: "failed", raw }, "api");
    expect(alerts).toHaveLength(1);
    expect(alerts[0]).toContain(t("refactorProbeFailed"));
    expect(alerts[0]).toContain(t("errQuotaApi"));
    expect(alerts[0]).toContain(raw);
    expect(document.body.textContent).not.toContain(t("refactorModeSuggestInterface", { evidence: "" }).trim());
  });

  it("解析不出：只有一句，不露內部代碼", () => {
    const alerts = render({ kind: "unparsable", raw: "refactor-recommend-unparsable" });
    expect(alerts).toEqual([t("refactorProbeUnparsable")]);
    expect(document.body.textContent).not.toContain("refactor-recommend-unparsable");
  });

  it("初判成功（failure 為 null）：顯示建議句、沒有失敗說明", () => {
    expect(render(null, undefined, "interface")).toEqual([]);
    expect(document.body.textContent).toContain(
      t("refactorModeSuggestInterface", { evidence: "證據句" }),
    );
  });
});
