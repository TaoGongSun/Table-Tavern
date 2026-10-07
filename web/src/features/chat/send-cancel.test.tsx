// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { SAMPLE_PLAY_CARD } from "../cards/sample-card";
import { FailoverRuntime } from "../openrouter/failover";
import { FreePool } from "../openrouter/free-pool";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { useChat, type ChatController, type GameSetup } from "./useChat";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

/** /key 掛住，直到測試手動放行；其餘請求都記下來。 */
function controlledSession() {
  const calls: string[] = [];
  let releaseKey: (response: Response) => void = () => {};
  const fetchImpl = ((input: RequestInfo | URL) => {
    const url = String(input);
    calls.push(url);
    if (url.endsWith("/key")) return new Promise<Response>((resolve) => (releaseKey = resolve));
    return Promise.resolve(new Response("{}", { status: 404 }));
  }) as typeof fetch;
  const deps = { fetch: fetchImpl, apiBase: "http://fake/api/v1" };
  const session: OpenRouterSession = {
    apiKey: "sk-or-v1-test",
    connecting: false,
    notice: null,
    quota: { kind: "counted", limit: 50, remaining: 25 },
    deps,
    pool: new FreePool(deps),
    runtime: new FailoverRuntime(),
    connect: async () => {},
    adoptKey: () => {},
    logout: () => {},
    refreshQuota: async () => {},
    quotaEvent: () => {},
  };
  const release = () =>
    releaseKey(new Response(JSON.stringify({ data: { free_model_daily_requests: { limit: 50, remaining: 25 } } })));
  return { session, calls, release };
}

const GAME: GameSetup = { card: SAMPLE_PLAY_CARD, userName: "玩家", openingIndex: 0 };

let root: Root | null = null;
let chat: ChatController;

function Probe({ session }: { session: OpenRouterSession }) {
  chat = useChat(GAME, session);
  return null;
}

async function mount(session: OpenRouterSession) {
  root = createRoot(document.createElement("div"));
  await act(async () => root!.render(<Probe session={session} />));
}

const flush = () => act(async () => new Promise((resolve) => setTimeout(resolve, 10)));

afterEach(async () => {
  await act(async () => root?.unmount());
  root = null;
});

describe("useChat cancellation during the pre-send /key check", () => {
  it("stop while /key hangs: no chat request, input kept, no player line", async () => {
    const { session, calls, release } = controlledSession();
    await mount(session);
    await act(async () => chat.setInput("你好"));
    await act(async () => void chat.send());
    expect(chat.busy).toBe(true);
    await act(async () => chat.stop());
    await act(async () => release());
    await flush();
    expect(calls.filter((url) => url.includes("/chat/completions"))).toEqual([]);
    expect(calls.filter((url) => url.includes("/models"))).toEqual([]);
    expect(chat.input).toBe("你好");
    expect(chat.entries.map((entry) => entry.role)).toEqual(["char"]);
    expect(chat.busy).toBe(false);
  });

  it("unmount while /key hangs: no chat request afterwards", async () => {
    const { session, calls, release } = controlledSession();
    await mount(session);
    await act(async () => chat.setInput("你好"));
    await act(async () => void chat.send());
    await act(async () => root!.unmount());
    root = null;
    await act(async () => release());
    await flush();
    expect(calls.filter((url) => url.includes("/chat/completions"))).toEqual([]);
    expect(calls.filter((url) => url.includes("/models"))).toEqual([]);
    expect(chat.input).toBe("你好");
  });
});

const OLD = Math.floor(Date.now() / 1000) - 30 * 86_400;
const freeModel = (id: string) => ({
  id,
  name: id,
  created: OLD,
  context_length: 65_536,
  pricing: { prompt: "0", completion: "0" },
  architecture: { input_modalities: ["text"], output_modalities: ["text"] },
});

