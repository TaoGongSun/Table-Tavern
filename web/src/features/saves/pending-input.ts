// 回合進行中玩家那一句的草稿（D28〔作者裁決 2026-10-07〕：串流中重新整理退回上次完整回合，玩家那句放回
// 輸入框）。存檔只在回合結束後寫，進行中的那句不在存檔裡，另外記在 sessionStorage：只跟這個分頁、重新整理
// 還在、關掉分頁就沒有，也不會跟著存檔匯出。存不了（瀏覽器不讓存）就算了，退回的回合照樣成立。
const key = (saveId: string) => `tt-web:pending-input:${saveId}`;

const storage = (): Storage | null => {
  try {
    return globalThis.sessionStorage ?? null;
  } catch {
    return null;
  }
};

export function rememberPendingInput(saveId: string, text: string): void {
  try {
    storage()?.setItem(key(saveId), text);
  } catch {
    // 瀏覽器不讓存就算了
  }
}

export function forgetPendingInput(saveId: string): void {
  try {
    storage()?.removeItem(key(saveId));
  } catch {
    // 同上
  }
}

/** 讀草稿（不清；放回輸入框後由呼叫端 `forgetPendingInput`）。 */
export function readPendingInput(saveId: string): string | null {
  try {
    return storage()?.getItem(key(saveId)) ?? null;
  } catch {
    return null;
  }
}
