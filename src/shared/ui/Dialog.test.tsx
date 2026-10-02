// @vitest-environment happy-dom

import { act, StrictMode, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Dialog, ModalShell } from "./Dialog";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// happy-dom 的 showModal 只設 open、沒有 top layer／inert，close() 同步發 close 事件：
// 這裡只驗狀態機（Esc、遮罩判定、重開、焦點），疊層與 Tab 圈限由實機驗收
describe("ModalShell / Dialog", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  function render(node: ReactNode) {
    if (!host) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    act(() => {
      root!.render(node);
    });
  }

  // 焦點歸還排在 commit 後的 microtask
  async function settle() {
    await act(async () => {
      await Promise.resolve();
    });
  }

  const dialogs = () => [...document.querySelectorAll("dialog")];
  const dialog = () => document.querySelector("dialog")!;

  function press(target: Element, key: string, init: KeyboardEventInit = {}) {
    const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
    act(() => {
      target.dispatchEvent(event);
    });
    return event;
  }

  // happy-dom 的框是 0×0：負座標＝框外，框內就把 getBoundingClientRect 撐大再點
  function pointer(down: Element, up: Element, at = { clientX: -10, clientY: -10 }) {
    act(() => {
      down.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, ...at }));
      up.dispatchEvent(new MouseEvent("click", { bubbles: true, ...at }));
    });
  }

  describe("Esc", () => {
    it("calls onDismiss and is cancelled", () => {
      const onDismiss = vi.fn();
      render(
        <Dialog title="T" onDismiss={onDismiss}>
          <button type="button">inside</button>
        </Dialog>,
      );
      const event = press(dialog().querySelector("button")!, "Escape");
      expect(onDismiss).toHaveBeenCalledTimes(1);
      expect(event.defaultPrevented).toBe(true);
    });

    it("does nothing without onDismiss, and the dialog stays open", () => {
      render(<Dialog title="T">body</Dialog>);
      press(dialog(), "Escape");
      expect(dialog().open).toBe(true);
    });

    it("never reaches window listeners", () => {
      const onWindow = vi.fn();
      window.addEventListener("keydown", onWindow);
      render(<Dialog title="T" onDismiss={() => {}}>body</Dialog>);
      press(dialog(), "Escape");
      window.removeEventListener("keydown", onWindow);
      expect(onWindow).not.toHaveBeenCalled();
    });

    it("while composing only stops propagation", () => {
      const onDismiss = vi.fn();
      const onWindow = vi.fn();
      window.addEventListener("keydown", onWindow);
      render(<Dialog title="T" onDismiss={onDismiss}>body</Dialog>);
      const event = press(dialog(), "Escape", { isComposing: true });
      window.removeEventListener("keydown", onWindow);
      expect(onDismiss).not.toHaveBeenCalled();
      expect(onWindow).not.toHaveBeenCalled();
      expect(event.defaultPrevented).toBe(false);
    });

    it("a nested dialog handles Esc alone", () => {
      const outer = vi.fn();
      const inner = vi.fn();
      render(
        <Dialog title="Outer" onDismiss={outer}>
          <Dialog title="Inner" onDismiss={inner}>
            <button type="button">inner</button>
          </Dialog>
        </Dialog>,
      );
      press(dialogs()[1].querySelector("button")!, "Escape");
      expect(inner).toHaveBeenCalledTimes(1);
      expect(outer).not.toHaveBeenCalled();
    });
  });

  describe("backdrop", () => {
    it("dismisses only when both press and release land outside the frame", () => {
      const onDismiss = vi.fn();
      render(
        <Dialog title="T" onDismiss={onDismiss} backdrop>
          <p>body</p>
        </Dialog>,
      );
      pointer(dialog(), dialog());
      expect(onDismiss).toHaveBeenCalledTimes(1);
    });

    it("pressing inside and releasing outside does not dismiss", () => {
      const onDismiss = vi.fn();
      render(
        <Dialog title="T" onDismiss={onDismiss} backdrop>
          <p>body</p>
        </Dialog>,
      );
      pointer(dialog().querySelector("p")!, dialog());
      expect(onDismiss).not.toHaveBeenCalled();
    });

    it("a click on the dialog's own padding inside the frame does not dismiss", () => {
      const onDismiss = vi.fn();
      render(
        <Dialog title="T" onDismiss={onDismiss} backdrop>
          body
        </Dialog>,
      );
      vi.spyOn(dialog(), "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 100, 100));
      pointer(dialog(), dialog(), { clientX: 50, clientY: 50 });
      expect(onDismiss).not.toHaveBeenCalled();
      pointer(dialog(), dialog(), { clientX: 150, clientY: 50 });
      expect(onDismiss).toHaveBeenCalledTimes(1);
    });

    it("does nothing without backdrop, or without onDismiss", () => {
      const onDismiss = vi.fn();
      render(<Dialog title="T" onDismiss={onDismiss}>body</Dialog>);
      pointer(dialog(), dialog());
      render(<Dialog title="T" backdrop>body</Dialog>);
      pointer(dialog(), dialog());
      expect(onDismiss).not.toHaveBeenCalled();
      expect(dialog().open).toBe(true);
    });
  });

  describe("open state", () => {
    it("opens on mount and closes on unmount without reopening", () => {
      render(<ModalShell label="L">x</ModalShell>);
      const element = dialog();
      expect(element.open).toBe(true);
      const showModal = vi.spyOn(element, "showModal");
      const close = vi.spyOn(element, "close");
      act(() => {
        root!.unmount();
      });
      root = null;
      host = null;
      expect(close).toHaveBeenCalledTimes(1);
      expect(showModal).not.toHaveBeenCalled();
      expect(element.open).toBe(false);
    });

    it("reopens after a close it did not ask for", () => {
      render(<ModalShell label="L">x</ModalShell>);
      act(() => {
        dialog().close();
      });
      expect(dialog().open).toBe(true);
    });

    it("always cancels the native cancel event", () => {
      render(<ModalShell label="L">x</ModalShell>);
      const event = new Event("cancel", { cancelable: true });
      dialog().dispatchEvent(event);
      expect(event.defaultPrevented).toBe(true);
    });

    it("stays open through the StrictMode remount", () => {
      render(
        <StrictMode>
          <ModalShell label="L">x</ModalShell>
        </StrictMode>,
      );
      expect(dialog().open).toBe(true);
      act(() => {
        dialog().close();
      });
      expect(dialog().open).toBe(true);
    });
  });

  describe("focus", () => {
    it("starts on [data-autofocus] when present", () => {
      render(
        <Dialog title="T" end={<button type="button">ok</button>}>
          <input data-autofocus="" />
        </Dialog>,
      );
      expect(document.activeElement).toBe(dialog().querySelector("input"));
    });

    it("otherwise starts on the content area, not a button", () => {
      render(
        <Dialog title="T" closeButton onDismiss={() => {}} end={<button type="button">ok</button>}>
          <button type="button">inside</button>
        </Dialog>,
      );
      expect(document.activeElement).toBe(dialog().querySelector(".dialog-body"));
    });

    function Page({ open, triggerDisabled = false, inert = false }: {
      open: boolean;
      triggerDisabled?: boolean;
      inert?: boolean;
    }) {
      return (
        <>
          <div inert={inert}>
            <button type="button" id="trigger" disabled={triggerDisabled}>
              open
            </button>
          </div>
          {open && <Dialog title="T">body</Dialog>}
        </>
      );
    }

    const trigger = () => document.getElementById("trigger") as HTMLButtonElement;

    function openFromTrigger(props: Partial<Parameters<typeof Page>[0]> = {}) {
      render(<Page open={false} {...props} />);
      trigger().focus();
      render(<Page open {...props} />);
      expect(document.activeElement).not.toBe(trigger());
    }

    it("returns to the trigger on close", async () => {
      openFromTrigger();
      render(<Page open={false} />);
      await settle();
      expect(document.activeElement).toBe(trigger());
    });

    it("does not return to a trigger that became disabled", async () => {
      openFromTrigger();
      // 看有沒有呼叫 focus：happy-dom 本身也會擋，只看 activeElement 驗不出判斷
      const focus = vi.spyOn(trigger(), "focus");
      render(<Page open={false} triggerDisabled />);
      await settle();
      expect(focus).not.toHaveBeenCalled();
    });

    it("does not return into an inert region", async () => {
      openFromTrigger();
      // 看有沒有呼叫 focus：happy-dom 本身也會擋，只看 activeElement 驗不出判斷
      const focus = vi.spyOn(trigger(), "focus");
      render(<Page open={false} inert />);
      await settle();
      expect(focus).not.toHaveBeenCalled();
    });

    it("does not return to a trigger that is gone", async () => {
      function Gone({ open, showTrigger }: { open: boolean; showTrigger: boolean }) {
        return (
          <>
            {showTrigger && (
              <button type="button" id="trigger">
                open
              </button>
            )}
            {open && <Dialog title="T">body</Dialog>}
          </>
        );
      }
      render(<Gone open={false} showTrigger />);
      const gone = trigger();
      gone.focus();
      render(<Gone open showTrigger />);
      const focus = vi.spyOn(gone, "focus");
      render(<Gone open showTrigger={false} />);
      render(<Gone open={false} showTrigger={false} />);
      await settle();
      expect(focus).not.toHaveBeenCalled();
    });

    it("with another dialog still open, only returns inside the top one", async () => {
      function Stack({ outer, inner, from }: {
        outer: boolean;
        inner: boolean;
        from: "page" | "outer";
      }) {
        return (
          <>
            <button type="button" id="page-trigger">
              page
            </button>
            {outer && (
              <Dialog title="Outer">
                <button type="button" id="outer-trigger">
                  outer
                </button>
                {inner && from === "outer" && <Dialog title="Inner">inner</Dialog>}
              </Dialog>
            )}
            {inner && from === "page" && <Dialog title="Inner">inner</Dialog>}
          </>
        );
      }
      const pageTrigger = () => document.getElementById("page-trigger")!;
      const outerTrigger = () => document.getElementById("outer-trigger")!;

      render(<Stack outer inner={false} from="outer" />);
      outerTrigger().focus();
      render(<Stack outer inner from="outer" />);
      render(<Stack outer inner={false} from="outer" />);
      await settle();
      expect(document.activeElement).toBe(outerTrigger());

      render(<Stack outer={false} inner={false} from="page" />);
      render(<Stack outer inner={false} from="page" />);
      pageTrigger().focus();
      render(<Stack outer inner from="page" />);
      render(<Stack outer inner={false} from="page" />);
      await settle();
      expect(document.activeElement).not.toBe(pageTrigger());
    });
  });

  it("the × uses onDismiss and is disabled without it", () => {
    const onDismiss = vi.fn();
    render(
      <Dialog title="T" closeButton onDismiss={onDismiss}>
        body
      </Dialog>,
    );
    const close = () => dialog().querySelector<HTMLButtonElement>(".dialog-close")!;
    act(() => {
      close().click();
    });
    expect(onDismiss).toHaveBeenCalledTimes(1);
    render(
      <Dialog title="T" closeButton>
        body
      </Dialog>,
    );
    expect(close().disabled).toBe(true);
  });
});
