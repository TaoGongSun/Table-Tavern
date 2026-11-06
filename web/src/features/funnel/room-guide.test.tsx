// @vitest-environment happy-dom
// 玩法說明：Markdown 子集解析、三語內容都有三段且截圖鍵對得上、頁面依語系挑內容與截圖（缺圖不畫）。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import { parseGuide } from "./guide-markdown";
import { RoomGuide } from "./RoomGuide";

const GUIDES = import.meta.glob<string>("./guide/room.*.md", { eager: true, query: "?raw", import: "default" });

let root: Root | null = null;

afterEach(() => {
  act(() => root?.unmount());
  root = null;
  document.body.innerHTML = "";
  setLang("zh-TW");
});

describe("玩法說明", () => {
  it("解析標題、段、小節、兩層清單、提示與截圖", () => {
    const blocks = parseGuide("# 標題\n\n## 房主\n### 1. 開房\n![](room-01)\n- 第一條 **粗**\n  - 子項\n- 第二條\n> 提醒\n---\n一般段落");
    expect(blocks.map((b) => b.kind)).toEqual(["title", "section", "sub", "shot", "list", "note", "para"]);
    expect(blocks[4]).toEqual({ kind: "list", items: [{ text: "第一條 **粗**", children: ["子項"] }, { text: "第二條", children: [] }] });
  });

  it("三語內容都有房主、朋友、常見問題三段，每節留一張截圖", () => {
    const shotsKept = ["02", "03", "04", "06", "08", "10", "11", "12", "13", "15", "17", "19"].map((n) => `room-${n}`);
    for (const lang of ["zh-CN", "zh-TW", "en"]) {
      const blocks = parseGuide(GUIDES[`./guide/room.${lang}.md`]);
      expect(blocks.filter((b) => b.kind === "section")).toHaveLength(3);
      expect(blocks.flatMap((b) => (b.kind === "shot" ? [b.key] : []))).toEqual(shotsKept);
    }
  });

  it("依語系挑內容與截圖，其餘語系用英文；沒有的圖不畫", () => {
    setLang("ja");
    root = createRoot(document.body.appendChild(document.createElement("div")));
    act(() => root!.render(<RoomGuide onClose={() => {}} shots={{ "./guide/shots/room-03.en.webp": "/a.webp" }} />));
    expect(document.querySelector("h1")!.textContent).toContain("Playing with friends");
    expect(document.querySelectorAll(".guide-nav a")).toHaveLength(3);
    expect([...document.querySelectorAll(".guide-shot img")].map((img) => img.getAttribute("src"))).toEqual(["/a.webp"]);
    expect(document.querySelector("[data-testid=guide-back]")?.getAttribute("href")).toBe("#download/room");
  });

  it("繁中缺圖時退英文圖，有自己的圖就不用英文圖", () => {
    setLang("zh-TW");
    root = createRoot(document.body.appendChild(document.createElement("div")));
    act(() => root!.render(<RoomGuide onClose={() => {}} shots={{ "./guide/shots/room-02.en.webp": "/en.webp" }} />));
    expect([...document.querySelectorAll(".guide-shot img")].map((img) => img.getAttribute("src"))).toEqual(["/en.webp"]);
    act(() => root!.render(<RoomGuide onClose={() => {}} shots={{ "./guide/shots/room-02.zh-TW.webp": "/tw.webp", "./guide/shots/room-02.en.webp": "/en.webp" }} />));
    expect([...document.querySelectorAll(".guide-shot img")].map((img) => img.getAttribute("src"))).toEqual(["/tw.webp"]);
  });
});