/** 依金鑰分帳號：hangKey 的 /models/user 掛住直到放行，其餘立即回各自的免費清單。 */
function twoAccountFetch(hangKey: string) {
  const calls: string[] = [];
  let releaseCatalog: () => void = () => {};
  const catalogFor = (key: string) =>
    key === "sk-or-v1-old" ? [freeModel("old/model:free")] : [freeModel("new/model:free")];
  const json = (body: unknown) => new Response(JSON.stringify(body));
  const fetchImpl = ((input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    calls.push(url);
    const auth = (init?.headers as Record<string, string> | undefined)?.Authorization ?? "";
    const key = auth.replace("Bearer ", "");
    if (url.endsWith("/key")) {
      return Promise.resolve(json({ data: { free_model_daily_requests: { limit: 50, remaining: 25 } } }));
    }
    if (url.endsWith("/models/user")) {
      const body = json({ data: catalogFor(key) });
      if (key !== hangKey) return Promise.resolve(body);
      return new Promise<Response>((resolve) => (releaseCatalog = () => resolve(body)));
    }
    if (url.includes("/endpoints")) return Promise.resolve(json({ data: { endpoints: [{ provider_name: "P" }] } }));
    if (url.includes("/models")) return Promise.resolve(json({ data: [] }));
    return Promise.resolve(new Response("{}", { status: 404 }));
  }) as typeof fetch;
  return { fetchImpl, calls, release: () => releaseCatalog() };
}

function sessionWith(fetchImpl: typeof fetch, apiKey: string): OpenRouterSession {
  const deps = { fetch: fetchImpl, apiBase: "http://fake/api/v1" };
  return { ...controlledSession().session, apiKey, deps, pool: new FreePool(deps), runtime: new FailoverRuntime() };
}

describe("useChat and the shared model pool", () => {
  it("cancel during pool.refresh: no plan, runtime untouched", async () => {
    const { fetchImpl, calls, release } = twoAccountFetch("sk-or-v1-old");
    const session = sessionWith(fetchImpl, "sk-or-v1-old");
    await mount(session);
    await act(async () => chat.setInput("你好"));
    await act(async () => void chat.send());
    await flush();
    expect(calls.some((url) => url.endsWith("/models/user"))).toBe(true);
    await act(async () => chat.stop());
    await act(async () => release());
    await flush();
    expect(calls.filter((url) => url.includes("/chat/completions"))).toEqual([]);
    expect(session.runtime.epoch).toBe(0);
    expect(session.runtime.model).toBe("");
    expect(chat.input).toBe("你好");
    expect(chat.entries.map((entry) => entry.role)).toEqual(["char"]);
  });

  it("old account lookup resolves after new account loaded: new lineup and runtime untouched", async () => {
    const { fetchImpl, calls, release } = twoAccountFetch("sk-or-v1-old");
    const session = sessionWith(fetchImpl, "sk-or-v1-old");
    await mount(session);
    await act(async () => chat.setInput("你好"));
    await act(async () => void chat.send());
    await flush();
    // 登出：畫面卸載、選模狀態作廢（同 useOpenRouterSession 的 logout）
    await act(async () => root!.unmount());
    root = null;
    session.runtime.invalidate();
    session.pool.invalidate();
    const epochAfterLogout = session.runtime.epoch;
    // 新帳號登入，同一個 FreePool 載入完成；舊帳號的查詢這時才回來
    const now = Math.floor(Date.now() / 1000);
    await session.pool.refresh("sk-or-v1-new", now);
    await act(async () => release());
    await flush();
    const plan = session.pool.plan(["嗨"], now);
    expect(plan.lineup).toEqual(["new/model:free"]);
    expect(plan.names.has("old/model:free")).toBe(false);
    expect(session.runtime.epoch).toBe(epochAfterLogout);
    expect(session.runtime.model).toBe("");
    expect(calls.filter((url) => url.includes("/chat/completions"))).toEqual([]);
  });
});
