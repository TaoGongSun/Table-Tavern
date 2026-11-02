// 回合錯誤字串（開頭是穩定碼）翻成玩家看得懂的話。只認開頭的碼，不解析供應商原文。
// 中國供應商的魔粒用完、未實名、內容審核實際回什麼沒實測（作者裁決 2026-11-02），文案只說「可能」。
import { t } from "../../i18n";
import { CONTEXT_TOO_LONG_CODE, RUNAWAY_CODE, STALLED_CODE } from "../openrouter/api-failure";
import { ALL_BUSY, FREE_MODEL_BUSY, NO_FREE_MODEL, PROMPT_EXCEEDS_CONTEXT } from "../openrouter/smart-call";
import { CN_PROVIDERS, isCn, type ProviderId } from "../provider/providers";

export interface ErrorContext {
  provider: ProviderId;
  /** 目前的金鑰：錯誤細節裡出現原字串就遮掉（魔搭令牌格式沒有文件可依） */
  apiKey?: string | null;
}

/** 錯誤細節裡不得出現金鑰（計畫 2.4）：遮掉 sk- 開頭的長字串與目前金鑰的原字串。 */
export function redactKeys(text: string, apiKey?: string | null): string {
  const masked = text.replace(/sk-or-[A-Za-z0-9_-]+/g, "sk-or-…").replace(/sk-(?!or-…)[A-Za-z0-9_-]{16,}/g, "sk-…");
  return apiKey ? masked.split(apiKey).join("…") : masked;
}

export function explainError(display: string, context: ErrorContext = { provider: "openrouter" }): string {
  const cn = isCn(context.provider) ? CN_PROVIDERS[context.provider] : null;
  const detail = () => redactKeys(display, context.apiKey).slice(0, 300);
  const withHint = (text: string) => (cn ? `${text}${t(cn.quotaHint)}` : text);
  if (display.startsWith(PROMPT_EXCEEDS_CONTEXT)) return t("errPromptTooLarge");
  if (display.startsWith(CONTEXT_TOO_LONG_CODE)) return t("errContextTooLong");
  if (display.startsWith(NO_FREE_MODEL)) return t("errNoFreeModel");
  if (display.startsWith(ALL_BUSY)) return withHint(t("errAllBusy"));
  if (display.startsWith(FREE_MODEL_BUSY)) return withHint(t("errBusy"));
  if (display.startsWith(STALLED_CODE)) return t("errStalled");
  if (display.startsWith(RUNAWAY_CODE)) return t("errRunaway");
  if (display.startsWith("AI_EMPTY_RESPONSE")) return t("errEmpty");
  if (display.startsWith("AI_CONTENT_FILTERED")) return cn ? `${t("errFiltered")}${t("errFilteredCn")}` : t("errFiltered");
  if (display.startsWith("AI_INCOMPLETE_RESPONSE")) return t("errIncomplete");
  if (cn && display.startsWith("AI_HTTP_STATUS_401")) return t(cn.authError);
  if (cn && display.startsWith("AI_HTTP_STATUS_403")) return withHint(t("errForbiddenCn", { detail: detail() }));
  if (/^AI_HTTP_STATUS_40[13]/.test(display)) return t("errAuth");
  if (display.startsWith("AI_HTTP_STATUS_429")) return cn ? withHint(t("errRateLimitedCn")) : t("errRateLimited");
  if (/^TypeError|NetworkError|Failed to fetch|Load failed/i.test(display)) return t("errNetwork");
  return t("errGeneric", { detail: detail() });
}
