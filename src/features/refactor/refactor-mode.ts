// 重構雙軌定向（refactor-mode-split）：本機三態偵測。
// 素材是後端 card_interfaces 回傳的清單——介面腳本存在與否本機判得準，人物一律不用本機猜。
import { type CardInterface } from "../card-interface/interface-card";
import { backendCode } from "../../shared/ui/backend-text";
import { isOpenRouterKeyMissing } from "../../shared/ui/ai-error";

export type RefactorTristate = "supported" | "unsupported" | "none";

/**
 * 三態：只要有一張卡帶可用顯示腳本＝supported（進二選一）；沒有可用腳本但有 DRM／雲端
 * 載入器卡＝unsupported（擋下，不跑重構）；兩者皆無＝none（免問，直跑角色線）。
 * 混合桌取「還有得救」優先：有一張可接管就值得問玩家。
 */
export function detectRefactorTristate(cards: CardInterface[]): RefactorTristate {
  if (cards.some((card) => card.unsupported === null && card.scripts.length > 0)) return "supported";
  if (cards.some((card) => card.unsupported !== null)) return "unsupported";
  return "none";
}

export type RefactorMode = "interface" | "characters";

/** 後端 refactor_recommend 的初判結果（對照 refactor_ai.rs RefactorRecommendOutcome）。
 * run_id 非空＝claude lane 開了短命 session，第二段憑 run_id＋fingerprint resume 承快取。 */
export interface RefactorRecommendOutcome {
  recommend: string;
  evidence: string;
  run_id: string;
  fingerprint: string;
  raw: string;
}

/** 第二段 resume 憑證：初判回傳的 session id＋卡片內容指紋。 */
export interface RefactorRunTicket {
  runId: string;
  fingerprint: string;
}

export type ProbeFailureKind = "aborted" | "not_sent" | "failed" | "unparsable";

// 有限白名單決定是否開二選一：名單碼屬持續性設定／前置障礙（設定、CLI 偵測、更新閘門、智慧免費選模）。
const NOT_SENT_CODES = new Set([
  "cli_risk_not_accepted",
  "cli_not_found",
  "agy_too_old",
  "unknown_transport",
  "update_gate_closed",
  "tier_model_missing",
  "no_free_models",
  "no_stable_free_model",
]);
// 智慧免費送出前查額度就擋的那一個，前綴是 429 給錯誤分流用
const DAILY_EXHAUSTED = /^(?:Error:\s*)?AI_HTTP_STATUS_429:\s*/;

/**
 * 初判失敗分類（2026-10-12 作者裁決）：not_sent＝狀態列報原因、不開二選一；failed／unparsable
 * 照 08-14 拍板開二選一並附原因。只有有限白名單算 not_sent，認不出的一律 failed。
 */
export function probeFailureKind(reason: unknown): ProbeFailureKind {
  const raw = String(reason);
  const bare = raw.startsWith("Error: ") ? raw.slice("Error: ".length) : raw;
  if (bare === "refactor-aborted") return "aborted";
  if (bare === "refactor-recommend-unparsable") return "unparsable";
  const code = backendCode(raw);
  if (code !== null && NOT_SENT_CODES.has(code)) return "not_sent";
  // 外層前綴只認這兩個特例：缺 OpenRouter key（裸碼或 AI_CALL_FAILED 前綴）、智慧免費今日額度用完；
  // 其餘帶前綴的一律 failed
  if (isOpenRouterKeyMissing(raw)) return "not_sent";
  if (DAILY_EXHAUSTED.test(raw) && backendCode(raw.replace(DAILY_EXHAUSTED, "")) === "smart_free_daily_exhausted") {
    return "not_sent";
  }
  return "failed";
}

const TRANSPORT_PREFIX = /^(?:Error:\s*)?(?:AI_CALL_FAILED|AI_HTTP_STATUS_\d{3}):\s*/;

/** not_sent 報狀態列前剝掉傳輸層前綴，狀態列只剩翻譯後的人話。 */
export function stripTransportPrefix(raw: string): string {
  return raw.replace(TRANSPORT_PREFIX, "");
}
