// CLI 錯誤旁要不要放「檢查 CLI 更新」（plans/cli-version-gate.md 包 2 第 2 點）。
// 模型叫不到、CLI 太舊、回覆格式認不得且版本比測過的舊，各有自己的碼；
// 其餘 CLI 自報或崩潰的錯誤，排除清單（斷網、限流、額度、登入、容量、不完整回覆…）都不符合，才算「無法分類」。
import { explainAiError } from "./ai-error";
import { backendChain } from "./backend-text";

const CLIS = ["claude", "codex", "agy", "grok"];

/** 打開設定視窗某一頁（App 監聽）；錯誤可能出在任何畫面，不一路傳回呼叫。 */
export const OPEN_SETTINGS_EVENT = "tt-open-settings";
/** CLI 換了版本：偵測快取作廢（features/ai-connection/cli.ts 監聽）。 */
export const CLI_CHANGED_EVENT = "tt-cli-changed";

export type CliUpdateKind = "model" | "too_old" | "older" | "unknown";

export interface CliUpdateOffer {
  cli: string;
  kind: CliUpdateKind;
  /** 伺服器或版本門檻要求的最低版本；比已測版高就跳過已測版 */
  required?: string;
  /** 叫不到的模型 id（玩家自訂的模型才給「改選模型」） */
  model?: string;
}

// 這些碼有自己的出路，或換執行檔救不了：一律不放按鈕
const EXCLUDED = new Set([
  "cli_not_found",
  "cli_updating",
  "cli_stalled",
  "cli_stalled_after_output",
  "cli_stdin_timeout",
  "cli_unexpected_abort",
  "cli_risk_not_accepted",
  "codex_auth_missing",
  "cli_output_unrecognized_changed",
  "cli_output_unrecognized_same",
  "cli_output_unrecognized_never",
  "cli_output_unrecognized_unknown_version",
]);

// 暫時性錯誤（跟 cli/runner.rs 的 api_error_kind 同一套：`API Error` 但不是設定類）與網路錯誤：
// 玩家斷網或供應商擁擠時不能叫他換執行檔
const API_ERROR = /api error/i;
// HTTP 狀態碼只認前後文（`API Error: 503`、`status 502`、`HTTP/1.1 500`、`500 Internal Server Error`…），
// 單獨的三位數會把 2.1.500 這類版號誤判成 5xx
const HTTP_CONTEXT = String.raw`(?:\b(?:api error|error|status(?: code)?|http(?:\/\d(?:\.\d)?)?|code)\s*[:=]?\s*)`;
const FATAL_API = new RegExp(
  String.raw`unknown provider|not_found|not found|invalid|authentication|unauthorized|permission|billing|${HTTP_CONTEXT}40[134]\b`,
  "i",
);
const NETWORK = new RegExp(
  String.raw`overloaded|${HTTP_CONTEXT}5\d\d\b|\b5\d\d (?:internal server error|bad gateway|service unavailable|gateway time-?out)|timed? ?out|ECONNRESET|ECONNREFUSED|ENOTFOUND|EAI_AGAIN|network|socket hang up|connection (?:reset|refused|closed)|temporarily unavailable`,
  "i",
);

export function cliUpdateOffer(raw: string, transport?: string): CliUpdateOffer | null {
  if (!transport || !CLIS.includes(transport)) return null;
  const chain = backendChain(raw);
  const found = (code: string) => chain.find((msg) => msg.code === code);
  const text = (value: unknown) => (typeof value === "string" ? value : "");
  const model = found("cli_model_unavailable");
  if (model) return { cli: text(model.params.cli) || transport, kind: "model", model: text(model.params.model) };
  const required = found("cli_too_old_required");
  if (required) {
    return { cli: text(required.params.cli) || transport, kind: "too_old", required: text(required.params.required) };
  }
  const old = found("cli_too_old");
  if (old) return { cli: text(old.params.cli) || transport, kind: "too_old" };
  if (found("cli_output_unrecognized_older")) return { cli: transport, kind: "older" };
  // 無法分類：只有 CLI 自報原文與崩潰兩種，而且要先通過排除判定
  if (chain.some((msg) => EXCLUDED.has(msg.code))) return null;
  if (!chain.some((msg) => msg.code === "cli_reply_error" || msg.code === "cli_crashed")) return null;
  // 限流、額度、登入、容量、不完整回覆、拒絕…已被分流認出來的都不放
  const explained = explainAiError(raw, transport);
  if (explained !== null && explained !== "errAiUnknown") return null;
  if ((API_ERROR.test(raw) && !FATAL_API.test(raw)) || NETWORK.test(raw)) return null;
  return { cli: transport, kind: "unknown" };
}
