// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

  function mount(
    items: MoreMenuItem[],
    onOpen?: () => void,
    extra: Partial<React.ComponentProps<typeof MoreMenu>> = {},
  ) {
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
          <MoreMenu label="More" items={items} onOpen={onOpen} {...extra} />
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

  it("renders a custom trigger with its own class and drops the aria-label when the text names it", () => {
    mount(threeItems().items, undefined, {
      trigger: <span>Add</span>,
      className: "btn rail-add",
      ariaLabelTrigger: false,
    });
    expect(trigger().className).toBe("btn rail-add");
    expect(trigger().textContent).toBe("Add");
    expect(trigger().hasAttribute("aria-label")).toBe(false);
    expect(trigger().title).toBe("More");
    openMenu();
    expect(menu()?.getAttribute("aria-label")).toBe("More");
  });

  it("closes an open menu when the trigger becomes disabled", () => {
    const { items } = threeItems();
    mount(items);
    openMenu();
    expect(menu()).not.toBeNull();
    act(() => {
      root?.render(<MoreMenu label="More" items={items} disabled />);
    });
    expect(menu()).toBeNull();
    expect(trigger().disabled).toBe(true);
  });

  describe("placement", () => {
    const originalHeight = window.innerHeight;
    let rectOf: ReturnType<typeof vi.spyOn>;
    let scrollHeightOf: ReturnType<typeof vi.spyOn>;

    beforeEach(() => {
      rectOf = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect");
      scrollHeightOf = vi.spyOn(HTMLElement.prototype, "scrollHeight", "get");
    });

    afterEach(() => {
      rectOf.mockRestore();
      scrollHeightOf.mockRestore();
      Object.defineProperty(window, "innerHeight", { value: originalHeight, configurable: true });
    });

    function layout(triggerTop: number, menuHeight: number) {
      Object.defineProperty(window, "innerHeight", { value: 800, configurable: true });
      rectOf.mockImplementation(function (this: HTMLElement) {
        const isTrigger = this.getAttribute("aria-haspopup") === "menu";
        return (isTrigger
          ? { top: triggerTop, bottom: triggerTop + 30, left: 100, right: 130, width: 30, height: 30 }
          : { top: 0, bottom: 0, left: 0, right: 0, width: 0, height: 0 }) as DOMRect;
      });
      scrollHeightOf.mockImplementation(function (this: HTMLElement) {
        return this.getAttribute("role") === "menu" ? menuHeight : 0;
      });
    }

    it("opens downward when there is room", () => {
      layout(100, 120);
      mount(threeItems().items);
      openMenu();
      expect(menu()?.style.top).toBe("134px");
      expect(menu()?.style.maxHeight).toBe("658px");
    });

    it("opens upward and scrolls within the space above when below is too small", () => {
      layout(740, 300);
      mount(threeItems().items);
      openMenu();
      // 觸發鈕上方剩 740-4-8＝728，放得下整份選單：貼著觸發鈕上緣往上長
      expect(menu()?.style.top).toBe("436px");
      expect(menu()?.style.maxHeight).toBe("728px");
    });

    it("keeps opening downward when the space above is no better", () => {
      layout(30, 900);
      mount(threeItems().items);
      openMenu();
      expect(menu()?.style.top).toBe("64px");
      expect(menu()?.style.maxHeight).toBe("728px");
    });
  });
});
