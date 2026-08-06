import { describe, expect, it } from "vitest";
import {
  defaultRefactorSelection,
  parseRefactorOutcome,
  refactorSummaryCounts,
  sourceEntryTitle,
  toggleIndex,
  type RefactorCharacter,
  type RefactorMechanism,
  type RefactorOutcome,
} from "./refactor-review";

function makeOutcome(overrides: Partial<RefactorOutcome> = {}): RefactorOutcome {
  return {
    characters: [],
    interface: null,
    mechanisms: [],
    rewrites: [],
    ...overrides,
  };
}

function makeCharacter(sourceUid: string): RefactorCharacter {
  return { name: "阿福", emoji: "🍺", public_md: "", private_md: "", source_uid: sourceUid, solo_entry_md: "" };
}

function makeMechanism(sourceUid: string): RefactorMechanism {
  return { source_uid: sourceUid, rules: {}, triggers: [] };
}

describe("parseRefactorOutcome", () => {
  it("完整 JSON 原樣解析", () => {
    const outcome = parseRefactorOutcome(JSON.stringify(makeOutcome({ characters: [makeCharacter("12")] })));
    expect(outcome.characters).toHaveLength(1);
    expect(outcome.characters[0].source_uid).toBe("12");
  });

  it("缺頂層鍵比照後端 #[serde(default)] 補空陣列／null", () => {
    expect(parseRefactorOutcome("{}")).toEqual(makeOutcome());
  });

  it("格式錯誤的 JSON 丟例外", () => {
    expect(() => parseRefactorOutcome("{not json")).toThrow();
  });
});

describe("defaultRefactorSelection", () => {
  it("全勾：角色與機制 indices 是 0..N-1，有介面產物就 apply_interface=true", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter("12"), makeCharacter("12"), makeCharacter("30")],
      interface: { state_fields: {}, source_uids: ["8"], raw: "" },
      mechanisms: [makeMechanism("19"), makeMechanism("20")],
    });
    expect(defaultRefactorSelection(outcome)).toEqual({
      character_indices: [0, 1, 2],
      apply_interface: true,
      mechanism_indices: [0, 1],
    });
  });

  it("沒有介面產物＝ apply_interface 預設不勾", () => {
    expect(defaultRefactorSelection(makeOutcome()).apply_interface).toBe(false);
  });
});

describe("refactorSummaryCounts", () => {
  it("三區都有產物", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter("12")],
      interface: { state_fields: {}, source_uids: [], raw: "" },
      mechanisms: [makeMechanism("19")],
    });
    expect(refactorSummaryCounts(outcome)).toEqual({ characters: 1, hasInterface: true, mechanisms: 1 });
  });

  it("空產物三區皆零／false", () => {
    expect(refactorSummaryCounts(makeOutcome())).toEqual({ characters: 0, hasInterface: false, mechanisms: 0 });
  });
});

describe("sourceEntryTitle", () => {
  const entries = [
    { uid: 12, title: "旅店常客" },
    { uid: 30, title: "" },
  ];

  it("uid 對得到就回標題", () => {
    expect(sourceEntryTitle(entries, "12")).toBe("旅店常客");
  });

  it("uid 對得到但標題是空字串＝兜底顯示 uid", () => {
    expect(sourceEntryTitle(entries, "30")).toBe("30");
  });

  it("uid 對不到（條目已刪）＝兜底顯示 uid", () => {
    expect(sourceEntryTitle(entries, "999")).toBe("999");
  });
});

describe("toggleIndex", () => {
  it("勾選加入 indices", () => {
    expect(toggleIndex([0, 2], 1, true)).toEqual([0, 2, 1]);
  });

  it("取消從 indices 移除", () => {
    expect(toggleIndex([0, 1, 2], 1, false)).toEqual([0, 2]);
  });
});
