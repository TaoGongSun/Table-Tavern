// @vitest-environment happy-dom
// 破圖收斂墊片（verify 會跑的那份）：error 標上 data-tt-broken、load 移除，只動自己的標記；樣式注入一次。
// 真實 WebKit 沙盒裡的完整行為另見 broken-image.webkit.test.tsx。
import { afterEach, describe, expect, it } from "vitest";
import { buildBrokenImageSource } from "./interface-card";

afterEach(() => {
  document.head.innerHTML = "";
  document.body.innerHTML = "";
});

describe("buildBrokenImageSource", () => {
  it("圖片 error 加標記、之後 load 移除；非圖片不動；卡片自己的屬性不碰", () => {
    new Function("document", buildBrokenImageSource())(document);
    expect(document.head.querySelector("style")?.textContent).toBe("img[data-tt-broken]{visibility:hidden!important}");
    const img = document.createElement("img");
    img.setAttribute("data-card", "own");
    const div = document.createElement("div");
    document.body.append(img, div);
    img.dispatchEvent(new Event("error"));
    div.dispatchEvent(new Event("error"));
    expect(img.hasAttribute("data-tt-broken")).toBe(true);
    expect(div.hasAttribute("data-tt-broken")).toBe(false);
    img.dispatchEvent(new Event("load"));
    expect(img.hasAttribute("data-tt-broken")).toBe(false);
    expect(img.getAttribute("data-card")).toBe("own");
  });
});
