import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { t } from "../../i18n";

// 後端 commands/chat.rs 的 claude 續聊線：某次嘗試落在 Claude 訂閱超額時發（claude-1h-cache）
export const CLAUDE_OVERAGE_EVENT = "claude-overage";

export interface ClaudeOveragePayload {
  eventId: string;
  /** 那次超額嘗試實際寫了哪種快取：整段 1h／其他（5m、混合、缺拆分、沒寫）／沒拿到用量 */
  observed: "one-hour" | "other" | "unknown";
}

const TEXT_KEYS = {
  "one-hour": "claudeOverageOneHour",
  other: "claudeOverageOther",
  unknown: "claudeOverageUnknown",
} as const;

// 超額是帳號狀態，不是桌的狀態：整個 app 執行期間只提示一次，不分桌，app 重啟才重來。
// 放模組層級，元件重掛（換頁、離桌）不會重置。
let shownThisRun = false;

/** 測試用：模擬 app 重啟 */
export function resetClaudeOverageForTest() {
  shownThisRun = false;
}

// Claude 訂閱進入超額的一次性提示：掛在 App 層，哪一桌觸發都當下顯示；純資訊，只能關掉。
export function ClaudeOverageToast() {
  const [observed, setObserved] = useState<ClaudeOveragePayload["observed"] | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ClaudeOveragePayload>(CLAUDE_OVERAGE_EVENT, (event) => {
      // 已卸載就不顯示也不耗名額；名額在顯示前同步佔住，連續兩個事件只出一次
      if (disposed || shownThisRun) return;
      shownThisRun = true;
      setObserved(event.payload.observed in TEXT_KEYS ? event.payload.observed : "unknown");
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  if (!observed) return null;
  return (
    <div className="claude-overage-toast" role="status">
      <span>{t(TEXT_KEYS[observed])}</span>
      <button type="button" className="ghost" onClick={() => setObserved(null)}>
        {t("smartFreeNewDismiss")}
      </button>
    </div>
  );
}
