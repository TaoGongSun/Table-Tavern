// 單支模型的 chat/completions 串流（穩定免費每發只帶一支）。照桌面版
// src-tauri/src/transport/client.rs:664（stream_chat_models_windowed）改寫：停滯逾時、失控上限、
// 收工判定都在這裡；取消＝外部 AbortSignal，取消後一個字都不再交給呼叫端。
import {
  failureFromHttp,
  failureNetwork,
  failureRunaway,
  failureStalled,
  failureStream,
  type ApiFailure,
} from "./api-failure";
import { RunawayGuard } from "./runaway-guard";
import { extractDelta, extractModel, extractReasoning, isProgress, SseParser, StreamOutcome } from "./sse";

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
  /** ST 範例對話的 example_user／example_assistant */
  name?: string;
}

/** 停滯窗口，照桌面版 stall.rs:12-14：200 之後等第一個合格進展 300 秒，之後兩次進展之間 120 秒。 */
export interface StallWindow {
  firstMs: number;
  afterMs: number;
}

export const DEFAULT_STALL_WINDOW: StallWindow = { firstMs: 300_000, afterMs: 120_000 };

export type StreamResult =
  | { kind: "ok"; text: string; model: string | null; truncated: string | null }
  | { kind: "failed"; failure: ApiFailure }
  /** 被呼叫端取消；text 是取消前已交出去的正文。 */
  | { kind: "aborted"; text: string };

export interface StreamRequest {
  fetch: typeof fetch;
  apiBase: string;
  apiKey: string;
  model: string;
  messages: ChatMessage[];
  signal: AbortSignal;
  onDelta: (delta: string) => void;
  window?: StallWindow;
}

const STALLED = Symbol("stalled");

export async function streamChat(request: StreamRequest): Promise<StreamResult> {
  const { signal, model } = request;
  const window = request.window ?? DEFAULT_STALL_WINDOW;
  // 停滯時由這裡中斷連線；外部取消一併傳進來
  const local = new AbortController();
  const forward = () => local.abort();
  if (signal.aborted) return { kind: "aborted", text: "" };
  signal.addEventListener("abort", forward, { once: true });
  let fullText = "";
  try {
    let response: Response;
    try {
      response = await request.fetch(`${request.apiBase}/chat/completions`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${request.apiKey}`,
          "Content-Type": "application/json",
          "X-Title": "Table Tavern Web",
        },
        body: JSON.stringify({ model, messages: request.messages, stream: true }),
        signal: local.signal,
      });
    } catch (error) {
      if (signal.aborted) return { kind: "aborted", text: "" };
      return { kind: "failed", failure: failureNetwork(String(error), false) };
    }
    if (!response.ok) {
      const body = await response.text().catch(() => "");
      if (signal.aborted) return { kind: "aborted", text: "" };
      return { kind: "failed", failure: failureFromHttp(response.status, response.headers, body) };
    }
    if (!response.body) return { kind: "failed", failure: failureNetwork("empty body", false) };

    const reader = response.body.getReader();
    const parser = new SseParser();
    const outcome = new StreamOutcome();
    const textGuard = RunawayGuard.text();
    const thinkingGuard = RunawayGuard.thinking();
    let deadline = Date.now() + window.firstMs;
    let progressed = false;
    let responder: string | null = null;
    let runaway: string | null = null;
    let stalled = false;

    reading: for (;;) {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const timeout = new Promise<typeof STALLED>((resolve) => {
        timer = setTimeout(() => resolve(STALLED), Math.max(0, deadline - Date.now()));
      });
      let next: ReadableStreamReadResult<Uint8Array> | typeof STALLED;
      try {
        next = await Promise.race([reader.read(), timeout]);
      } catch (error) {
        if (signal.aborted) return { kind: "aborted", text: fullText };
        return { kind: "failed", failure: failureNetwork(String(error), fullText !== "") };
      } finally {
        clearTimeout(timer);
      }
      if (signal.aborted) return { kind: "aborted", text: fullText };
      if (next === STALLED) {
        stalled = true;
        break;
      }
      if (next.done) break;
      for (const payload of parser.push(next.value)) {
        if (payload === "[DONE]") {
          outcome.sawDone = true;
          break reading;
        }
        if (isProgress(payload)) {
          progressed = true;
          deadline = Date.now() + window.afterMs;
        }
        outcome.absorb(payload);
        responder = extractModel(payload) ?? responder;
        const reasoning = extractReasoning(payload);
        if (reasoning !== null) {
          const reason = thinkingGuard.push(reasoning);
          if (reason) {
            runaway = thinkingGuard.message(reason);
            break reading;
          }
        }
        const delta = extractDelta(payload);
        if (delta !== null) {
          const reason = textGuard.push(delta);
          if (reason) {
            runaway = textGuard.message(reason);
            break reading;
          }
          request.onDelta(delta);
          fullText += delta;
        }
      }
    }
    local.abort();
    if (runaway !== null) return { kind: "failed", failure: failureRunaway(runaway) };
    if (stalled) {
      const secs = Math.round((progressed ? window.afterMs : window.firstMs) / 1000);
      return { kind: "failed", failure: failureStalled(secs, fullText !== "") };
    }
    const logModel = responder ?? model;
    const failure = outcome.failure(fullText, logModel);
    if (failure !== null) {
      return { kind: "failed", failure: failureStream(failure, outcome.errorDetail, fullText !== "") };
    }
    return { kind: "ok", text: fullText, model: responder, truncated: outcome.truncation(fullText) };
  } finally {
    signal.removeEventListener("abort", forward);
  }
}
