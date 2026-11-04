// 錯誤碼 → 客戶端顯示（協定 §8「客戶端顯示」欄逐碼對應）與執行端錯誤的公開說明（§7.1「全員看到」欄）。
import type { ErrorCode, ExecutorErrorKind } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";

export const ERROR_TEXT: Record<ErrorCode | "E_OFFLINE", MsgKey> = {
  E_BAD_FRAME: "errVersion", // 「版本不符，請重新整理」
  E_VERSION: "errUpdate",
  E_AUTH_REQUIRED: "errReconnecting", // 自動重連
  E_AUTH_FAILED: "errLinkInvalid",
  E_ROOM_NOT_FOUND: "errRoomNotFound",
  E_ROOM_CLOSED: "errRoomClosed",
  E_BANNED: "errRemoved",
  E_KICKED: "errRemoved", // 同上，可用新連結回來
  E_ROOM_FULL: "errRoomFull",
  E_NAME_INVALID: "errNameInvalid", // 欄位下紅字
  E_TEXT_INVALID: "errTextInvalid", // 輸入框下紅字，原文保留
  E_FORBIDDEN: "errForbidden",
  E_ROUND_STALE: "errRoundStale", // 草稿保留
  E_ROUND_NOT_ACTING: "errRoundNotActing", // 輸入框照可打字
  E_STATE_CONFLICT: "errStateConflict", // 重抓快照後以畫面為準，不另跳窗（useRoom 處理）
  E_SEAT_TAKEN: "errSeatTaken",
  E_SEAT_REQUIRED: "errSeatRequired",
  E_EXECUTOR_OFFLINE: "errExecutorOffline",
  E_EXECUTOR_TIMEOUT: "errExecutorTimeout",
  E_GEN_TIMEOUT: "holdProviderUnreachable", // 同 provider_unreachable
  E_PAIR_CODE_INVALID: "errPairCodeInvalid",
  E_PAIR_LOCKED: "errPairLocked",
  E_MODEL_NOT_OFFERED: "errModelNotOffered",
  E_SETTING_INVALID: "errSettingInvalid",
  E_RATE_LIMITED: "errRateLimited", // 草稿類靜默延後（useRoom）
  E_DISPATCH_UNKNOWN: "errInternal", // 只有執行端會收到
  E_SESSION_REPLACED: "errSessionReplaced", // 不自動重連
  E_EXECUTOR_REPLACED: "errInternal", // 只有執行端會收到
  E_INTERNAL: "errInternal",
  // 玩家卡上傳與生圖（room-player-cards）：包 3 換成各自的說明，先用通用錯誤
  E_LENGTH_REQUIRED: "errInternal",
  E_TOO_LARGE: "errInternal",
  E_UNSUPPORTED_FORMAT: "errInternal",
  E_CARD_INVALID: "errInternal",
  E_GEN_QUOTA: "errInternal",
  E_GEN_UNSUPPORTED: "errInternal",
  E_GEN_REFUSED: "errInternal",
  E_OFFLINE: "errOffline", // 客戶端自己的：斷線中送不出去
};

/** 不另跳窗、靜默處理的碼 */
const SILENT = new Set<string>(["E_STATE_CONFLICT", "E_RATE_LIMITED", "E_AUTH_REQUIRED"]);

export function errorText(code: ErrorCode | "E_OFFLINE", retryAfterMs?: number): string | null {
  if (SILENT.has(code)) return null;
  return t(ERROR_TEXT[code], { minutes: Math.max(1, Math.ceil((retryAfterMs ?? 0) / 60_000)) });
}

export const HOLD_TEXT: Record<ExecutorErrorKind, MsgKey> = {
  refusal: "holdRefusal",
  quota: "holdQuota",
  tool_format: "holdNeedsHost",
  provider_unreachable: "holdProviderUnreachable",
  scene_full: "holdSceneFull",
  not_ready: "holdNeedsHost",
  other: "holdOther",
};

export const clock = (at: number) => new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
