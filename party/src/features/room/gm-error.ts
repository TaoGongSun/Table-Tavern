// GM 生成失敗的錯誤碼 → 種類與一句人話（room-host-actions）。桌面版資訊列「最近錯誤」與重生時的換模型選單共用這一份：
// 模型本身用不了（下架、金鑰／登入）就該換模型；暫時性（額度、逾時、網路）原模型直接重試。
// 碼取執行端分類過的 `ExecutorError.code`（桌面版錯誤字串的開頭），不看供應商 body。
import type { ExecutorErrorKind } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";

export type GmErrorKind = "gone" | "auth" | "quota" | "stalled" | "network" | "other";

const KINDS: Array<[RegExp, GmErrorKind]> = [
  [/^(AI_HTTP_STATUS_404|cli_model_unavailable)/, "gone"],
  [/^(AI_HTTP_STATUS_40[13]|cli_not_found|cli_risk_not_accepted|openrouter_api_key_missing)/, "auth"],
  [/^(AI_HTTP_STATUS_(402|429)|AI_FREE_MODEL_BUSY)/, "quota"],
  [/^(AI_STREAM_STALLED|cli_stalled|E_GEN_TIMEOUT)/, "stalled"],
  [/^(AI_CALL_FAILED|AI_HTTP_STATUS_5|cli_crashed|cli_turn_failed)/, "network"],
];
const TEXT: Record<GmErrorKind, MsgKey> = { gone: "gmErrGone", auth: "gmErrAuth", quota: "gmErrQuota", stalled: "gmErrStalled", network: "gmErrNetwork", other: "gmErrOther" };

export const gmErrorKind = (code: string): GmErrorKind => KINDS.find(([re]) => re.test(code))?.[1] ?? "other";
/** 卡住回合的公開種類（執行端 classify 分好的，人人看得到）對到同一套種類 */
const HOLD: Partial<Record<ExecutorErrorKind, GmErrorKind>> = { model_unavailable: "gone", not_ready: "auth", quota: "quota", provider_unreachable: "network" };
export const holdKind = (kind: ExecutorErrorKind): GmErrorKind => HOLD[kind] ?? "other";
/** 這個模型本身用不了：重生時灰掉它、預設換別的 */
export const modelBroken = (kind: GmErrorKind) => kind === "gone" || kind === "auth";
export const gmErrorText = (code: string) => t(TEXT[gmErrorKind(code)], { code: code.slice(0, 40) });
