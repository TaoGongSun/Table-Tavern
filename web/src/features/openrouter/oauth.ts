// OpenRouter 一鍵授權（PKCE，計畫 2.4）。順序寫死：回呼參數讀進記憶體→立刻 replaceState 清網址→
// 驗 state→交換一次。verifier 與一次性 state 只存同一分頁的 sessionStorage，驗後即消耗。
// PKCE 本體共用桌面版（D2）。
import { createOpenRouterPkce } from "@desktop/features/ai-connection/openrouter-onboarding";
import { exchangeCode, type ApiDeps } from "./openrouter-api";

const PENDING_KEY = "tt-web:oauth-pending";
/** 回呼網址上帶的 state 參數名；OpenRouter 會在後面接 `code`。 */
const STATE_PARAM = "tt_oauth";

export interface OAuthCallback {
  state: string;
  code: string | null;
}

function randomState(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** 產生 PKCE 與 state，回傳要導去的授權網址。 */
export async function beginAuthorization(
  location: Pick<Location, "origin" | "pathname">,
  storage: Storage,
  authUrl: string,
): Promise<string> {
  const { verifier, challenge } = await createOpenRouterPkce();
  const state = randomState();
  storage.setItem(PENDING_KEY, JSON.stringify({ state, verifier }));
  const callback = `${location.origin}${location.pathname}?${STATE_PARAM}=${state}`;
  const url = new URL(authUrl);
  url.searchParams.set("callback_url", callback);
  url.searchParams.set("code_challenge", challenge);
  url.searchParams.set("code_challenge_method", "S256");
  return url.toString();
}

/**
 * 頁面一載入就呼叫（React 掛載前）：網址帶回呼參數就讀進記憶體並立刻清掉網址，錯誤或取消也清。
 * 不是回呼就回 null、不動網址。
 */
export function takeCallback(win: Pick<Window, "location" | "history">): OAuthCallback | null {
  const params = new URLSearchParams(win.location.search);
  if (!params.has(STATE_PARAM) && !params.has("code")) return null;
  const callback = { state: params.get(STATE_PARAM) ?? "", code: params.get("code") };
  win.history.replaceState(null, "", `${win.location.pathname}${win.location.hash}`);
  return callback;
}

/** 驗 state 並換金鑰。pending 先消耗再交換，同一組回呼不可能換第二次。 */
export async function completeAuthorization(
  callback: OAuthCallback,
  storage: Storage,
  deps: ApiDeps,
): Promise<string> {
  const raw = storage.getItem(PENDING_KEY);
  storage.removeItem(PENDING_KEY);
  let pending: { state?: unknown; verifier?: unknown } | null = null;
  try {
    pending = raw ? JSON.parse(raw) : null;
  } catch {
    pending = null;
  }
  if (!pending || typeof pending.verifier !== "string" || pending.state !== callback.state || !callback.state) {
    throw new Error("oauth_state");
  }
  if (!callback.code) throw new Error("oauth_cancelled");
  return exchangeCode(deps, callback.code, pending.verifier);
}
