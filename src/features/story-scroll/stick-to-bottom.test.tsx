// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useStickToBottom } from "./useStickToBottom";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let fireResize: () => void = () => {};

function List({ onReady }: { onReady?: (mark: () => void) => void }) {
  const ref = (globalThis as { __ref?: { current: HTMLElement | null } }).__ref!;
  const mark = useStickToBottom(ref);
  onReady?.(mark);
  return <section ref={ref} />;
}

describe("useStickToBottom", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  let el: HTMLElement;
  let content = 1000;

  beforeEach(() => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(cb: () => void) {
          fireResize = cb;
        }
        observe() {}
        disconnect() {}
      },
    );
    (globalThis as { __ref?: unknown }).__ref = { current: null };
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root?.render(<List />));
    el = (globalThis as unknown as { __ref: { current: HTMLElement } }).__ref.current;
    content = 1000;
    // happy-dom 不排版：scrollHeight／clientHeight 自己給，scrollTop 是普通屬性
    Object.defineProperty(el, "scrollHeight", { get: () => content, configurable: true });
    Object.defineProperty(el, "clientHeight", { value: 400, configurable: true });
    el.scrollTop = 600;
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    vi.unstubAllGlobals();
  });

  const scrolled = () => act(() => void el.dispatchEvent(new Event("scroll")));

  it("keeps the bottom after a resize when it was at the bottom", () => {
    scrolled();
    content = 1300; // 欄寬變窄、文字多折幾行
    fireResize();
    expect(el.scrollTop).toBe(1300);
  });

  it("leaves the position alone once the reader scrolled up", () => {
    el.scrollTop = 200;
    scrolled();
    content = 1300;
    fireResize();
    expect(el.scrollTop).toBe(200);
  });

  it("sticks again after scrolling back to the bottom", () => {
    el.scrollTop = 200;
    scrolled();
    el.scrollTop = 600;
    scrolled();
    content = 1100;
    fireResize();
    expect(el.scrollTop).toBe(1100);
  });
});
