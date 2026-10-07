import { describe, expect, it } from "vitest";
import { SAMPLE_CARD } from "../cards/sample-card";
import { resolveTurn, type ChatEntry } from "./chat-turn";
import { explainError, redactKeys } from "./error-text";
import { buildMessages } from "./prompt";

const opening: ChatEntry = { id: "o", role: "char", text: "開場", opening: true };
const user: ChatEntry = { id: "u", role: "user", text: "你好" };
const before = [opening, user];
let next = 0;
const newId = () => `n${(next += 1)}`;

describe("resolveTurn", () => {
  it("appends the reply on success and flags provider truncation", () => {
    const result = resolveTurn(before, user, { kind: "ok", text: "嗨", model: null, truncated: "length", failover: null }, newId);
    expect(result.entries.map((e) => [e.role, e.text, e.interrupted])).toEqual([
      ["char", "開場", undefined],
      ["user", "你好", undefined],
      ["char", "嗨", true],
    ]);
    expect(result.restoreInput).toBeNull();
  });

  it("stop after text: keeps the partial reply as interrupted and keeps the player line", () => {
    const result = resolveTurn(before, user, { kind: "aborted", text: "說到一半", failover: null }, newId);
    expect(result.entries[result.entries.length - 1]).toMatchObject({ role: "char", text: "說到一半", interrupted: true });
    expect(result.entries).toContain(user);
    expect(result.restoreInput).toBeNull();
  });

  it("cancel before any text: takes the player line back into the input box", () => {
    const result = resolveTurn(before, user, { kind: "aborted", text: "  ", failover: null }, newId);
    expect(result.entries).toEqual([opening]);
    expect(result.restoreInput).toBe("你好");
    expect(result.error).toBeNull();
  });

  it("failure: drops the player line, restores the input and reports the error", () => {
    const result = resolveTurn(
      before,
      user,
      { kind: "error", display: "AI_EMPTY_RESPONSE: x", failure: null, cls: "other", daily: false, failover: null },
      newId,
    );
    expect(result.entries).toEqual([opening]);
    expect(result.restoreInput).toBe("你好");
    expect(explainError(result.error!)).toContain("沒有回任何內容");
  });
});

describe("error text", () => {
  it("never echoes an API key", () => {
    expect(redactKeys("bad key sk-or-v1-abcDEF123 here")).toBe("bad key sk-or-… here");
    expect(explainError("weird sk-or-v1-secret")).not.toContain("secret");
  });
});

describe("buildMessages", () => {
  it("fills names and keeps ST chat-completion order", () => {
    const messages = buildMessages(SAMPLE_CARD.data, "旅人", [opening, user]);
    expect(messages[0]).toEqual({ role: "system", content: "Write 瑟拉's next reply in a fictional chat between 瑟拉 and 旅人." });
    expect(messages.some((m) => m.content.includes("{{"))).toBe(false);
    expect(messages.slice(-2)).toEqual([
      { role: "assistant", content: "開場" },
      { role: "user", content: "你好" },
    ]);
  });
});
