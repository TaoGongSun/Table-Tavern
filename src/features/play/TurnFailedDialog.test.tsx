// @vitest-environment happy-dom
// 回合失敗彈窗：分流文案＋原文小字、玩家原文唯讀可選、只有「關閉」、遮罩不關。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { nextTurnFailure, TurnFailedDialog } from "./TurnFailedDialog";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("TurnFailedDialog", () => {
  let root: Root | null = null;

  afterEach(() => {
    act(() => root?.unmount());
    root = null;
    document.body.innerHTML = "";
  });

  function render(raw: string, draft?: string, transport = "api") {
    const onClose = vi.fn();
    const host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() =>
      root!.render(
        <TurnFailedDialog
          failure={draft === undefined ? { raw } : { raw, draft }}
          transport={transport}
          onClose={onClose}
        />,
      ),
    );
    return { onClose, dialog: document.querySelector("dialog")! };
  }

  it("shows the explained message with the raw text in small print", () => {
    const { dialog } = render("AI_HTTP_STATUS_429: slow down");
    expect(dialog.textContent).toContain(t("turnFailedTitle"));
    expect(dialog.textContent).toContain(t("errQuotaApi"));
    expect(dialog.querySelector("small")?.textContent).toContain("slow down");
    expect(dialog.querySelector("textarea")).toBeNull();
  });

  it("falls back to the raw text when nothing matches", () => {
    const { dialog } = render("disk on fire");
    expect(dialog.querySelector("[role=alert]")?.textContent).toBe("disk on fire");
    expect(dialog.querySelector("small")).toBeNull();
  });

  it("keeps the player's draft as read-only selectable text", () => {
    const { dialog } = render("AI_CALL_FAILED: boom", "  我推開門  ");
    const draft = dialog.querySelector("textarea")!;
    expect(draft.readOnly).toBe(true);
    expect(draft.value).toBe("  我推開門  ");
    expect(dialog.textContent).toContain(t("turnFailedDraftLabel"));
  });

  it("only has a close button, focused on open; Esc closes, the backdrop does not", () => {
    const { dialog, onClose } = render("boom");
    const buttons = [...dialog.querySelectorAll("button")];
    expect(buttons.map((button) => button.textContent)).toEqual([t("closeBtn")]);
    expect(document.activeElement).toBe(buttons[0]);

    act(() => {
      dialog.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: -10, clientY: -10 }));
      dialog.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: -10, clientY: -10 }));
    });
    expect(onClose).not.toHaveBeenCalled();

    act(() => {
      dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    });
    expect(onClose).toHaveBeenCalledTimes(1);

    act(() => buttons[0].click());
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});

describe("TurnFailedDialog 換幕鈕", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  function show(raw: string, onAdvanceScene?: () => void) {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    act(() =>
      root.render(
        <TurnFailedDialog failure={{ raw }} transport="claude" onClose={() => {}} onAdvanceScene={onAdvanceScene} />,
      ),
    );
    const advance = Array.from(document.querySelectorAll("dialog button")).find(
      (node) => node.textContent === t("sceneAdvance"),
    ) as HTMLButtonElement | undefined;
    return { root, advance };
  }

  it("這一幕太長、換幕容量已滿：附換幕鈕；其他錯誤與沒傳動作都不附", () => {
    const onAdvanceScene = vi.fn();
    const tooLong = show("AI_CONTEXT_TOO_LONG: Prompt is too long", onAdvanceScene);
    act(() => tooLong.advance!.click());
    expect(onAdvanceScene).toHaveBeenCalledTimes(1);
    act(() => tooLong.root.unmount());
    const full = show('TTMSG:{"code":"scene_capacity_full"}', onAdvanceScene);
    expect(full.advance).toBeDefined();
    act(() => full.root.unmount());
    const other = show("AI_HTTP_STATUS_429: slow", onAdvanceScene);
    expect(other.advance).toBeUndefined();
    act(() => other.root.unmount());
    const summaryFailed = show('TTMSG:{"code":"scene_summary_failed"}', onAdvanceScene);
    expect(summaryFailed.advance).toBeUndefined();
    act(() => summaryFailed.root.unmount());
    const noAction = show("AI_CONTEXT_TOO_LONG: x");
    expect(noAction.advance).toBeUndefined();
    act(() => noAction.root.unmount());
  });
});

describe("nextTurnFailure", () => {
  it("keeps an open failure that carries the player's draft", () => {
    const withDraft = { raw: "a", draft: "我推開門" };
    expect(nextTurnFailure(withDraft, { raw: "b" })).toBe(withDraft);
    expect(nextTurnFailure(withDraft, { raw: "c", draft: "別的" })).toBe(withDraft);
  });

  it("replaces a failure without a draft, or opens a fresh one", () => {
    const next = { raw: "b", draft: "x" };
    expect(nextTurnFailure({ raw: "a" }, next)).toBe(next);
    expect(nextTurnFailure(null, next)).toBe(next);
  });
});
