// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MoreMenu, type MoreMenuItem } from "./MoreMenu";

// happy-dom 不排版，這裡只驗行為與焦點；彈層的幾何（夾限、內捲）由實機驗收
describe("MoreMenu", () => {
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

  function mount(items: MoreMenuItem[], onOpen?: () => void) {
    host = document.createElement("div");
    document.body.appendChild(host);
    host.innerHTML = "";
    root = createRoot(host);
    act(() => {
      root?.render(
        <>
          <button type="button" id="before">
            before
          </button>
          <MoreMenu label="More" items={items} onOpen={onOpen} />
          <button type="button" id="after">
            after
          </button>
        </>,
      );
    });
  }

  const trigger = () => document.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!;
  const menu = () => document.querySelector<HTMLElement>('[role="menu"]');
  const menuItems = () => [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
  const active = () => document.activeElement;

  function press(target: Element, key: string, init: KeyboardEventInit = {}) {
    const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
    act(() => {
      target.dispatchEvent(event);
    });
    return event;
  }

  function openMenu() {
    act(() => {
      trigger().click();
    });
  }

  function threeItems() {
    const exportFn = vi.fn();
    const disabledFn = vi.fn();
    const deleteFn = vi.fn();
    const items: MoreMenuItem[] = [
      { key: "export", label: "Export", onSelect: exportFn },
      { key: "off", label: "Off", onSelect: disabledFn, disabled: true },
      { key: "delete", label: "Delete", onSelect: deleteFn, danger: true },
    ];
    return { items, exportFn, disabledFn, deleteFn };
  }

  it("opens on click, reflects state on the trigger, focuses the first item, and portals to body", () => {
    const onOpen = vi.fn();
    mount(threeItems().items, onOpen);
    expect(trigger().getAttribute("aria-expanded")).toBe("false");
    expect(menu()).toBeNull();

    openMenu();
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(trigger().getAttribute("aria-expanded")).toBe("true");
    expect(menu()?.parentElement).toBe(document.body);
    expect(trigger().getAttribute("aria-controls")).toBe(menu()?.id);
    expect(active()).toBe(menuItems()[0]);
  });

  it("separates danger items and marks disabled items with aria-disabled while keeping them focusable", () => {
    mount(threeItems().items);
    openMenu();
    expect(menu()?.querySelector("hr")).not.toBeNull();
    const [, off, del] = menuItems();
    expect(off.getAttribute("aria-disabled")).toBe("true");
    expect(off.disabled).toBe(false);
    expect(del.className).toContain("menu-item-danger");
  });

  it("cycles focus with arrows, Home and End, including disabled items", () => {
    mount(threeItems().items);
    openMenu();
    const [first, second, third] = menuItems();
    press(active()!, "ArrowDown");
    expect(active()).toBe(second);
    press(active()!, "ArrowDown");
    expect(active()).toBe(third);
    press(active()!, "ArrowDown");
    expect(active()).toBe(first);
    press(active()!, "ArrowUp");
    expect(active()).toBe(third);
    press(active()!, "Home");
    expect(active()).toBe(first);
    press(active()!, "End");
    expect(active()).toBe(third);
  });

  it("does not run disabled items by keyboard or click and stays open", () => {
    const { items, disabledFn } = threeItems();
    mount(items);
    openMenu();
    const off = menuItems()[1];
    act(() => off.focus());
    press(off, "Enter");
    press(off, " ");
    act(() => off.click());
    expect(disabledFn).not.toHaveBeenCalled();
    expect(menu()).not.toBeNull();
  });

  it.each(["Enter", " "])("%j runs the item, closes, and returns focus to the trigger", (key) => {
    const { items, exportFn } = threeItems();
    mount(items);
    openMenu();
    const event = press(menuItems()[0], key);
    expect(event.defaultPrevented).toBe(true);
    expect(exportFn).toHaveBeenCalledTimes(1);
    expect(menu()).toBeNull();
    expect(active()).toBe(trigger());
  });

  it("lets a dialog opened by the item keep focus", () => {
    const dialogButton = document.createElement("button");
    const items: MoreMenuItem[] = [
      {
        key: "dialog",
        label: "Open dialog",
        onSelect: () => {
          document.body.appendChild(dialogButton);
          dialogButton.focus();
        },
      },
    ];
    mount(items);
    openMenu();
    press(menuItems()[0], "Enter");
    expect(menu()).toBeNull();
    expect(active()).toBe(dialogButton);
  });

  it("Escape closes and returns focus to the trigger", () => {
    mount(threeItems().items);
    openMenu();
    press(menuItems()[1], "Escape");
    expect(menu()).toBeNull();
    expect(active()).toBe(trigger());
  });

  it("an outside pointerdown closes without stealing focus from what was clicked", () => {
    mount(threeItems().items);
    openMenu();
    const outside = document.getElementById("after") as HTMLButtonElement;
    act(() => {
      outside.focus();
      outside.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    });
    expect(menu()).toBeNull();
    expect(active()).toBe(outside);
  });

  it("clicking the trigger again closes the menu", () => {
    mount(threeItems().items);
    openMenu();
    act(() => {
      trigger().dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      trigger().click();
    });
    expect(menu()).toBeNull();
  });

  it.each([false, true])(
    "Tab (shift=%s) hands focus to the trigger without blocking the default move, then unmounts",
    async (shiftKey) => {
      mount(threeItems().items);
      openMenu();
      const event = press(menuItems()[0], "Tab", { shiftKey });
      expect(event.defaultPrevented).toBe(false);
      expect(active()).toBe(trigger());
      // 預設移焦發生前選單還在，下一個 tick 才卸載
      expect(menu()).not.toBeNull();
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
      expect(menu()).toBeNull();
      expect(trigger().getAttribute("aria-expanded")).toBe("false");
    },
  );
});
