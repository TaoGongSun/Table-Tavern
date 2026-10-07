import { describe, expect, it } from "vitest";
import { streamChat, type StreamRequest } from "./stream-chat";

const sse = (payload: object | string) => `data: ${typeof payload === "string" ? payload : JSON.stringify(payload)}\n\n`;
const delta = (content: string) => sse({ model: "real/model", choices: [{ delta: { content } }] });
const stop = sse({ choices: [{ delta: {}, finish_reason: "stop" }] });

/** 依序吐出 chunks；`hang` 為真時吐完不關閉（模擬上游卡住）。 */
function fakeFetch(chunks: string[], options: { status?: number; hang?: boolean; body?: string } = {}): typeof fetch {
  return (async (_url: RequestInfo | URL, init?: RequestInit) => {
    if (options.status && options.status !== 200) {
      return new Response(options.body ?? "{}", { status: options.status });
    }
    const encoder = new TextEncoder();
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
        if (!options.hang) controller.close();
        init?.signal?.addEventListener("abort", () => controller.error(new DOMException("aborted", "AbortError")));
      },
    });
    return new Response(stream, { status: 200 });
  }) as typeof fetch;
}

function request(fetchImpl: typeof fetch, extra: Partial<StreamRequest> = {}): StreamRequest & { seen: string[] } {
  const seen: string[] = [];
  return {
    fetch: fetchImpl,
    apiBase: "http://fake/api/v1",
    apiKey: "sk-or-test",
    model: "m:free",
    messages: [{ role: "user", content: "嗨" }],
    signal: new AbortController().signal,
    onDelta: (text) => seen.push(text),
    seen,
    ...extra,
  };
}

describe("streamChat", () => {
  it("streams deltas and reports the responding model", async () => {
    const req = request(fakeFetch([delta("你"), delta("好"), stop, sse("[DONE]")]));
    expect(await streamChat(req)).toEqual({ kind: "ok", text: "你好", model: "real/model", truncated: null });
    expect(req.seen).toEqual(["你", "好"]);
  });

  it("turns non-2xx into a structured failure", async () => {
    const result = await streamChat(request(fakeFetch([], { status: 429, body: '{"error":{"code":429}}' })));
    expect(result).toMatchObject({ kind: "failed", failure: { status: 429, emittedText: false } });
  });

  it("gives up when no progress arrives inside the stall window", async () => {
    const req = request(fakeFetch([": OPENROUTER PROCESSING\n\n"], { hang: true }), {
      window: { firstMs: 30, afterMs: 30 },
    });
    const result = await streamChat(req);
    expect(result).toMatchObject({ kind: "failed", failure: { stage: "timeout", emittedText: false } });
  });

  it("marks a stall after some text as emitted", async () => {
    const req = request(fakeFetch([delta("半")], { hang: true }), { window: { firstMs: 30, afterMs: 30 } });
    expect(await streamChat(req)).toMatchObject({ kind: "failed", failure: { emittedText: true } });
  });

  it("stops a runaway whitespace stream", async () => {
    const req = request(fakeFetch([delta(" ".repeat(2_500)), stop, sse("[DONE]")]));
    const result = await streamChat(req);
    expect(result.kind === "failed" && result.failure.display).toMatch(/^AI_OUTPUT_RUNAWAY: reason=whitespace_run/);
  });

  it("returns aborted with the text shown so far and delivers nothing afterwards", async () => {
    const controller = new AbortController();
    const req = request(fakeFetch([delta("前半")], { hang: true }), { signal: controller.signal });
    const pending = streamChat(req);
    await new Promise((resolve) => setTimeout(resolve, 20));
    controller.abort();
    expect(await pending).toEqual({ kind: "aborted", text: "前半" });
    expect(req.seen).toEqual(["前半"]);
  });

  it("returns aborted with no text when stopped before anything arrived", async () => {
    const controller = new AbortController();
    const pending = streamChat(request(fakeFetch([], { hang: true }), { signal: controller.signal }));
    await new Promise((resolve) => setTimeout(resolve, 10));
    controller.abort();
    expect(await pending).toEqual({ kind: "aborted", text: "" });
  });
});
