// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { followVisualViewport } from "./viewport";

describe("followVisualViewport", () => {
  it("版面高度跟著可見區域，鍵盤彈出後更新並拉回頁頂", () => {
    const viewport = Object.assign(new EventTarget(), { height: 714 });
    const scrollTo = vi.fn();
    const win = { visualViewport: viewport, document, scrollY: 0, scrollTo } as unknown as Window;
    followVisualViewport(win);
    expect(document.documentElement.style.getPropertyValue("--app-height")).toBe("714px");
    viewport.height = 377;
    (win as { scrollY: number }).scrollY = 49;
    viewport.dispatchEvent(new Event("resize"));
    expect(document.documentElement.style.getPropertyValue("--app-height")).toBe("377px");
    expect(scrollTo).toHaveBeenCalledWith(0, 0);
  });
});
