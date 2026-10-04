import { t } from "../../i18n";

// 後端 smart_free 的換模事件（payload 見 .ai/plans/stable-free-failover.md §2.4）。
// from／to／model 已是顯示名；turnId 為 null＝非聊天輪的呼叫（換幕摘要、翻譯、重構、開桌、生圖）。
export const FAILOVER_EVENT = "smart-free-failover";
export const SWITCHED_EVENT = "smart-free-model-switched";

export interface FailoverPayload {
  eventId: string;
  world: string | null;
  turnId: string | null;
  from: string;
  to: string;
  retried: boolean;
}

export interface SwitchedPayload {
  eventId: string;
  world: string | null;
  turnId: string | null;
  model: string;
}

/** 提示行的內容：換模（failover）或一般換手（switched） */
export type SmartFreeNotice =
  | { kind: "failover"; from: string; to: string }
  | { kind: "switched"; model: string };

export function failoverNotice(payload: FailoverPayload): SmartFreeNotice {
  return { kind: "failover", from: payload.from, to: payload.to };
}

export function switchedNotice(payload: SwitchedPayload): SmartFreeNotice {
  return { kind: "switched", model: payload.model };
}

export function noticeText(notice: SmartFreeNotice): string {
  return notice.kind === "failover"
    ? t("smartFreeFailover", { from: notice.from, to: notice.to })
    : t("smartFreeSwitched", { model: notice.model });
}

/** 以 eventId 去重：第一次見到回 true，重複回 false。紀錄隨呼叫端的檢視期間存活（換幕、離桌、卸載即丟），不設上限。 */
export function createEventDeduper(): (eventId: string) => boolean {
  const seen = new Set<string>();
  return (eventId) => {
    if (seen.has(eventId)) return false;
    seen.add(eventId);
    return true;
  };
}
