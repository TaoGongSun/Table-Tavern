// 回合錯誤字串（開頭是穩定碼）翻成玩家看得懂的話。只認開頭的碼，不解析供應商原文。
import { t } from "../../i18n";
import { RUNAWAY_CODE, STALLED_CODE } from "../openrouter/api-failure";
import { ALL_BUSY, FREE_MODEL_BUSY, NO_FREE_MODEL } from "../openrouter/smart-call";

/** 錯誤細節裡不得出現金鑰（計畫 2.4）：保險起見遮掉任何 sk-or- 開頭的字串。 */
export function redactKeys(text: string): string {
  return text.replace(/sk-or-[A-Za-z0-9_-]+/g, "sk-or-…");
}

export function explainError(display: string): string {
  if (display.startsWith(NO_FREE_MODEL)) return t("errNoFreeModel");
  if (display.startsWith(ALL_BUSY)) return t("errAllBusy");
  if (display.startsWith(FREE_MODEL_BUSY)) return t("errBusy");
  if (display.startsWith(STALLED_CODE)) return t("errStalled");
  if (display.startsWith(RUNAWAY_CODE)) return t("errRunaway");
  if (display.startsWith("AI_EMPTY_RESPONSE")) return t("errEmpty");
  if (display.startsWith("AI_CONTENT_FILTERED")) return t("errFiltered");
  if (display.startsWith("AI_INCOMPLETE_RESPONSE")) return t("errIncomplete");
  if (/^AI_HTTP_STATUS_40[13]/.test(display)) return t("errAuth");
  if (display.startsWith("AI_HTTP_STATUS_429")) return t("errRateLimited");
  if (/^TypeError|NetworkError|Failed to fetch|Load failed/i.test(display)) return t("errNetwork");
  return t("errGeneric", { detail: redactKeys(display).slice(0, 300) });
}
