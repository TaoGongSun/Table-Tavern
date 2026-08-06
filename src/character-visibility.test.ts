import { describe, expect, it } from "vitest";
import { isCharacterHidden } from "./character-visibility";

describe("isCharacterHidden", () => {
  const base = { id: "c1", archived: false, auto_hidden: false };

  it("一般卡（沒封存也沒自動隱藏）＝主區", () => {
    expect(isCharacterHidden(base, new Set())).toBe(false);
  });

  it("手動封存＝隱藏，不管本幕有沒有出場", () => {
    expect(isCharacterHidden({ ...base, archived: true }, new Set(["c1"]))).toBe(true);
  });

  it("自動隱藏且本幕沒出場＝隱藏", () => {
    expect(isCharacterHidden({ ...base, auto_hidden: true }, new Set())).toBe(true);
  });

  it("自動隱藏但本幕已出場＝主區（劇情帶上場，立刻從隱藏區移回）", () => {
    expect(isCharacterHidden({ ...base, auto_hidden: true }, new Set(["c1"]))).toBe(false);
  });

  it("自動隱藏＋已封存＝隱藏（封存優先，不因為出場而跑進主區）", () => {
    expect(isCharacterHidden({ ...base, archived: true, auto_hidden: true }, new Set(["c1"]))).toBe(true);
  });
});
