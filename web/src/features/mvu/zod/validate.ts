// 主頁驗 Worker 的回傳（mvu-zod-semantics 規格 1.4、4 節）：卡片腳本與核心同一個 realm，核心自己的上限可能被卡片
// 改掉，所以筆數、大小、形狀在這裡再擋一次（同桌面 Rust 端）。不合就丟錯（整次失敗）。
import type { MvuDetailedResult } from "@desktop/features/mvu-engine/host-contract";
import type { ZodInitialized } from "./frame-host";

/** 同核心的 PARSE_LIMITS 與 1.4 節 */
const LIMITS = { TABLE_BYTES: 2 * 1024 * 1024, RESULTS: 1000, CHANGES: 2000, REPLY_CHARS: 48 * 1024 * 1024 };
const STATUSES = new Set(["applied", "no-change", "rejected", "ignored"]);

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const isPath = (value: unknown) => Array.isArray(value) && value.every((part) => typeof part === "string");
const bytes = (value: unknown) => new TextEncoder().encode(JSON.stringify(value) ?? "").length;

function parseReply(json: string): unknown {
  if (typeof json !== "string" || json.length > LIMITS.REPLY_CHARS) throw new Error("回傳過大");
  return JSON.parse(json);
}

export function checkDetailed(json: string): MvuDetailedResult {
  const value = parseReply(json);
  if (!isObject(value) || !isObject(value.table) || typeof value.zod !== "boolean") throw new Error("回傳形狀不對");
  const { table, results, changes } = value;
  if (bytes(table) > LIMITS.TABLE_BYTES) throw new Error("變數表超過上限");
  if (!Array.isArray(results) || results.length > LIMITS.RESULTS) throw new Error("逐條結果超過上限");
  for (const item of results) {
    if (!isObject(item) || typeof item.index !== "number" || typeof item.op !== "string" || !isPath(item.path) || !STATUSES.has(item.status as string)) {
      throw new Error("逐條結果形狀不對");
    }
  }
  if (!Array.isArray(changes) || changes.length > LIMITS.CHANGES || !changes.every((item) => isObject(item) && isPath(item.path))) {
    throw new Error("變動清單不對");
  }
  if (typeof value.changesTruncated !== "boolean") throw new Error("回傳形狀不對");
  return value as unknown as MvuDetailedResult;
}

export function checkInitialized(json: string): ZodInitialized {
  const value = parseReply(json);
  if (!isObject(value) || !isObject(value.outcome) || typeof value.outcome.ok !== "boolean" || !isObject(value.stat_data)) {
    throw new Error("開局回傳形狀不對");
  }
  if (bytes(value.stat_data) > LIMITS.TABLE_BYTES) throw new Error("變數表超過上限");
  return value as unknown as ZodInitialized;
}
