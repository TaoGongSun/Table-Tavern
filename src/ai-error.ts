// AI 失敗訊息分流：各家 CLI／API 的錯誤格式不一，只認指得出下一步的幾類，
// 認不出來回 null 讓呼叫端決定（誤判比不判更糟，見 .ai/tasks/ai-error-messages.md）。
// 不診斷根因——聚合 router 會把上游任何失敗都轉包成自己的 5xx，真正的原因在轉包時就沒了。
const QUOTA_ERROR = /usage limit|quota|out of credit|insufficient[ _]credit|insufficient_quota|rate.?limit|resource_exhausted|too many requests|\b402\b|\b429\b/i;
const AUTH_ERROR = /not logged in|not authenticated|unauthorized|authentication|api[ _]key|credential|expired token|\b401\b/i;
// 只有這四個是 CLI；其他值（含拿不到設定時的空手）一律走中性文案，不亂指路
const CLI_TRANSPORTS = ["claude", "codex", "agy", "grok"];

const REFUSAL_ERROR =
  /content polic|guideline|safety system|can'?t (create|generate|make|help)|cannot (create|generate|make)|won'?t (create|generate)|unable to (create|generate)|declin|無法(生成|產生|製作)|不能生成|拒絕|违反|違反/i;

// 傳輸層判定的失敗態帶穩定前綴（見 .ai/plans/stream-failure-visible.md）：串流正常走完
// 卻零內容、被截斷、被內容過濾，三者玩家的下一步不同。一律只認開頭（允許 Tauri 在外面
// 包一個 `Error:`）——供應商的 body 會被原樣附在錯誤字串裡，用 includes 等於讓 body
// 裡抄到的字樣翻盤掉真正的失敗態。
const FAILURE_CODES = [
  [/^(?:Error:\s*)?AI_EMPTY_RESPONSE:/, "errEmptyReply"],
  [/^(?:Error:\s*)?AI_INCOMPLETE_RESPONSE:/, "errIncompleteReply"],
  [/^(?:Error:\s*)?AI_CONTENT_FILTERED:/, "errFiltered"],
] as const;

// 話送出去了、模型沒能回話，而且沒有更精確的碼（連不上、CLI 中途死掉…）。由 lib.rs 的
// stream_via_transport 掛在真正的呼叫結果上；讀卡、寫逐字稿、找不到 CLI 都不帶這個碼。
// 這是最後的保底：CLI 吐的原話（限流、未登入）還是要先走下面的正則分流，
// 被包了一層就退化成籠統一句是拿掉玩家本來看得到的線索。只認開頭，body 抄不進來。
const CALL_FAILED = /^(?:Error:\s*)?AI_CALL_FAILED:/;

// API 路非 2xx 時由 transport.rs 掛在開頭的真實 HTTP 狀態。只匹配開頭（允許外層包一個
// `Error:`），不用 includes——聚合 router 會把上游錯誤整包塞進 body，body 裡的數字
// 若能命中就等於讓供應商偽造狀態碼。有這個碼＝來源必定是 API 路，文案可以直接指路。
const HTTP_STATUS = /^(?:Error:\s*)?AI_HTTP_STATUS_(\d{3}):/;

/**
 * `transport` 給得出來就給（api｜claude｜codex｜agy｜grok）：認證失敗的下一步兩邊不同——
 * API 是「去換一把金鑰」，CLI 是「去重新登入」。給不出來就沿用中性文案，
 * 不猜（叫 API 使用者去按 CLI 的重新驗證鈕，比不指路更糟）。
 */
export function explainAiError(
  raw: string,
  transport?: string,
):
  | "errQuota"
  | "errQuotaApi"
  | "errAuth"
  | "errAuthApi"
  | "errAuthCli"
  | "errApiForbidden"
  | "errApiRequest"
  | "errApiUpstream"
  | "errAiUnknown"
  | "errNoImage"
  | "errRefused"
  | "errEmptyReply"
  | "errIncompleteReply"
  | "errFiltered"
  | null {
  // 真實 HTTP 狀態最先判、命中就結案：這個碼由我們自己掛在最前面，後面接的是供應商
  // 原封不動的 body。任何「往字串裡找字樣」的判斷都排在它後面，否則 body 抄到什麼
  // 都能翻盤——實測見過狀態 503、body 裡卻寫著 429 的（那是 router 轉包上游的殘骸）。
  const status = HTTP_STATUS.exec(raw);
  if (status) {
    const code = Number(status[1]);
    if (code === 401) return "errAuthApi";
    if (code === 403) return "errApiForbidden";
    if (code === 402 || code === 429) return "errQuotaApi";
    return code < 500 ? "errApiRequest" : "errApiUpstream";
  }
  // 再認傳輸層自己的失敗碼；供應商中途送的 error 塊刻意不帶碼，原話往下走既有正則
  const coded = FAILURE_CODES.find(([prefix]) => prefix.test(raw));
  if (coded) return coded[1];
  // REFUSED／NO_IMAGE 是生圖 prompt 跟 CLI 約好的暗號：不肯生這一張（內容規範）
  // 與根本生不出圖（多半是生圖額度或方案），玩家的下一步不同。這兩個是模型回話裡的
  // 暗號、不一定在開頭，只能用 includes——所以更要排在 HTTP 碼後面
  if (raw.includes("REFUSED")) return "errRefused";
  if (raw.includes("NO_IMAGE")) return "errNoImage";
  if (QUOTA_ERROR.test(raw)) return transport === "api" ? "errQuotaApi" : "errQuota";
  if (AUTH_ERROR.test(raw)) {
    if (transport === "api") return "errAuthApi";
    return transport && CLI_TRANSPORTS.includes(transport) ? "errAuthCli" : "errAuth";
  }
  // 模型沒照暗號回時的保底：拒絕的原話多半帶這些字樣
  if (REFUSAL_ERROR.test(raw)) return "errRefused";
  // 什麼都認不出來，但至少知道話是送出去了：給一句「再試一次」，原文照樣附在小字
  if (CALL_FAILED.test(raw)) return "errAiUnknown";
  return null;
}
