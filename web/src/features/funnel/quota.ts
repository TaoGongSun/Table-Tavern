// 今日免費次數與「用完就導去下載」的狀態機。只看 /key（不佔次數）與回合錯誤，不替玩家做任何
// 會花錢的事；用完時只導向桌面版下載（D11）。
import type { FreeDaily } from "../openrouter/openrouter-api";

export type QuotaState =
  | { kind: "unknown" }
  | { kind: "unlimited" }
  | { kind: "counted"; limit: number; remaining: number }
  /** 用完：/key 剩 0，或回合錯誤是平台的每日上限。 */
  | { kind: "exhausted"; limit: number | null };

export type QuotaEvent =
  | { type: "key-info"; daily: FreeDaily | null }
  | { type: "daily-exhausted-error" }
  | { type: "logout" };

export function nextQuota(state: QuotaState, event: QuotaEvent): QuotaState {
  switch (event.type) {
    case "logout":
      return { kind: "unknown" };
    case "daily-exhausted-error":
      return { kind: "exhausted", limit: state.kind === "counted" ? state.limit : null };
    case "key-info": {
      const daily = event.daily;
      // 查不到就保留原狀，不把「用完」誤解成「恢復」
      if (daily === null) return state;
      if (daily.kind === "unlimited") return { kind: "unlimited" };
      return daily.remaining <= 0
        ? { kind: "exhausted", limit: daily.limit }
        : { kind: "counted", limit: daily.limit, remaining: daily.remaining };
    }
  }
}

/** 用完時擋住送出、顯示導流面板。 */
export const quotaBlocksSending = (state: QuotaState) => state.kind === "exhausted";
