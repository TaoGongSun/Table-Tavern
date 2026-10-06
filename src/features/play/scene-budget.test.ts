import { describe, expect, it } from "vitest";
import {
  budgetTokens,
  capacityHint,
  draftSize,
  wouldOverflow,
  type SceneBudgetReply,
  type SummaryBudget,
} from "./scene-budget";

const summary = (used: number, cap: number, gReply: number, lockable = true): SummaryBudget => ({
  unit: "bytes",
  used,
  cap,
  hint: true,
  ratio: 1,
  reliable: true,
  lockable,
  gReply,
  over: used > cap,
});

describe("scene-budget（與 Rust scene_budget 同一算法）", () => {
  it("保守估計與 Rust 對拍（src-tauri/src/scene_budget/tests.rs 同一組數字）", () => {
    expect(budgetTokens('雷恩說：Let\'s go! {"hp":3}')).toBe(13);
    expect(draftSize('雷恩說：Let\'s go! {"hp":3}', "tokens", 1.5)).toBe(20 + 16);
    expect(draftSize("測a", "bytes", 1)).toBe(4 + 64);
    expect(draftSize("", "bytes", 1)).toBe(0);
  });

  it("鎖：剛好塞滿不鎖、差一就鎖；無玩家句的動作本句 0；不可鎖永不鎖", () => {
    const s = summary(900, 1000, 36);
    // 本句「ab」＝2＋64＝66：900＋66＋36＝1002 > 1000
    expect(wouldOverflow(s, "ab")).toBe(true);
    expect(wouldOverflow(summary(900, 1002, 36), "ab")).toBe(false);
    expect(wouldOverflow(summary(900, 1000, 100), "")).toBe(false);
    expect(wouldOverflow(summary(901, 1000, 100), "")).toBe(true);
    expect(wouldOverflow(summary(5000, 1000, 100, false), "很長")).toBe(false);
    expect(wouldOverflow(null, "x")).toBe(false);
    // 長草稿自己就超過：打字當下就鎖
    expect(wouldOverflow(summary(0, 1000, 100), "x".repeat(900))).toBe(true);
  });

  it("提醒種類：換幕觸發優先，只因聊天觸發用中性說法", () => {
    const reply = (summaryHint: boolean, chatHint: boolean): SceneBudgetReply => ({
      worldId: "w",
      configGen: "g",
    configTag: "t",
      requestSeq: 1,
      scene: 0,
      summary: { ...summary(1, 10, 1), hint: summaryHint },
      chatHint,
    });
    expect(capacityHint(reply(true, true))).toBe("summary");
    expect(capacityHint(reply(false, true))).toBe("chat");
    expect(capacityHint(reply(false, false))).toBe(null);
    expect(capacityHint(null)).toBe(null);
  });
});
