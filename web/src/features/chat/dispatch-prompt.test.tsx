// @vitest-environment happy-dom
// 一個回合實際派送出去的提示：巨集看到的是這一發的模型與上限，第 0 則照 ST 在卡欄位之後寫回，
// 試組與換模不重複提交巨集副作用。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { playCardFromValue } from "../cards/play-card";
import { failureFromHttp } from "../openrouter/api-failure";
import { FailoverRuntime } from "../openrouter/failover";
import { runSmartCall, type CallPlan } from "../openrouter/smart-call";
import { streamChat, type ChatMessage, type StreamResult } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { useChat, type ChatController } from "./useChat";

vi.mock("../openrouter/openrouter-api", () => ({ fetchFreeDaily: async () => null }));
vi.mock("../openrouter/stream-chat", () => ({ streamChat: vi.fn() }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

const CONTEXT: Record<string, number> = { A: 8192, B: 65536 };
const ok: StreamResult = { kind: "ok", text: "reply", model: null, truncated: null };
const busy: StreamResult = {
  kind: "failed",
  failure: { ...failureFromHttp(503, new Headers(), '{"error":{"code":503,"message":"busy"}}'), emittedText: false },
};

/** 送一句；`results` 依序是每一發的結果。回傳每一發的模型與 messages、回合後的對話。 */
async function sendTurn(data: unknown, options: { fits: string[]; results: StreamResult[]; runtime?: FailoverRuntime }) {
  const card = playCardFromValue("json", data);
  const plan: CallPlan = { account: "acct", lineup: ["A", "B"], others: [], fits: (model) => options.fits.includes(model), names: new Map() };
  const session = {
    apiKey: "sk-or-v1-test",
    quota: { kind: "unknown" },
    deps: { fetch: fetch, apiBase: "http://fake/api/v1" },
    runtime: options.runtime ?? new FailoverRuntime(),
    pool: { refresh: async () => {}, plan: () => plan, contextLength: (id: string) => CONTEXT[id] ?? 0, tokenizer: () => null },
    quotaEvent: () => {},
    refreshQuota: async () => {},
  } as unknown as OpenRouterSession;
  const results = [...options.results];
  const sent: { model: string; messages: ChatMessage[] }[] = [];
  vi.mocked(streamChat).mockReset();
  vi.mocked(streamChat).mockImplementation(async ({ model, messages }) => {
    sent.push({ model, messages: structuredClone(messages) });
    return results.shift() ?? ok;
  });
  let chat!: ChatController;
  function Probe() {
    chat = useChat({ card, userName: "U", openingIndex: card.openings.length ? 0 : null }, session);
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  await act(async () => chat.setInput("hello"));
  await act(async () => chat.send());
  const after = { entries: chat.entries, variables: chat.setup.variables.local.values };
  await act(async () => root.unmount());
  return { sent, ...after, plan };
}

const contents = (messages: ChatMessage[]) => messages.map((message) => message.content);

describe("macros in the dispatched prompt", () => {
  it("actual dispatched model and limits must match their macros", async () => {
    // A 放不下本句、實際派 B：提示裡是 B 與它的上限
    const card = { name: "C", scenario: "{{model}}|{{maxContext}}|{{maxResponse}}", description: "{{incvar::n}}desc" };
    const single = await sendTurn(card, { fits: ["B"], results: [ok] });
    expect(single.sent.map((call) => call.model)).toEqual(["B"]);
    expect(contents(single.sent[0].messages)).toContain("B|65536|4096");
    expect(single.variables.n).toBe(1);

    // 換模第二發：第一發 A 失敗換到 B，B 那一發用 B 重組；巨集副作用只落一次
    const runtime = new FailoverRuntime();
    await runSmartCall({ ...single.plan, fits: () => true }, runtime, {
      send: async () => busy,
      dailyRemaining: async () => 10,
      signal: new AbortController().signal,
      now: () => Math.floor(Date.now() / 1000),
    });
    const failover = await sendTurn(card, { fits: ["A", "B"], results: [busy, ok], runtime });
    expect(failover.sent.map((call) => call.model)).toEqual(["A", "B"]);
    expect(contents(failover.sent[0].messages)).toContain("A|8192|4096");
    expect(contents(failover.sent[1].messages)).toContain("B|65536|4096");
    expect(contents(failover.sent[1].messages)).not.toContain("A|8192|4096");
    expect(failover.variables.n).toBe(1);
    expect(failover.entries.map((entry) => entry.text)).toEqual(["hello", "reply"]);
  });

  it("ST evaluates card fields before settling message zero", async () => {
    const { sent, entries } = await sendTurn(
      { name: "C", description: "{{setvar::x::READY}}description", first_mes: "value={{getvar::x}}" },
      { fits: ["A", "B"], results: [ok] },
    );
    expect(entries[0].text).toBe("value=READY");
    expect(contents(sent[0].messages)).toContain("value=READY");
  });
});
