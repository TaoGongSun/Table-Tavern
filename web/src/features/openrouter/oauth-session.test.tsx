// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryStorage } from "./memory-storage";
import { useOpenRouterSession, type OpenRouterSession } from "./useOpenRouterSession";

const KEY_SLOT = "tt-web:openrouter-key";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let releaseExchange: (response: Response) => void = () => {};

beforeEach(() => {
  vi.stubGlobal("localStorage", new MemoryStorage());
  vi.stubGlobal("sessionStorage", new MemoryStorage());
  // 換金鑰掛住，直到測試放行；/key 一律回空
  vi.stubGlobal(
    "fetch",
    (input: RequestInfo | URL) =>
      String(input).endsWith("/auth/keys")
        ? new Promise<Response>((resolve) => (releaseExchange = resolve))
        : Promise.resolve(new Response("{}")),
  );
});

let root: Root | null = null;
let session: OpenRouterSession;

function Probe({ callback }: { callback: { state: string; code: string | null } }) {
  session = useOpenRouterSession(callback);
  return null;
}

async function mountWithPendingExchange() {
  window.localStorage.setItem(KEY_SLOT, "sk-or-v1-old");
  window.sessionStorage.setItem("tt-web:oauth-pending", JSON.stringify({ state: "s", verifier: "v" }));
  root = createRoot(document.createElement("div"));
  await act(async () => root!.render(<Probe callback={{ state: "s", code: "c" }} />));
}

const flush = () => act(async () => new Promise((resolve) => setTimeout(resolve, 10)));
const exchanged = () => releaseExchange(new Response(JSON.stringify({ key: "sk-or-v1-late" })));

afterEach(async () => {
  await act(async () => root?.unmount());
  root = null;
  vi.unstubAllGlobals();
});

describe("OAuth exchange is voided by later key decisions", () => {
  it("logout while the exchange hangs: the late key is not adopted or stored", async () => {
    await mountWithPendingExchange();
    expect(session.apiKey).toBe("sk-or-v1-old");
    await act(async () => session.logout());
    await act(async () => exchanged());
    await flush();
    expect(session.apiKey).toBeNull();
    expect(window.localStorage.getItem(KEY_SLOT)).toBeNull();
    expect(session.connecting).toBe(false);
  });

  it("pasting a key while the exchange hangs: the pasted key wins", async () => {
    await mountWithPendingExchange();
    await act(async () => session.adoptKey("sk-or-v1-pasted"));
    await act(async () => exchanged());
    await flush();
    expect(session.apiKey).toBe("sk-or-v1-pasted");
    expect(window.localStorage.getItem(KEY_SLOT)).toBe("sk-or-v1-pasted");
  });

  it("control: without interference the exchanged key is adopted", async () => {
    await mountWithPendingExchange();
    await act(async () => exchanged());
    await flush();
    expect(session.apiKey).toBe("sk-or-v1-late");
    expect(window.localStorage.getItem(KEY_SLOT)).toBe("sk-or-v1-late");
  });
});
