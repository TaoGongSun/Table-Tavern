// @vitest-environment happy-dom
// 一個回合碰到上下文上限：每支模型照自己的上限裁切、固定段落放不下就不送、平台說太長要講清楚。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { playCardFromValue } from "../cards/play-card";
import { bodySaysTooLong, failureFromHttp } from "../openrouter/api-failure";
import { FailoverRuntime } from "../openrouter/failover";
import { FreePool } from "../openrouter/free-pool";
import { runSmartCall, type CallPlan } from "../openrouter/smart-call";
import { streamChat, type ChatMessage, type StreamResult } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { t } from "../../i18n";
import { EMPTY_CARRY } from "../saves/web-save-codec";
import type { ChatEntry } from "./chat-turn";
import { useChat, type ChatController } from "./useChat";

// 只換掉每日次數查詢；模型清單照真的走（下面用真 FreePool 的測試要）
vi.mock("../openrouter/openrouter-api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../openrouter/openrouter-api")>()),
  fetchFreeDaily: async () => null,
}));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

const ok: StreamResult = { kind: "ok", text: "reply", model: null, truncated: null };
const busy: StreamResult = {
  kind: "failed",
  failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503,"message":"busy"}}'), emittedText: false },
};
const tooLong: StreamResult = {
  kind: "failed",
  failure: failureFromHttp(
    400,
    new Headers(),
    JSON.stringify({ error: { code: 400, message: "This endpoint's maximum context length is 8192 tokens. However, you requested about 9000 tokens." } }),
  ),
};

/** 名單 A→B；`context` 是各模型的上下文上限（保留輸出 4096 另扣）。選模走真正的「放得下固定段落」判斷。 */
async function turn(context: Record<string, number>, results: StreamResult[], history: ChatEntry[] = [], runtime = new FailoverRuntime()) {
  const card = playCardFromValue("json", { name: "C", description: "描述".repeat(40) });
  const session = {
    apiKey: "sk-or-v1-test",
    quota: { kind: "unknown" },
    deps: { fetch, apiBase: "http://fake/api/v1" },
    runtime,
    pool: {
      refresh: async () => {},
      plan: (holds: (model: string) => boolean): CallPlan => ({ account: "acct", lineup: ["A", "B"], others: [], fits: holds, names: new Map() }),
      contextLength: (id: string) => context[id] ?? 0,
      tokenizer: () => "Qwen",
    },
    quotaEvent: () => {},
    refreshQuota: async () => {},
  } as unknown as OpenRouterSession;
  const queue = [...results];
  const sent: { model: string; messages: ChatMessage[] }[] = [];
  vi.mocked(streamChat).mockReset();
  vi.mocked(streamChat).mockImplementation(async ({ model, messages }) => {
    sent.push({ model, messages: structuredClone(messages) });
    return queue.shift() ?? ok;
  });
  let chat!: ChatController;
  function Probe() {
    chat = useChat({ card, userName: "U", openingIndex: null, resume: history.length ? { entries: history, local: {}, carry: EMPTY_CARRY } : undefined }, session);
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  await act(async () => chat.setInput("hello"));
  await act(async () => chat.send());
  const after = { sent, entries: chat.entries, input: chat.input, error: chat.error };
  await act(async () => root.unmount());
  return after;
}

describe("context limits in a turn", () => {
  it("skips a model whose budget cannot hold the fixed prompts and sends to one that can", async () => {
    // 描述約 80 字（Qwen 結構估 ~82 token）：A 的預算只剩 50，B 放得下
    const result = await turn({ A: 4096 + 50, B: 8192 }, [ok]);
    expect(result.sent.map((call) => call.model)).toEqual(["B"]);
    expect(result.entries.map((entry) => entry.text)).toEqual(["hello", "reply"]);
  });

  it("fixed prompts exceed every model: nothing is sent and the line goes back to the input", async () => {
    const result = await turn({ A: 4096 + 50, B: 4096 + 60 }, []);
    expect(result.sent).toEqual([]);
    expect(result.error).toBe(t("errPromptTooLarge"));
    expect(result.input).toBe("hello");
    expect(result.entries).toEqual([]);
  });

  it("the failover shot is trimmed to the second model's own limit", async () => {
    const history: ChatEntry[] = Array.from({ length: 30 }, (_, index) => ({
      id: `h${index}`,
      role: index % 2 === 0 ? "char" : "user",
      text: `第${index}則：${"字".repeat(30)}`,
    }));
    // 先讓 A 失敗一次，這一句的失敗才會讓換模成立（同 dispatch-prompt 測試）
    const runtime = new FailoverRuntime();
    const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: () => true, names: new Map() };
    await runSmartCall(plan, runtime, {
      send: async () => busy,
      dailyRemaining: async () => 10,
      signal: new AbortController().signal,
      now: () => Math.floor(Date.now() / 1000),
    });
    const result = await turn({ A: 4096 + 400, B: 65536 }, [busy, ok], history, runtime);
    expect(result.sent.map((call) => call.model)).toEqual(["A", "B"]);
    const kept = (messages: ChatMessage[]) => messages.filter((message) => message.role !== "system").length;
    // A 只放得下最近幾則；B 全放（30 則歷史＋玩家這句）
    expect(kept(result.sent[0].messages)).toBeLessThan(31);
    expect(kept(result.sent[0].messages)).toBeGreaterThan(0);
    expect(kept(result.sent[1].messages)).toBe(31);
    // 留下的是最新的那幾則
    expect(result.sent[0].messages[result.sent[0].messages.length - 1].content).toBe("hello");
  });

  it("the platform rejects the prompt as too long: explained, not swallowed, and the line goes back", async () => {
    const result = await turn({ A: 65536, B: 65536 }, [tooLong]);
    expect(result.sent.map((call) => call.model)).toEqual(["A"]);
    expect(result.error).toBe(t("errContextTooLong"));
    expect(result.input).toBe("hello");
    expect(result.entries).toEqual([]);
  });
});

