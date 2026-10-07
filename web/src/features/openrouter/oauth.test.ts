import { describe, expect, it } from "vitest";
import { pkceChallengeForVerifier } from "@desktop/features/ai-connection/openrouter-onboarding";
import { MemoryStorage } from "./memory-storage";
import { beginAuthorization, completeAuthorization, takeCallback } from "./oauth";

function fakeWindow(search: string) {
  const replaced: string[] = [];
  const win = {
    location: { search, pathname: "/play/", hash: "" } as Location,
    history: { replaceState: (_s: unknown, _t: string, url: string) => replaced.push(url) } as unknown as History,
  };
  return { win, replaced };
}

describe("PKCE authorization", () => {
  it("sends an S256 challenge and a one-time state parameter, with a query-free callback URL", async () => {
    const storage = new MemoryStorage();
    const url = new URL(await beginAuthorization({ origin: "https://tt.example", pathname: "/" }, storage, "https://auth.example/auth"));
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    // OpenRouter 導回時會丟掉 callback_url 自己的 query，state 要走授權網址的 state 參數
    expect(url.searchParams.get("callback_url")).toBe("https://tt.example/");
    const state = url.searchParams.get("state")!;
    expect(state).toMatch(/^[0-9a-f]{32}$/);
    const pending = JSON.parse(storage.getItem("tt-web:oauth-pending")!);
    expect(pending.state).toBe(state);
    expect(url.searchParams.get("code_challenge")).toBe(await pkceChallengeForVerifier(pending.verifier));
  });

  it("reads the callback into memory and clears the URL, including on cancel", () => {
    const { win, replaced } = fakeWindow("?code=xyz&state=abc");
    expect(takeCallback(win)).toEqual({ state: "abc", code: "xyz" });
    expect(replaced).toEqual(["/play/"]);
    const cancelled = fakeWindow("?state=abc&error=access_denied");
    expect(takeCallback(cancelled.win)).toEqual({ state: "abc", code: null });
    expect(cancelled.replaced).toEqual(["/play/"]);
    // 只帶 code（state 被丟掉）也當回呼、照樣清網址，之後驗 state 失敗
    const codeOnly = fakeWindow("?code=xyz");
    expect(takeCallback(codeOnly.win)).toEqual({ state: "", code: "xyz" });
    expect(codeOnly.replaced).toEqual(["/play/"]);
    const plain = fakeWindow("");
    expect(takeCallback(plain.win)).toBeNull();
    expect(plain.replaced).toEqual([]);
  });

  it("rejects a mismatched state and consumes the pending entry either way", async () => {
    const storage = new MemoryStorage();
    storage.setItem("tt-web:oauth-pending", JSON.stringify({ state: "good", verifier: "v" }));
    const deps = { fetch: (async () => new Response("{}")) as typeof fetch, apiBase: "http://fake" };
    await expect(completeAuthorization({ state: "evil", code: "c" }, storage, deps)).rejects.toThrow("oauth_state");
    expect(storage.getItem("tt-web:oauth-pending")).toBeNull();
  });

  it("rejects a callback that carries a code but no state, without exchanging", async () => {
    const storage = new MemoryStorage();
    storage.setItem("tt-web:oauth-pending", JSON.stringify({ state: "good", verifier: "v" }));
    let calls = 0;
    const deps = { fetch: (async () => (calls++, new Response("{}"))) as typeof fetch, apiBase: "http://fake" };
    const callback = takeCallback(fakeWindow("?code=xyz").win)!;
    await expect(completeAuthorization(callback, storage, deps)).rejects.toThrow("oauth_state");
    expect(calls).toBe(0);
    expect(storage.getItem("tt-web:oauth-pending")).toBeNull();
  });

  it("exchanges the code exactly once", async () => {
    const storage = new MemoryStorage();
    storage.setItem("tt-web:oauth-pending", JSON.stringify({ state: "s", verifier: "ver" }));
    const bodies: unknown[] = [];
    const deps = {
      fetch: (async (_url: RequestInfo | URL, init?: RequestInit) => {
        bodies.push(JSON.parse(String(init?.body)));
        return new Response(JSON.stringify({ key: "sk-or-v1-new" }));
      }) as typeof fetch,
      apiBase: "http://fake",
    };
    const callback = takeCallback(fakeWindow("?code=c&state=s").win)!;
    expect(await completeAuthorization(callback, storage, deps)).toBe("sk-or-v1-new");
    await expect(completeAuthorization({ state: "s", code: "c" }, storage, deps)).rejects.toThrow("oauth_state");
    expect(bodies).toEqual([{ code: "c", code_verifier: "ver", code_challenge_method: "S256" }]);
  });
});
