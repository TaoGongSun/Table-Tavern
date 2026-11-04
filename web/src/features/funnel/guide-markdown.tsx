// 玩法說明的 Markdown 只用到一小組語法：# 標題、## 段（頂部錨點）、### 小節、- 清單（可縮一層）、> 提示、**粗體**、
// ![](鍵) 截圖、--- 分隔（略過）。內容是自家寫的靜態檔，直接轉成 React 元素，不經 innerHTML。
import { Fragment, type ReactNode } from "react";

export type GuideBlock =
  | { kind: "title"; text: string }
  | { kind: "section"; id: string; text: string }
  | { kind: "sub"; text: string }
  | { kind: "para"; text: string }
  | { kind: "note"; text: string }
  | { kind: "list"; items: { text: string; children: string[] }[] }
  | { kind: "shot"; key: string };

export function parseGuide(source: string): GuideBlock[] {
  const blocks: GuideBlock[] = [];
  let sections = 0;
  for (const raw of source.split("\n")) {
    const line = raw.trimEnd();
    const last = blocks[blocks.length - 1];
    const shot = /^!\[\]\(([\w-]+)\)$/.exec(line);
    if (!line.trim() || line === "---") continue;
    if (line.startsWith("### ")) blocks.push({ kind: "sub", text: line.slice(4) });
    else if (line.startsWith("## ")) blocks.push({ kind: "section", id: `guide-${++sections}`, text: line.slice(3) });
    else if (line.startsWith("# ")) blocks.push({ kind: "title", text: line.slice(2) });
    else if (line.startsWith("> ")) blocks.push({ kind: "note", text: line.slice(2) });
    else if (shot) blocks.push({ kind: "shot", key: shot[1] });
    else if (/^\s+- /.test(line) && last?.kind === "list") last.items[last.items.length - 1].children.push(line.trim().slice(2));
    else if (line.startsWith("- ")) {
      const item = { text: line.slice(2), children: [] };
      if (last?.kind === "list") last.items.push(item);
      else blocks.push({ kind: "list", items: [item] });
    } else blocks.push({ kind: "para", text: line });
  }
  return blocks;
}

/** **粗體** 以外原樣顯示 */
export function inline(text: string): ReactNode {
  return text.split(/\*\*(.+?)\*\*/).map((part, index) => (index % 2 ? <strong key={index}>{part}</strong> : <Fragment key={index}>{part}</Fragment>));
}
