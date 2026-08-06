import { describe, expect, it } from "vitest";
import {
  buildRefactorExpandQueue,
  defaultRefactorSelection,
  mergeRefactorExpandResults,
  mergeRefactorInterfaces,
  parseRefactorOutcome,
  refactorSummaryCounts,
  sourceEntryTitle,
  toggleIndex,
  type RefactorCharacter,
  type RefactorExpandOutcome,
  type RefactorMechanism,
  type RefactorOutcome,
  type RefactorSurveyOutcome,
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

function makeSurvey(overrides: Partial<RefactorSurveyOutcome> = {}): RefactorSurveyOutcome {
  return { persons: [], interface_uids: [], mechanism_uids: [], raw: "", ...overrides };
}

function makeExpandOutcome(overrides: Partial<RefactorExpandOutcome> = {}): RefactorExpandOutcome {
  return { characters: [], rewrite: null, interface: null, mechanism: null, raw: "", ...overrides };
}

describe("buildRefactorExpandQueue", () => {
  it("人物合集→介面→機制依序展開，各自標好 kind（序列 await 靠這個順序建 system 快取）", () => {
    const queue = buildRefactorExpandQueue(
      makeSurvey({
        persons: [{ uid: "1", names: ["阿福"] }, { uid: "2", names: [] }],
        interface_uids: ["8"],
        mechanism_uids: ["19", "20"],
      }),
    );
    expect(queue).toEqual([
      { uid: "1", kind: "person" },
      { uid: "2", kind: "person" },
      { uid: "8", kind: "interface" },
      { uid: "19", kind: "mechanism" },
      { uid: "20", kind: "mechanism" },
    ]);
  });

  it("三區都空＝空佇列", () => {
    expect(buildRefactorExpandQueue(makeSurvey())).toEqual([]);
  });
});

describe("mergeRefactorInterfaces", () => {
  it("零條回傳 null", () => {
    expect(mergeRefactorInterfaces([])).toBeNull();
  });

  it("state_fields 兩邊都是物件＝淺合併、後蓋前；source_uids 串聯；raw 空行接起來", () => {
    const merged = mergeRefactorInterfaces([
      { state_fields: { hp: 10, mp: 5 }, source_uids: ["1"], raw: "第一段" },
      { state_fields: { hp: 20 }, source_uids: ["2"], raw: "第二段" },
    ]);
    expect(merged).toEqual({ state_fields: { hp: 20, mp: 5 }, source_uids: ["1", "2"], raw: "第一段\n\n第二段" });
  });

  it("state_fields 不是物件（解析失敗退原文之類）＝後者整個蓋掉前者", () => {
    const merged = mergeRefactorInterfaces([
      { state_fields: { hp: 10 }, source_uids: [], raw: "" },
      { state_fields: "解析失敗的原文", source_uids: [], raw: "" },
    ]);
    expect(merged?.state_fields).toBe("解析失敗的原文");
  });
});

describe("mergeRefactorExpandResults", () => {
  it("角色與機制全累積、rewrite 過濾掉 null、介面走多條合併規則", () => {
    const outcome = mergeRefactorExpandResults([
      makeExpandOutcome({ characters: [makeCharacter("1")], rewrite: { uid: "1", remainder_md: "剩下的" } }),
      makeExpandOutcome({ interface: { state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" } }),
      makeExpandOutcome({ mechanism: makeMechanism("19") }),
    ]);
    expect(outcome).toEqual({
      characters: [makeCharacter("1")],
      interface: { state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" },
      mechanisms: [makeMechanism("19")],
      rewrites: [{ uid: "1", remainder_md: "剩下的" }],
    });
  });

  it("零條展開結果＝空殼 outcome（介面 null，其餘空陣列）", () => {
    expect(mergeRefactorExpandResults([])).toEqual(makeOutcome());
  });
});

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
