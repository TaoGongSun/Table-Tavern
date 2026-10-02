import { useEffect, useRef, useState } from "react";

/**
 * 設定視窗的分頁。視窗已開著時，外面再要求開某一頁（`requestKey` 變了）也要切過去，
 * 但先經過 `confirmLeave`（有未儲存修改時問一次），答取消就留在原頁。
 */
export function useRequestedTab<T extends string>(
  requested: T,
  requestKey: number,
  confirmLeave: () => Promise<boolean>,
) {
  const [tab, setTab] = useState<T>(requested);
  const seen = useRef(requestKey);
  const confirmRef = useRef(confirmLeave);
  confirmRef.current = confirmLeave;

  useEffect(() => {
    if (requestKey === seen.current) return;
    seen.current = requestKey;
    let cancelled = false;
    void confirmRef.current().then((leave) => {
      if (leave && !cancelled) setTab(requested);
    });
    return () => {
      cancelled = true;
    };
  }, [requestKey, requested]);

  return [tab, setTab] as const;
}
