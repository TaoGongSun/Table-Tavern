// AI 失敗訊息分流：各家 CLI／API 的錯誤格式不一，只認額度與未登入兩類最痛的，
// 認不出來回 null 走原本的通用文案——誤判比不判更糟（.ai/tasks/ai-error-messages.md）
const QUOTA_ERROR = /usage limit|quota|out of credit|insufficient[ _]credit|insufficient_quota|rate.?limit|resource_exhausted|too many requests|\b402\b|\b429\b/i;
const AUTH_ERROR = /not logged in|not authenticated|unauthorized|authentication|api[ _]key|credential|expired token|\b401\b/i;
// 只有這四個是 CLI；其他值（含拿不到設定時的空手）一律走中性文案，不亂指路
const CLI_TRANSPORTS = ["claude", "codex", "agy", "grok"];

const REFUSAL_ERROR =
  /content polic|guideline|safety system|can'?t (create|generate|make|help)|cannot (create|generate|make)|won'?t (create|generate)|unable to (create|generate)|declin|無法(生成|產生|製作)|不能生成|拒絕|违反|違反/i;

// 傳輸層判定的失敗態帶穩定前綴（見 .ai/plans/stream-failure-visible.md）：串流正常走完
// 卻零內容、被截斷、被內容過濾，三者玩家的下一步不同。用 includes 不用 startsWith——
// 同一個錯誤字串在不同呼叫層可能被包裝過。
const FAILURE_CODES = [
  ["AI_EMPTY_RESPONSE", "errEmptyReply"],
  ["AI_INCOMPLETE_RESPONSE", "errIncompleteReply"],
  ["AI_CONTENT_FILTERED", "errFiltered"],
] as const;

/**
 * `transport` 給得出來就給（api｜claude｜codex｜agy｜grok）：認證失敗的下一步兩邊不同——
 * API 是「去換一把金鑰」，CLI 是「去重新登入」。給不出來就沿用中性文案，
 * 不猜（叫 API 使用者去按 CLI 的重新驗證鈕，比不指路更糟）。
 */
export function explainAiError(
  raw: string,
  transport?: string,
): "errQuota" | "errAuth" | "errAuthApi" | "errAuthCli" | "errNoImage" | "errRefused" | "errEmptyReply" | "errIncompleteReply" | "errFiltered" | null {
  // 帶碼的先認碼；供應商中途送的 error 塊刻意不帶碼，原話往下走既有正則
  // （免費層 429 的原話會被 QUOTA_ERROR 接住，玩家看到「額度用完」而非籠統錯誤）
  const coded = FAILURE_CODES.find(([prefix]) => raw.includes(prefix));
  if (coded) return coded[1];
  // REFUSED／NO_IMAGE 是生圖 prompt 跟 CLI 約好的暗號：不肯生這一張（內容規範）
  // 與根本生不出圖（多半是生圖額度或方案），玩家的下一步不同
  if (raw.includes("REFUSED")) return "errRefused";
  if (raw.includes("NO_IMAGE")) return "errNoImage";
  if (QUOTA_ERROR.test(raw)) return "errQuota";
  if (AUTH_ERROR.test(raw)) {
    if (transport === "api") return "errAuthApi";
    return transport && CLI_TRANSPORTS.includes(transport) ? "errAuthCli" : "errAuth";
  }
  // 模型沒照暗號回時的保底：拒絕的原話多半帶這些字樣
  if (REFUSAL_ERROR.test(raw)) return "errRefused";
  return null;
}
