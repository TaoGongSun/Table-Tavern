import { describe, expect, it } from "vitest";
import { extractDelta, extractModel, extractReasoning, isProgress, SseParser, StreamOutcome } from "./sse";

const encode = (text: string) => new TextEncoder().encode(text);

describe("SseParser", () => {
  it("joins payloads split across chunks and ignores comments and blank lines", () => {
    const parser = new SseParser();
    expect(parser.push(encode(": OPENROUTER PROCESSING\n\ndata: {\"a\""))).toEqual([]);
    expect(parser.push(encode(":1}\r\n\ndata: [DONE]\n"))).toEqual(['{"a":1}', "[DONE]"]);
  });

  it("keeps a multi-byte character that is cut by a chunk boundary", () => {
    const bytes = encode('data: {"t":"雪"}\n');
    const parser = new SseParser();
    const cut = bytes.indexOf(0xe9) + 1; // 「雪」的第一個位元組之後
    expect(parser.push(bytes.slice(0, cut))).toEqual([]);
    expect(parser.push(bytes.slice(cut))).toEqual(['{"t":"雪"}']);
  });
});

describe("payload helpers", () => {
  const chunk = (delta: object, extra: object = {}) => JSON.stringify({ choices: [{ delta }], ...extra });

  it("reads content, reasoning (one field only) and responder model", () => {
    expect(extractDelta(chunk({ content: "嗨" }))).toBe("嗨");
    expect(extractDelta(chunk({ content: "" }))).toBeNull();
    expect(extractReasoning(chunk({ reasoning: "想", reasoning_content: "想" }))).toBe("想");
    expect(extractReasoning(chunk({ reasoning_content: "想" }))).toBe("想");
    expect(extractModel(chunk({}, { model: "x/y:free" }))).toBe("x/y:free");
  });

  it("counts only content or reasoning as progress", () => {
    expect(isProgress(chunk({ role: "assistant" }))).toBe(false);
    expect(isProgress(chunk({ reasoning: "…" }))).toBe(true);
    expect(isProgress("not json")).toBe(false);
  });
});

describe("StreamOutcome", () => {
  const absorbAll = (payloads: object[], sawDone = true) => {
    const outcome = new StreamOutcome();
    for (const payload of payloads) outcome.absorb(JSON.stringify(payload));
    outcome.sawDone = sawDone;
    return outcome;
  };

  it("treats thinking-without-content as a failure, not a success", () => {
    const outcome = absorbAll([{ choices: [{ delta: { reasoning: "…" }, finish_reason: "stop" }] }]);
    expect(outcome.failure("", "m")).toMatch(/^AI_EMPTY_RESPONSE/);
  });

  it("reports provider errors verbatim and keeps their detail", () => {
    const outcome = absorbAll([{ error: { code: 502, message: "upstream down", metadata: { error_type: "provider_unavailable" } } }]);
    expect(outcome.failure("", "m")).toBe("upstream down");
    expect(outcome.errorDetail).toMatchObject({ code: 502, errorType: "provider_unavailable" });
  });

  it("classifies missing [DONE] without finish_reason as incomplete", () => {
    expect(absorbAll([], false).failure("半句", "m")).toMatch(/^AI_INCOMPLETE_RESPONSE/);
  });

  it("keeps text cut by length or content filter but marks it truncated", () => {
    const outcome = absorbAll([{ choices: [{ delta: {}, finish_reason: "length" }] }]);
    expect(outcome.failure("一些字", "m")).toBeNull();
    expect(outcome.truncation("一些字")).toBe("length");
    expect(outcome.failure("", "m")).toMatch(/^AI_INCOMPLETE_RESPONSE/);
  });
});
