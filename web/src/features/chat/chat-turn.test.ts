import { describe, expect, it } from "vitest";
import { deleteLast, regenerateBase, replaceLast, resolveTurn, type ChatEntry, type PendingTurn } from "./chat-turn";
import { explainError, redactKeys } from "./error-text";

const opening: ChatEntry = { id: "o", role: "char", text: "開場", opening: true };
const user: ChatEntry = { id: "u", role: "user", text: "（揮手）你好" };
const reply: ChatEntry = { id: "r", role: "char", text: "舊回覆" };
const before = [opening, user];
const send: PendingTurn = { kind: "send", userEntry: user, rawInput: "*揮手*你好" };
let next = 0;
const newId = () => `n${(next += 1)}`;
const error = { kind: "error", display: "AI_EMPTY_RESPONSE: x", failure: null, cls: "other", daily: false, failover: null } as const;

describe("resolveTurn after a send", () => {
  it("appends the reply on success, runs the reply through `finish` and flags provider truncation", () => {
    const result = resolveTurn(before, send, { kind: "ok", text: "嗨", model: null, truncated: "length", failover: null }, newId, (text) => `${text}!`);
    expect(result.entries.map((e) => [e.role, e.text, e.interrupted])).toEqual([
      ["char", "開場", undefined],
      ["user", "（揮手）你好", undefined],
      ["char", "嗨!", true],
    ]);
    expect(result.restoreInput).toBeNull();
  });

  it("stop after text: keeps the partial reply as interrupted and keeps the player line", () => {
    const result = resolveTurn(before, send, { kind: "aborted", text: "說到一半", failover: null }, newId);
    expect(result.entries[result.entries.length - 1]).toMatchObject({ role: "char", text: "說到一半", interrupted: true });
    expect(result.entries).toContain(user);
    expect(result.restoreInput).toBeNull();
  });

  it("cancel before any text: takes the player line back and restores the raw input (before regex/macros)", () => {
    const result = resolveTurn(before, send, { kind: "aborted", text: "  ", failover: null }, newId);
    expect(result.entries).toEqual([opening]);
    expect(result.restoreInput).toBe("*揮手*你好");
    expect(result.error).toBeNull();
  });

  it("failure: drops the player line, restores the input and reports the error", () => {
    const result = resolveTurn(before, send, error, newId);
    expect(result.entries).toEqual([opening]);
    expect(result.restoreInput).toBe("*揮手*你好");
    expect(explainError(result.error!)).toContain("沒有回任何內容");
  });
});

describe("regenerate, edit and delete the last message", () => {
  it("regenerate replaces the last reply; no new reply puts the old one back", () => {
    const entries = [opening, user, reply];
    const base = regenerateBase(entries)!;
    expect(base).toEqual({ before: [opening, user], replaced: reply });
    const pending: PendingTurn = { kind: "regenerate", replaced: base.replaced };
    const ok = resolveTurn(base.before, pending, { kind: "ok", text: "新回覆", model: null, truncated: null, failover: null }, newId);
    expect(ok.entries.map((e) => e.text)).toEqual(["開場", "（揮手）你好", "新回覆"]);
    expect(resolveTurn(base.before, pending, { kind: "aborted", text: "", failover: null }, newId).entries).toEqual(entries);
    const failed = resolveTurn(base.before, pending, error, newId);
    expect(failed.entries).toEqual(entries);
    expect(failed.restoreInput).toBeNull();
  });

  it("regenerate after a player line just generates; never regenerates the opening", () => {
    expect(regenerateBase([opening, user])).toEqual({ before: [opening, user], replaced: null });
    expect(regenerateBase([opening])).toBeNull();
    expect(regenerateBase([])).toBeNull();
  });

  it("edit and delete only touch the last message", () => {
    expect(replaceLast([opening, user], "改過")).toEqual([opening, { ...user, text: "改過" }]);
    expect(deleteLast([opening, user])).toEqual([opening]);
    expect(deleteLast([])).toEqual([]);
  });
});

describe("error text", () => {
  it("never echoes an API key", () => {
    expect(redactKeys("bad key sk-or-v1-abcDEF123 here")).toBe("bad key sk-or-… here");
    expect(explainError("weird sk-or-v1-secret")).not.toContain("secret");
  });
});
