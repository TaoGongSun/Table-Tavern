// OpenRouter chat/completions 串流的逐塊解析與收工判定。照桌面版 src-tauri/src/transport/client.rs
// （SseParser :152、extract_delta :334、StreamOutcome :353）、runaway.rs:74（chat_reasoning）、
// stall.rs:95（chat_progress）改寫。只搬 extract_delta 會把「想完沒有 content」當成功，所以收工
// 判定一定要走 StreamOutcome。
import { detailFromErrorObject, type ErrorDetail } from "./api-failure";

/** 逐塊切出 `data:` 承載；TextDecoder 串流模式處理被 chunk 邊界切斷的 UTF-8 字元。 */
export class SseParser {
  private decoder = new TextDecoder();
  private buffer = "";

  push(chunk: Uint8Array): string[] {
    this.buffer += this.decoder.decode(chunk, { stream: true });
    const payloads: string[] = [];
    let index = this.buffer.indexOf("\n");
    while (index >= 0) {
      const line = this.buffer.slice(0, index).replace(/\r$/, "");
      this.buffer = this.buffer.slice(index + 1);
      // 其餘：空行（事件分隔）與 ": comment"（OpenRouter 的處理中心跳）一律忽略
      if (line.startsWith("data:")) payloads.push(line.slice(5).trimStart());
      index = this.buffer.indexOf("\n");
    }
    return payloads;
  }
}

function parse(payload: string): Record<string, unknown> | null {
  try {
    const value = JSON.parse(payload);
    return value && typeof value === "object" ? (value as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function delta(value: Record<string, unknown> | null): Record<string, unknown> | null {
  const choices = value?.choices;
  if (!Array.isArray(choices)) return null;
  const first = choices[0] as Record<string, unknown> | undefined;
  const result = first?.delta;
  return result && typeof result === "object" ? (result as Record<string, unknown>) : null;
}

const nonEmpty = (value: unknown): string | null => (typeof value === "string" && value !== "" ? value : null);

/** 一則 payload 的正文增量；非增量塊回 null。 */
export function extractDelta(payload: string): string | null {
  return nonEmpty(delta(parse(payload))?.content);
}

/** 思考增量：兩欄是同一段文字的相容複製，只取一欄，否則重複計數。 */
export function extractReasoning(payload: string): string | null {
  const part = delta(parse(payload));
  return nonEmpty(part?.reasoning) ?? nonEmpty(part?.reasoning_content);
}

/** 合格進展（延後停滯 deadline）：正文或思考任一非空。保活註解與 role-only 塊不算。 */
export function isProgress(payload: string): boolean {
  const part = delta(parse(payload));
  return ["content", "reasoning", "reasoning_content"].some((key) => nonEmpty(part?.[key]) !== null);
}

/** 實際回應的模型（top-level model）；缺欄不猜。 */
export function extractModel(payload: string): string | null {
  const model = parse(payload)?.model;
  return typeof model === "string" && model.trim() !== "" ? model : null;
}

/** 串流全程累積的收工訊號：error 塊、finish_reason、有沒有見到 [DONE]。 */
export class StreamOutcome {
  error: string | null = null;
  errorDetail: ErrorDetail | null = null;
  finishReason: string | null = null;
  reasoningTokens: number | null = null;
  sawDone = false;

  absorb(payload: string): void {
    const value = parse(payload);
    if (!value) return;
    const error = value.error;
    if (error !== undefined && error !== null) {
      this.errorDetail = detailFromErrorObject(error);
      const message = (error as Record<string, unknown>).message;
      this.error = typeof message === "string" && message.trim() !== "" ? message : JSON.stringify(error);
    }
    const choices = value.choices;
    const reason = Array.isArray(choices) ? (choices[0] as Record<string, unknown> | undefined)?.finish_reason : null;
    if (typeof reason === "string") this.finishReason = reason;
    const usage = value.usage as Record<string, unknown> | undefined;
    const details = usage?.completion_tokens_details as Record<string, unknown> | undefined;
    if (typeof details?.reasoning_tokens === "number") this.reasoningTokens = details.reasoning_tokens;
  }

  /** 收工判定，回錯誤字串或 null（成功）。優先序：供應商原話 → 內容過濾 → 不完整 → 正文空。 */
  failure(text: string, model: string): string | null {
    if (this.error !== null) return this.error;
    const reason = this.finishReason;
    const diagnosis = `model=${model} finish_reason=${reason ?? "(none)"}${
      this.reasoningTokens !== null ? ` reasoning_tokens=${this.reasoningTokens}` : ""
    }`;
    const hasText = text.trim() !== "";
    if ((reason === "content_filter" || reason === "length") && hasText) return null;
    if (reason === "content_filter") return `AI_CONTENT_FILTERED: ${diagnosis}`;
    if (reason !== null && reason !== "stop") return `AI_INCOMPLETE_RESPONSE: ${diagnosis}`;
    if (reason === null && !this.sawDone) return `AI_INCOMPLETE_RESPONSE: ${diagnosis}`;
    if (!hasText) return `AI_EMPTY_RESPONSE: ${diagnosis}`;
    return null;
  }

  /** 有正文時，內容過濾與長度上限保留正文，但標成中途截斷。 */
  truncation(text: string): string | null {
    if (text.trim() === "") return null;
    return this.finishReason === "content_filter" || this.finishReason === "length" ? this.finishReason : null;
  }
}
