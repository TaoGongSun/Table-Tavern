// zod 卡逐條結果給 GM 的回饋句（mvu-zod-semantics 規格 6.1 節）：桌面（經 QuickJS bundle，Rust 只映射成記錄）與
// 網頁共用這一份，兩邊句子一字不差。上限同桌面 mechanism/note.rs 的 build_notes：同文去重、最多 5 條、每條 240 碼點。
import type { ZodCommandResult } from "./host-contract";

/** 一輪最多回饋幾條 */
export const NOTES_MAX = 5;
/** 一條回饋句的硬上限（Unicode 碼點） */
export const NOTE_MAX = 240;

/** 路徑寫法同 `_.set` 的路徑：鍵名含點、括號、引號（或是空字串）的段寫成 `["…"]` */
function pathText(segments: string[]): string {
  if (segments.length === 0) return "整張表";
  return segments
    .map((segment, index) => {
      if (/^[^.[\]"'\\]+$/.test(segment)) return index === 0 ? segment : `.${segment}`;
      return `[${JSON.stringify(segment)}]`;
    })
    .join("");
}

/** 路徑裡第一個以 _ 開頭的段（上游任一段以 _ 開頭就整條唯讀） */
function readonlySegment(segments: string[]): string {
  return segments.find((item) => item.startsWith("_")) ?? "_";
}

function issuesText(result: ZodCommandResult): string {
  const issues = result.issues ?? [];
  const more = result.issueCount !== undefined && result.issueCount > issues.length ? `（共 ${result.issueCount} 則）` : "";
  return issues.join("；") + more;
}

/** 一條結果的回饋句；不必回饋（applied、上游靜默且本 app 也不回饋的 ignored）回 null */
export function zodFeedbackLine(result: ZodCommandResult): string | null {
  const path = pathText(result.path);
  switch (result.status) {
    case "applied":
      return null;
    case "no-change":
      return `${path} 的 ${result.op} 通過了卡片 schema，但值沒有變（schema 沒列的鍵會被剝掉、被刪的會補回預設值），沒有寫入。`;
    case "rejected":
      if (result.reason === "add-not-number") return `${path} 的 add 沒有套用、維持原值：${issuesText(result)}`;
      if (result.reason === "move-source-missing") return `move 的來源 ${path} 不存在，沒有搬。`;
      if (result.reason === "move-source-readonly") return `move 的來源 ${path} 裡的「${readonlySegment(result.path)}」以 _ 開頭，是唯讀欄位，沒有搬。`;
      if (result.reason?.startsWith("move-source-")) return `move 的來源 ${path} 刪不掉、沒有搬：${issuesText(result)}`;
      return `${path} 的 ${result.op} 被卡片 schema 拒收、維持原值：${issuesText(result)}`;
    case "ignored": {
      if (result.reason !== "readonly") return null;
      // move 被擋在目的地（來源是唯讀時是 rejected／move-source-readonly）：指出目的地，GM 才不會去改來源
      if (result.op === "move" && result.to) {
        return `move 的目的地 ${pathText(result.to)} 裡的「${readonlySegment(result.to)}」以 _ 開頭，是唯讀欄位，沒有搬。`;
      }
      return `${path} 裡的「${readonlySegment(result.path)}」以 _ 開頭，是唯讀欄位，這條 ${result.op} 沒有套用。`;
    }
  }
}

/** 超過上限截到 max−1 個碼點補「…」（同桌面 clip_end） */
function clip(text: string, max: number): string {
  const points = Array.from(text);
  return points.length <= max ? text : points.slice(0, max - 1).join("") + "…";
}

/** 整批結果 → 給下一輪 GM 的回饋：依結果順序、同文去重、最多 5 條、每條截到 240 碼點 */
export function zodNotes(results: ZodCommandResult[]): string[] {
  const slots: string[] = [];
  for (const result of results) {
    const line = zodFeedbackLine(result);
    if (line !== null && !slots.includes(line) && slots.length < NOTES_MAX) slots.push(line);
  }
  const notes: string[] = [];
  for (const slot of slots) {
    const note = clip(slot, NOTE_MAX);
    if (!notes.includes(note)) notes.push(note);
  }
  return notes;
}
