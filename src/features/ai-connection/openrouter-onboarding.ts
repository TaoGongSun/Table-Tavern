import type { OpenRouterOnboardingMsgKey } from "../../i18n/features/openrouter-onboarding";

export const OPENROUTER_ONBOARDING_ERROR_CODES = [
  "openrouter_oauth_browser",
  "openrouter_oauth_timeout",
  "openrouter_oauth_callback",
  "openrouter_oauth_state",
  "openrouter_oauth_cancelled",
  "openrouter_oauth_network",
  "openrouter_oauth_exchange",
  "openrouter_oauth_save",
  "openrouter_oauth_pkce",
  "openrouter_oauth_crypto",
  "openrouter_key_empty",
] as const;

type ErrorCode = (typeof OPENROUTER_ONBOARDING_ERROR_CODES)[number];

const ERROR_KEYS = {
  openrouter_oauth_browser: "onboardErrBrowser",
  openrouter_oauth_timeout: "onboardErrTimeout",
  openrouter_oauth_callback: "onboardErrCallback",
  openrouter_oauth_state: "onboardErrCallback",
  openrouter_oauth_cancelled: "onboardErrCancelled",
  openrouter_oauth_network: "onboardErrNetwork",
  openrouter_oauth_exchange: "onboardErrExchange",
  openrouter_oauth_save: "onboardErrSave",
  openrouter_oauth_pkce: "onboardErrCrypto",
  openrouter_oauth_crypto: "onboardErrCrypto",
  openrouter_key_empty: "onboardErrKeyEmpty",
} satisfies Record<ErrorCode, OpenRouterOnboardingMsgKey>;

const BASE64URL = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

function base64Url(bytes: Uint8Array): string {
  let result = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index];
    const second = bytes[index + 1];
    const third = bytes[index + 2];
    const value = (first << 16) | ((second ?? 0) << 8) | (third ?? 0);
    result += BASE64URL[(value >>> 18) & 63];
    result += BASE64URL[(value >>> 12) & 63];
    if (second !== undefined) result += BASE64URL[(value >>> 6) & 63];
    if (third !== undefined) result += BASE64URL[value & 63];
  }
  return result;
}

export async function pkceChallengeForVerifier(verifier: string): Promise<string> {
  const crypto = globalThis.crypto;
  if (!crypto?.subtle) throw new Error("openrouter_oauth_crypto");
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64Url(new Uint8Array(digest));
}

export async function createOpenRouterPkce(): Promise<{ verifier: string; challenge: string }> {
  const crypto = globalThis.crypto;
  if (!crypto?.getRandomValues || !crypto?.subtle) throw new Error("openrouter_oauth_crypto");
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  const verifier = base64Url(bytes);
  return { verifier, challenge: await pkceChallengeForVerifier(verifier) };
}

export function openRouterOnboardingErrorKey(reason: unknown): OpenRouterOnboardingMsgKey {
  const raw = reason instanceof Error ? reason.message : String(reason);
  const code = raw.replace(/^Error:\s*/, "").trim() as ErrorCode;
  return ERROR_KEYS[code] ?? "onboardErrUnknown";
}
