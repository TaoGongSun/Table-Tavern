// 進出桌互斥：同一時間只允許一個「換桌級」操作（進桌、回大廳、開／刪／匯入…）在飛。
// 鎖用同步 ref 取，連點兩下不會雙雙通過；busy 另存成 state 給畫面停用按鈕。
// 一定在 finally 釋放，守門取消、確認框取消、進桌失敗都不會把鎖卡死。
import { useCallback, useRef, useState } from "react";

export function useTableOp() {
  const held = useRef(false);
  const [busy, setBusy] = useState(false);

  /** 拿不到鎖就直接返回 undefined、不執行 fn。只放在頂層入口，fn 內不得再呼叫 run */
  const run = useCallback(async <T>(fn: () => Promise<T>): Promise<T | undefined> => {
    if (held.current) return undefined;
    held.current = true;
    setBusy(true);
    try {
      return await fn();
    } finally {
      held.current = false;
      setBusy(false);
    }
  }, []);

  /** 同步問「現在有沒有換桌級操作在飛」：函式層擋住那些不該在這時動的入口 */
  const isHeld = useCallback(() => held.current, []);

  return { busy, run, isHeld };
}
