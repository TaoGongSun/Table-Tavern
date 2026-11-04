// 信任邊界（協定 §10）的訊框外形：一張表掃完。
import { describe, expect, it } from "vitest";
import { validateFrame } from "./validate";

const ok = (t: string, p: unknown) => JSON.stringify({ t, id: "r1", p });
const CASES: Array<[string, "member" | "executor", string, boolean]> = [
  ["合法的成員訊框", "member", ok("action.submit", { round: 2, text: "嗨" }), true],
  ["ping 沒有 p", "member", '{"t":"ping"}', true],
  ["多的欄位忽略", "member", ok("seat.claim", { cardId: "c1", extra: 1 }), true],
  [
    "可為 null 的欄位",
    "executor",
    ok("gen.failed", { dispatchId: "d", atPart: null, error: {} }),
    true,
  ],
  ["不是 JSON", "member", "{nope", false],
  ["不是物件", "member", "[1]", false],
  ["未知的 t", "member", ok("room.nuke", {}), false],
  ["執行端專屬的 t 從成員來", "member", ok("gen.done", { dispatchId: "d", aborted: false }), false],
  ["原型鏈上的鍵不算白名單", "member", ok("toString", {}), false],
  ["缺欄位", "member", ok("action.submit", { round: 2 }), false],
  ["型別不對", "member", ok("action.submit", { round: "2", text: "x" }), false],
  ["負數回合號", "member", ok("action.skip", { round: -1 }), false],
  ["不可為 null 的欄位給 null", "member", ok("seat.claim", { cardId: null }), false],
  ["請求 id 格式不對", "member", JSON.stringify({ t: "ping", id: "有空白 的id" }), false],
  ["孤立代理字元", "member", '{"t":"action.draft","p":{"text":"\\ud800"}}', false],
  ["成員訊框超過 16 KiB", "member", ok("action.draft", { text: "字".repeat(6000) }), false],
  [
    "執行端訊框 1 MiB 內可以",
    "executor",
    ok("gen.delta", { dispatchId: "d", part: 0, text: "字".repeat(300_000) }),
    true,
  ],
];

describe("validateFrame", () => {
  it.each(CASES)("%s", (_name, from, raw, expected) => {
    const result = validateFrame(raw, from);
    expect(result.ok).toBe(expected);
    if (!result.ok) expect(result.code).toBe("E_BAD_FRAME");
  });
});
