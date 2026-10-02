import { useEffect, useRef, useState } from "react";

/**
 * 離開檢查的結果：true＝可以離開、false＝玩家在確認窗按了取消、
 * "busy"＝守門暫忙（儲存中、權限提示開著、另一個確認窗還沒回覆），這次不算數。
 */
export type LeaveDecision = boolean | "busy";

interface RequestedTabOptions {
  /** 儲存中或巢狀提示開著：外部請求先留著，解除後再處理最新那筆。 */
  blocked?: boolean;
  /** 確認窗還沒回覆：不並行再問一次，回覆後才處理期間進來的最新請求。 */
  confirmPending?: boolean;
  /** 外部請求實際切了頁（設定視窗據此把焦點從卸載的元素移到新分頁）。 */
  onSwitched?: () => void;
}

/**
 * 設定視窗的分頁。視窗已開著時，外面再要求開某一頁（`requestKey` 變了）也要切過去，
 * 但先經過 `confirmLeave`（有未儲存修改時問一次），答取消就留在原頁。
 * 只保留最新一筆請求：守門暫忙時不處理也不記成看過，等解除後照最新的 requested 再走一次；
 * 只有實際切頁或玩家按取消才算處理完。
 */
export function useRequestedTab<T extends string>(
  requested: T,
  requestKey: number,
  confirmLeave: () => Promise<LeaveDecision>,
  { blocked = false, confirmPending = false, onSwitched }: RequestedTabOptions = {},
) {
  const [tab, setTab] = useState<T>(requested);
  const [retry, setRetry] = useState(0);
  const seen = useRef(requestKey);
  const inFlight = useRef(false);
  const tabRef = useRef(tab);
  tabRef.current = tab;
  const latestKey = useRef(requestKey);
  latestKey.current = requestKey;
  const confirmRef = useRef(confirmLeave);
  confirmRef.current = confirmLeave;
  const switchedRef = useRef(onSwitched);
  switchedRef.current = onSwitched;

  useEffect(() => {
    if (requestKey === seen.current || blocked || confirmPending || inFlight.current) return;
    const key = requestKey;
    const target = requested;
    if (target === tabRef.current) {
      seen.current = key;
      return;
    }
    inFlight.current = true;
    // 不在 cleanup 取消：confirmPending 一變就會跑 cleanup，進行中的確認會把自己取消掉
    void confirmRef
      .current()
      // 確認本身失敗（系統對話框叫不起來）視同玩家取消：標記、不切頁、不無限重試
      .catch((): LeaveDecision => false)
      .then((decision) => {
        if (decision === "busy") return;
        seen.current = key;
        if (decision) {
          setTab(target);
          switchedRef.current?.();
        }
      })
      .finally(() => {
        inFlight.current = false;
        // 確認期間又來了新請求，或這次撞上守門：讓 effect 再跑一次，看最新那筆
        if (latestKey.current !== seen.current) setRetry((n) => n + 1);
      });
  }, [requestKey, requested, blocked, confirmPending, retry]);

  return [tab, setTab] as const;
}
