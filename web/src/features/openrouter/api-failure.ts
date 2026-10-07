// 派送失敗的結構化資訊與換模分類。照桌面版 src-tauri/src/transport/api_failure.rs 與
// src-tauri/src/smart_free/failover.rs（classify :70、needs_key_check :66）改寫成 TS。

export type FailureStage = "http" | "stream" | "network" | "timeout";

export interface RateLimit {
  limit: string | null;
  remaining: string | null;
  reset: string | null;
}

export interface ErrorDetail {
  code: number | null;
  message: string;
  errorType: string | null;
  providerName: string | null;
}

export interface ApiFailure {
  stage: FailureStage;
  /** HTTP 狀態；串流錯誤時為 null（狀態已是 200），改看 detail.code。 */
  status: number | null;
  rateLimit: RateLimit | null;
  detail: ErrorDetail;
  /** 失敗前是否已經吐出任何正文：已吐就不重送（換模的第二發只在零正文時派）。 */
  emittedText: boolean;
  /** 給畫面的字串，開頭掛穩定碼（AI_HTTP_STATUS_*、AI_STREAM_STALLED: …）。 */
  display: string;
}

const EMPTY_DETAIL: ErrorDetail = { code: null, message: "", errorType: null, providerName: null };

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : null;
}

export function detailFromErrorObject(error: unknown): ErrorDetail {
  if (!error || typeof error !== "object") return EMPTY_DETAIL;
  const record = error as Record<string, unknown>;
  const metadata = (record.metadata ?? null) as Record<string, unknown> | null;
  const rawCode = record.code;
  const code =
    typeof rawCode === "number"
      ? Math.trunc(rawCode)
      : typeof rawCode === "string" && /^\s*-?\d+\s*$/.test(rawCode)
        ? Number.parseInt(rawCode, 10)
        : null;
  return {
    code,
    message: text(record.message) ?? "",
    errorType: text(metadata?.error_type),
    providerName: text(metadata?.provider_name),
  };
}

function detailFromBody(body: string): ErrorDetail {
  try {
    const value = JSON.parse(body) as Record<string, unknown> | null;
    return value && typeof value === "object" && "error" in value
      ? detailFromErrorObject(value.error)
      : EMPTY_DETAIL;
  } catch {
    return EMPTY_DETAIL;
  }
}

/** 平台說輸入超過模型的上下文上限（同桌面版 transport/context_overflow.rs 的 CODE）。 */
export const CONTEXT_TOO_LONG_CODE = "AI_CONTEXT_TOO_LONG:";

/** 供應商錯誤訊息是不是在說輸入超過模型容量（同桌面版 context_overflow.rs message_says_too_long）。 */
function messageSaysTooLong(message: string): boolean {
  const lower = message.toLowerCase();
  return (
    lower.includes("prompt is too long") ||
    lower.includes("context_length_exceeded") ||
    lower.includes("maximum context length") ||
    lower.includes("exceeds the context window") ||
    lower.includes("maximum prompt length") ||
    lower.includes("input_too_large") ||
    (lower.includes("input token count") && lower.includes("exceeds the maximum"))
  );
}

/**
 * 只在 400／413 時看 body 的 error 物件（`code` 字串或 `message`），不在泛用全文裡掃字樣
 * （同桌面版 api_body_says_too_long；陣列包一層取第一個）。
 */
export function bodySaysTooLong(status: number, body: string): boolean {
  if (status !== 400 && status !== 413) return false;
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    return false;
  }
  const root = Array.isArray(value) ? value[0] : value;
  const error = root && typeof root === "object" ? (root as Record<string, unknown>).error : undefined;
  if (!error || typeof error !== "object") return false;
  const field = (key: string) => {
    const raw = (error as Record<string, unknown>)[key];
    return typeof raw === "string" ? raw : "";
  };
  return field("code") === "context_length_exceeded" || messageSaysTooLong(field("message"));
}

/** 非 2xx 的顯示字串：穩定碼在前（容量爆掉另掛 AI_CONTEXT_TOO_LONG），原文留到 2000 字（真的超長才截，並明講）。 */
export function httpErrorDisplay(status: number, body: string): string {
  const LIMIT = 2000;
  const chars = [...body];
  const kept = chars.slice(0, LIMIT).join("");
  const cut = chars.length > LIMIT ? "…[truncated]" : "";
  const overflow = bodySaysTooLong(status, body) ? `${CONTEXT_TOO_LONG_CODE} ` : "";
  return `${overflow}AI_HTTP_STATUS_${status}: status=${status} body=${kept}${cut}`;
}