describe("recognising the platform's context overflow (same rule as desktop context_overflow.rs)", () => {
  it.each([
    [400, '{"error":{"message":"This model\'s maximum context length is 8192 tokens."}}', true],
    [400, '{"error":{"code":"context_length_exceeded","message":"too big"}}', true],
    [413, '{"error":{"message":"Prompt is too long"}}', true],
    [400, '[{"error":{"message":"The input token count (9000) exceeds the maximum number of tokens allowed (8192)."}}]', true],
    // 只看 400／413 的 error 物件，不在其他狀態或全文裡掃字樣
    [500, '{"error":{"message":"maximum context length"}}', false],
    [400, "maximum context length", false],
    [400, '{"error":{"message":"invalid model"}}', false],
  ])("status %i %s → %s", (status, body, expected) => {
    expect(bodySaysTooLong(status, body)).toBe(expected);
  });
});

describe("choosing models with the real free pool", () => {
  const OLD = Math.floor(Date.now() / 1000) - 30 * 86_400;
  const freeModel = (id: string, contextLength: number, created = OLD) => ({
    id,
    created,
    context_length: contextLength,
    pricing: { prompt: "0", completion: "0" },
    architecture: { input_modalities: ["text"], output_modalities: ["text"], tokenizer: "Qwen" },
  });

  async function sendWith(catalog: unknown[]) {
    const json = (body: unknown) => new Response(JSON.stringify(body));
    const fetchImpl = ((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/key")) return Promise.resolve(json({ data: { free_model_daily_requests: { limit: 50, remaining: 25 } } }));
      if (url.endsWith("/models/user")) return Promise.resolve(json({ data: catalog }));
      if (url.includes("/endpoints")) return Promise.resolve(json({ data: { endpoints: [{ provider_name: "P" }] } }));
      if (url.includes("/models")) return Promise.resolve(json({ data: [] }));
      return Promise.resolve(new Response("{}", { status: 404 }));
    }) as typeof fetch;
    const deps = { fetch: fetchImpl, apiBase: "http://fake/api/v1" };
    const session = {
      apiKey: "sk-or-v1-test",
      quota: { kind: "unknown" },
      deps,
      runtime: new FailoverRuntime(),
      pool: new FreePool(deps),
      quotaEvent: () => {},
      refreshQuota: async () => {},
    } as unknown as OpenRouterSession;
    vi.mocked(streamChat).mockReset();
    vi.mocked(streamChat).mockResolvedValue(ok);
    const card = playCardFromValue("json", { name: "C" });
    let chat!: ChatController;
    function Probe() {
      chat = useChat({ card, userName: "U", openingIndex: null }, session);
      return null;
    }
    const root = createRoot(document.createElement("div"));
    await act(async () => root.render(<Probe />));
    await act(async () => chat.setInput("hello"));
    await act(async () => chat.send());
    const after = { error: chat.error, input: chat.input, entries: chat.entries, sent: vi.mocked(streamChat).mock.calls.length };
    await act(async () => root.unmount());
    return after;
  }

  it("stable models below output reserve are prompt overflow", async () => {
    const result = await sendWith([freeModel("a/small:free", 2048), freeModel("b/small:free", 2048)]);
    expect(result.sent).toBe(0);
    expect(result.error).toBe(t("errPromptTooLarge"));
    expect(result.input).toBe("hello");
    expect(result.entries).toEqual([]);
  });

  it("no stable free model at all is still 'no free model'", async () => {
    // 上架未滿 7 天的不算穩定
    const result = await sendWith([freeModel("a/fresh:free", 65_536, Math.floor(Date.now() / 1000) - 86_400)]);
    expect(result.sent).toBe(0);
    expect(result.error).toBe(t("errNoFreeModel"));
    expect(result.input).toBe("hello");
  });

  it("a stable model that holds the prompt is still picked next to too-small ones", async () => {
    const result = await sendWith([freeModel("a/small:free", 2048), freeModel("b/large:free", 65_536)]);
    expect(result.sent).toBe(1);
    expect(vi.mocked(streamChat).mock.calls[0][0].model).toBe("b/large:free");
    expect(result.error).toBeNull();
  });
});
