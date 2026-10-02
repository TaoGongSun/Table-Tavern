// 真實 WebKit（Tauri macOS 的引擎）才看得到的焦點可見性：npm run test:webkit。
// 點擊、移游標、按鍵一律經 provider 送真輸入，不用 dispatchEvent／DOM click。
// 每個案例先用滑鼠點文字框：WebKit 記得「上次焦點來自滑鼠」時，程式移焦不會符合 :focus-visible，
// 這是 menu-keyboard-webkit 的觸發條件（見 .ai/plans/menu-keyboard-webkit.md）。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { userEvent } from "vitest/browser";
import "../../App.css";
import { MoreMenu, type MoreMenuItem } from "./MoreMenu";

let root: Root | null = null;
let host: HTMLDivElement | null = null;
let noTransition: HTMLStyleElement | null = null;
let selected: string[] = [];

beforeEach(async () => {
  selected = [];
  // 反白有 transition，量到一半會是半透明；測的是終值
  noTransition = document.createElement("style");
  noTransition.textContent = "*, *::before, *::after { transition: none !important; }";
  document.head.appendChild(noTransition);
  host = document.createElement("div");
  host.style.padding = "40px";
  document.body.appendChild(host);
  root = createRoot(host);
  const items: MoreMenuItem[] = ["A", "B", "C", "D"].map((key) => ({
    key,
    label: `Item ${key}`,
    disabled: key === "C",
    onSelect: () => selected.push(key),
  }));
  await act(async () => {
    root!.render(
      <>
        <textarea aria-label="notes" />
        <MoreMenu label="More" items={items} />
        <button type="button">after</button>
      </>,
    );
  });
  await userEvent.click(textarea());
});

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  noTransition?.remove();
  vi.restoreAllMocks();
});

const textarea = () => document.querySelector("textarea")!;
const trigger = () => document.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!;
const item = (key: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find(
    (node) => node.textContent === `Item ${key}`,
  )!;
const allItems = () => [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
const activeLabel = () => document.activeElement?.textContent;
const transparent = (color: string) =>
  color === "transparent" || /^rgba\(0, 0, 0, 0\)$/.test(color) || /\/ 0\)$/.test(color);
const highlighted = (node: Element) => !transparent(getComputedStyle(node).backgroundColor);
const ringed = (node: Element) => getComputedStyle(node).outlineStyle !== "none";
const litItems = () => allItems().filter(highlighted).map((node) => node.textContent);

async function press(key: string) {
  await userEvent.keyboard(`{${key}}`);
}

async function openByMouse() {
  await userEvent.click(trigger());
  await vi.waitFor(() => expect(activeLabel()).toBe("Item A"));
}

describe("MoreMenu on WebKit", () => {
  it("mouse open after a mouse-focused field: first item focused and highlighted", async () => {
    await openByMouse();
    expect(litItems()).toEqual(["Item A"]);
  });

  it("arrow keys after a mouse open show a visible focus on the moved-to item", async () => {
    await openByMouse();
    await press("ArrowDown");
    expect(activeLabel()).toBe("Item B");
    expect(litItems()).toEqual(["Item B"]);
    expect(ringed(item("B"))).toBe(true);
    await press("ArrowUp");
    await press("ArrowUp");
    expect(activeLabel()).toBe("Item D");
    expect(litItems()).toEqual(["Item D"]);
    expect(ringed(item("D"))).toBe(true);
  });

  it("Escape returns a visible focus to the trigger", async () => {
    await openByMouse();
    await press("ArrowDown");
    await press("Escape");
    expect(document.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger());
    expect(ringed(trigger())).toBe(true);
  });

  it("keeps the focus ring when the engine ignores the focusVisible option", async () => {
    const native = HTMLElement.prototype.focus;
    vi.spyOn(HTMLElement.prototype, "focus").mockImplementation(function (
      this: HTMLElement,
      options?: FocusOptions,
    ) {
      native.call(this, options ? { preventScroll: options.preventScroll } : undefined);
    });
    await openByMouse();
    await press("ArrowDown");
    expect(activeLabel()).toBe("Item B");
    expect(ringed(item("B"))).toBe(true);
    await press("Enter");
    expect(selected).toEqual(["B"]);
    expect(document.activeElement).toBe(trigger());
    expect(ringed(trigger())).toBe(true);
  });

  it("a resting cursor does not leave a second highlight while the keyboard moves on", async () => {
    await openByMouse();
    await userEvent.hover(item("B"));
    expect(activeLabel()).toBe("Item B");
    await press("ArrowDown");
    expect(activeLabel()).toBe("Item C");
    expect(litItems()).toEqual(["Item C"]);
  });

  it("the mouse takes over and arrows continue from the hovered item", async () => {
    await openByMouse();
    await press("ArrowDown");
    await userEvent.hover(item("D"));
    expect(activeLabel()).toBe("Item D");
    expect(litItems()).toEqual(["Item D"]);
    expect(ringed(item("D"))).toBe(false);
    await press("ArrowDown");
    expect(activeLabel()).toBe("Item A");
    expect(ringed(item("A"))).toBe(true);
  });

  it("mouse-selected item returns focus to the trigger without a ring", async () => {
    await openByMouse();
    await press("ArrowDown");
    await userEvent.click(item("D"));
    expect(selected).toEqual(["D"]);
    expect(document.activeElement).toBe(trigger());
    expect(ringed(trigger())).toBe(false);
  });

  it("keyboard open puts a visible focus on the first item", async () => {
    // macOS WebKit 預設 Tab 不停在按鈕上（系統「鍵盤導覽」關閉時），直接把焦點放到觸發鈕
    act(() => trigger().focus());
    await press("Enter");
    await vi.waitFor(() => expect(activeLabel()).toBe("Item A"));
    expect(ringed(item("A"))).toBe(true);
    expect(litItems()).toEqual(["Item A"]);
  });

  it("a disabled item stays focusable but does not run", async () => {
    await openByMouse();
    await press("ArrowDown");
    await press("ArrowDown");
    expect(activeLabel()).toBe("Item C");
    await press("Enter");
    expect(selected).toEqual([]);
    expect(document.querySelector('[role="menu"]')).not.toBeNull();
  });
});