export function failureFromHttp(status: number, headers: Headers, body: string): ApiFailure {
  const rateLimit: RateLimit = {
    limit: headers.get("x-ratelimit-limit"),
    remaining: headers.get("x-ratelimit-remaining"),
    reset: headers.get("x-ratelimit-reset"),
  };
  const hasRateLimit = rateLimit.limit !== null || rateLimit.remaining !== null || rateLimit.reset !== null;
  return {
    stage: "http",
    status,
    rateLimit: hasRateLimit ? rateLimit : null,
    detail: detailFromBody(body),
    emittedText: false,
    display: httpErrorDisplay(status, body),
  };
}

function bare(stage: FailureStage, display: string, emittedText: boolean): ApiFailure {
  return { stage, status: null, rateLimit: null, detail: EMPTY_DETAIL, emittedText, display };
}

export const failureNetwork = (display: string, emittedText: boolean): ApiFailure =>
  bare("network", display, emittedText);

export const STALLED_CODE = "AI_STREAM_STALLED:";
export const RUNAWAY_CODE = "AI_OUTPUT_RUNAWAY:";

/** 串流停滯逾時：分類走 timeout。 */
export const failureStalled = (secs: number, emittedText: boolean): ApiFailure =>
  bare("timeout", `${STALLED_CODE} idle_secs=${secs}`, emittedText);

/** 輸出失控：必定已吐出內容，所以不重送；分類落在 other，不計入換模。 */
export const failureRunaway = (display: string): ApiFailure => bare("stream", display, true);

export function failureStream(display: string, detail: ErrorDetail | null, emittedText: boolean): ApiFailure {
  return { ...bare("stream", display, emittedText), detail: detail ?? EMPTY_DETAIL };
}

export const failureCode = (failure: ApiFailure): number | null => failure.status ?? failure.detail.code;

export type FailureClass = "model" | "gone" | "unknown-rate-limit" | "account" | "other";

/** 可計入換模次數的類別。 */
export const countsTowardSwitch = (cls: FailureClass) =>
  cls === "model" || cls === "gone" || cls === "unknown-rate-limit";

/** 無平台證據的 429 才查 /key；結果只用來排除「每日用完」。 */
export type KeyCheck = { kind: "not-queried" } | { kind: "failed" } | { kind: "remaining"; remaining: number };

function platformEvidence(failure: ApiFailure): boolean {
  const remaining = failure.rateLimit?.remaining;
  if (remaining != null && Number.isFinite(Number(remaining.trim())) && Number(remaining.trim()) <= 0) {
    return true;
  }
  return [failure.detail.message, failure.display].some((value) => {
    const lower = value.toLowerCase();
    return lower.includes("free-models-per-day") || lower.includes("free-models-per-min");
  });
}

/**
 * 今日免費次數用完的證據：429 且平台明說是每日上限，或 /key 確認剩 0。換模分類把每分鐘與每日
 * 限流都歸 account（換哪支都一樣被擋，桌面版 classify 同樣如此）；但只有每日證據才擋送出、
 * 跳用完導流——每分鐘限流稍等就好。
 */
export function dailyEvidence(failure: ApiFailure, key: KeyCheck): boolean {
  if (failureCode(failure) !== 429) return false;
  if (key.kind === "remaining" && key.remaining <= 0) return true;
  return [failure.detail.message, failure.display].some((value) => value.toLowerCase().includes("free-models-per-day"));
}

export const needsKeyCheck = (failure: ApiFailure) => failureCode(failure) === 429 && !platformEvidence(failure);

export function classifyFailure(failure: ApiFailure, key: KeyCheck): FailureClass {
  const code = failureCode(failure);
  if (code !== null && code >= 401 && code <= 403) return "account";
  if (code === 429) {
    const dailyGone = key.kind === "remaining" && key.remaining <= 0;
    return platformEvidence(failure) || dailyGone ? "account" : "unknown-rate-limit";
  }
  if (code === 404) return "gone";
  if (
    failure.stage === "network" ||
    failure.stage === "timeout" ||
    code === 408 ||
    code === 502 ||
    code === 503 ||
    code === 504 ||
    ["provider_overloaded", "provider_unavailable", "timeout"].includes(failure.detail.errorType ?? "")
  ) {
    return "model";
  }
  return "other";
}
