// 顯示時藏 MVU 的 `<UpdateVariable>`（規則照桌面版 reply_cleanup，共用案例 src/shared/contracts/reply-cleanup/cases.json）。
// 只改畫面：逐字稿、存檔、卡片介面、MVU 解析與送模歷史都照原文。
import { MVU_PLACEHOLDER } from "@desktop/features/card-interface/mvu/card-mvu-shim";

const CLOSED = /<UpdateVariable(?=[\s>])[^>]*>[\s\S]*?<\/UpdateVariable>/gi;
const OPEN = /<UpdateVariable(?=[\s>]|$)/i;
const MARKER = "<updatevariable";

/** 落檔訊息的顯示：剝閉合區塊、沒閉合的切到結尾；切掉的尾巴裡有宿主補的狀態欄占位就補回結尾。 */
export function hideUpdateVariable(text: string): string {
  const closed = text.replace(CLOSED, "");
  const at = closed.search(OPEN);
  if (at === -1) return closed;
  const kept = closed.slice(0, at).trimEnd();
  return closed.slice(at).includes(MVU_PLACEHOLDER) ? `${kept}\n\n${MVU_PLACEHOLDER}` : kept;
}

/** 串流中：從第一個標記截到結尾；結尾停在標記前半先扣住，等後續字到確定不是標記再顯示。 */
export function hideUpdateVariableStreaming(text: string): string {
  const at = text.search(OPEN);
  const shown = at === -1 ? text : text.slice(0, at);
  for (let length = MARKER.length - 1; length > 0; length--) {
    if (shown.slice(-length).toLowerCase() === MARKER.slice(0, length)) return shown.slice(0, -length);
  }
  return shown;
}
