// 訊息裡的「前端介面」區塊：照酒館助手的渲染器，程式碼區塊的內容含 `html>`、`<head>` 或 `<body` 就畫成
// 沙盒 iframe，其餘照一般 Markdown 顯示（JS-Slash-Runner `util/is_frontend.ts` 的判斷，只當規格書讀）。
// 一則訊息切成依序的片段：文字片段走宿主的安全渲染，前端片段交給 CardFrontend。

export type MessageSegment = { kind: "text"; text: string } | { kind: "frontend"; html: string };

// 圍欄區塊：開頭 ``` 後可帶語言標記，內容到下一個 ```（同桌面版 interface-card 的圍欄寫法）
const FENCE = /```([^\r\n`]*)\r?\n([\s\S]*?)```/g;

export function isFrontend(content: string): boolean {
  return ["html>", "<head>", "<body"].some((tag) => content.includes(tag));
}

export function splitFrontends(text: string): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let last = 0;
  FENCE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = FENCE.exec(text)) !== null) {
    if (!isFrontend(match[2])) continue;
    const before = text.slice(last, match.index);
    if (before.trim() !== "") segments.push({ kind: "text", text: before });
    segments.push({ kind: "frontend", html: match[2] });
    last = match.index + match[0].length;
  }
  const rest = text.slice(last);
  if (rest.trim() !== "" || segments.length === 0) segments.push({ kind: "text", text: rest });
  return segments;
}
