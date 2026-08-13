// AI 失敗訊息分流：各家 CLI／API 的錯誤格式不一，只認額度與未登入兩類最痛的，
// 認不出來回 null 走原本的通用文案——誤判比不判更糟（.ai/tasks/ai-error-messages.md）
const QUOTA_ERROR = /usage limit|quota|out of credit|insufficient[ _]credit|insufficient_quota|rate.?limit|resource_exhausted|too many requests|\b402\b|\b429\b/i;
const AUTH_ERROR = /not logged in|not authenticated|unauthorized|authentication|api[ _]key|credential|expired token|\b401\b/i;
const REFUSAL_ERROR =
  /content polic|guideline|safety system|can'?t (create|generate|make|help)|cannot (create|generate|make)|won'?t (create|generate)|unable to (create|generate)|declin|無法(生成|產生|製作)|不能生成|拒絕|违反|違反/i;

export function explainAiError(raw: string): "errQuota" | "errAuth" | "errNoImage" | "errRefused" | null {
  // REFUSED／NO_IMAGE 是生圖 prompt 跟 CLI 約好的暗號：不肯生這一張（內容規範）
  // 與根本生不出圖（多半是生圖額度或方案），玩家的下一步不同
  if (raw.includes("REFUSED")) return "errRefused";
  if (raw.includes("NO_IMAGE")) return "errNoImage";
  if (QUOTA_ERROR.test(raw)) return "errQuota";
  if (AUTH_ERROR.test(raw)) return "errAuth";
  // 模型沒照暗號回時的保底：拒絕的原話多半帶這些字樣
  if (REFUSAL_ERROR.test(raw)) return "errRefused";
  return null;
}
