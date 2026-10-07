// OpenRouter 的非聊天端點：免費模型清單、排行、上游、今日免費次數、PKCE 換金鑰。
// 照桌面版 src-tauri/src/smart_free/api.rs 與 openrouter_oauth.rs:171（exchange_code）。都不是模型呼叫，
// 不佔每日免費次數。抓不到一律回 null，由呼叫端保留舊值。
import { parseCatalog, parseRankedIds, parseRankedSlugs, parseUpstreams, type FreeModel } from "./catalog";

const FETCH_TIMEOUT_MS = 15_000;

export interface ApiDeps {
  fetch: typeof fetch;
  apiBase: string;
}

async function getJson(deps: ApiDeps, path: string, apiKey?: string): Promise<unknown | null> {
  try {
    const response = await deps.fetch(`${deps.apiBase}${path}`, {
      headers: apiKey ? { Authorization: `Bearer ${apiKey}` } : {},
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    });
    return response.ok ? await response.json() : null;
  } catch {
    return null;
  }
}

export async function fetchUserCatalog(deps: ApiDeps, apiKey: string): Promise<FreeModel[] | null> {
  return parseCatalog(await getJson(deps, "/models/user", apiKey));
}

export async function fetchWeeklyIds(deps: ApiDeps): Promise<string[] | null> {
  return parseRankedIds(await getJson(deps, "/models?sort=top-weekly"));
}

/** 角色扮演排行：公開、不需金鑰。 */
export async function fetchRoleplaySlugs(deps: ApiDeps): Promise<string[] | null> {
  return parseRankedSlugs(await getJson(deps, "/models?category=roleplay"));
}

export async function fetchUpstreams(deps: ApiDeps, apiKey: string, modelId: string): Promise<string[] | null> {
  const path = `/models/${modelId.split("/").map(encodeURIComponent).join("/")}/endpoints`;
  return parseUpstreams(await getJson(deps, path, apiKey));
}

/** 今日免費次數。`unlimited`＝帳號沒有 free_model_daily_requests（儲值過的帳號）。 */
export type FreeDaily = { kind: "counted"; limit: number; remaining: number } | { kind: "unlimited" };

const integer = (value: unknown): number | null => {
  const number = typeof value === "number" ? value : typeof value === "string" ? Number(value) : Number.NaN;
  return Number.isFinite(number) ? Math.trunc(number) : null;
};

export function freeDailyFromBody(body: unknown): FreeDaily | null {
  if (!body || typeof body !== "object") return null;
  const record = body as Record<string, unknown>;
  const data = (record.data && typeof record.data === "object" ? record.data : record) as Record<string, unknown>;
  const daily = data.free_model_daily_requests;
  if (daily === undefined || daily === null) return { kind: "unlimited" };
  if (typeof daily !== "object") return null;
  const fields = daily as Record<string, unknown>;
  const limit = integer(fields.limit);
  if (limit === null) return null;
  const used = integer(fields.used ?? fields.usage);
  const remaining = integer(fields.remaining) ?? (used === null ? null : Math.max(0, limit - used));
  return remaining === null ? null : { kind: "counted", limit, remaining };
}

export async function fetchFreeDaily(deps: ApiDeps, apiKey: string): Promise<FreeDaily | null> {
  return freeDailyFromBody(await getJson(deps, "/key", apiKey));
}

/** PKCE 換金鑰；失敗丟出錯誤代碼。 */
export async function exchangeCode(deps: ApiDeps, code: string, verifier: string): Promise<string> {
  let response: Response;
  try {
    response = await deps.fetch(`${deps.apiBase}/auth/keys`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code, code_verifier: verifier, code_challenge_method: "S256" }),
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    });
  } catch {
    throw new Error("oauth_network");
  }
  if (!response.ok) throw new Error("oauth_exchange");
  const payload = (await response.json().catch(() => null)) as { key?: unknown } | null;
  const key = typeof payload?.key === "string" ? payload.key.trim() : "";
  if (!key) throw new Error("oauth_exchange");
  return key;
}
