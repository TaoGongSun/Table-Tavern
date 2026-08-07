import { describe, expect, it } from "vitest";
import {
  assembleRefactorOutcome,
  buildRefactorPersonPlan,
  buildSharedEntryDraws,
  defaultRefactorSelection,
  localConvertPerson,
  mergeRefactorInterfaces,
  parseRefactorOutcome,
  refactorSummaryCounts,
  setPlayerIndex,
  sourceEntryTitle,
  sourceEntryTitles,
  toggleIndex,
  unselectCharacter,
  type RefactorCharacter,
  type RefactorMechanism,
  type RefactorOutcome,
  type RefactorSurveyOutcome,
} from "./refactor-review";

function makeOutcome(overrides: Partial<RefactorOutcome> = {}): RefactorOutcome {
  return {
    characters: [],
    interface: null,
    mechanisms: [],
    deletable_shared_uids: [],
    ...overrides,
  };
}

function makeCharacter(sourceUids: string[], overrides: Partial<RefactorCharacter> = {}): RefactorCharacter {
  return {
    name: "阿福",
    emoji: "🍺",
    public_md: "",
    private_md: "",
    source_uids: sourceUids,
    solo_entry_md: "",
    suspected_player: false,
    ...overrides,
  };
}

function makeMechanism(sourceUid: string): RefactorMechanism {
  return { source_uid: sourceUid, rules: {}, triggers: [] };
}

function makeSurvey(overrides: Partial<RefactorSurveyOutcome> = {}): RefactorSurveyOutcome {
  return { persons: [], interface_uids: [], mechanism_uids: [], raw: "", ...overrides };
}

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

describe("localConvertPerson", () => {
  const entries = [
    { uid: 12, content: "  亞瑟：劍術高超。 \n" },
    { uid: 30, content: "霍玄一段。長老一段。" },
  ];

  it("單一專屬來源：直接拿條目內容當公開設定，trim 掉前後空白，PRIVATE 留空", () => {
    const character = localConvertPerson({ name: "亞瑟", uids: ["12"], is_player: false }, entries);
    expect(character).toEqual({
      name: "亞瑟",
      emoji: "🎭",
      public_md: "亞瑟：劍術高超。",
      private_md: "",
      source_uids: ["12"],
      solo_entry_md: "亞瑟：劍術高超。",
      suspected_player: false,
    });
  });

  it("疑似玩家旗標原樣帶進 suspected_player", () => {
    const character = localConvertPerson({ name: "亞瑟", uids: ["12"], is_player: true }, entries);
    expect(character?.suspected_player).toBe(true);
  });

  it("多來源（uids 長度 >1）不是本地轉換的範圍，回 null", () => {
    expect(localConvertPerson({ name: "霍玄", uids: ["30", "31"], is_player: false }, entries)).toBeNull();
  });

  it("uid 對不到任何條目（條目已刪或資料不一致）回 null", () => {
    expect(localConvertPerson({ name: "查無此人", uids: ["999"], is_player: false }, entries)).toBeNull();
  });
});

describe("buildRefactorPersonPlan", () => {
  const entries = [
    { uid: 1, content: "亞瑟專屬設定" },
    { uid: 2, content: "亞瑟性格" },
    { uid: 3, content: "小華專屬設定" },
    { uid: 4, content: "共用速覽：霍玄一段、長老一段" },
  ];

  it("單一專屬來源的人走本地轉換，不進展開佇列", () => {
    const survey = makeSurvey({ persons: [{ name: "小華", uids: ["3"], is_player: false }] });
    const { local, queue } = buildRefactorPersonPlan(survey, entries);
    expect(local).toHaveLength(1);
    expect(local[0].name).toBe("小華");
    expect(queue).toHaveLength(0);
  });

  it("多來源的人（自己專屬條目＋共用速覽）進展開佇列，帶齊全部來源 uid", () => {
    const survey = makeSurvey({
      persons: [
        { name: "亞瑟", uids: ["1", "2"], is_player: false },
        { name: "小華", uids: ["3"], is_player: false },
      ],
    });
    const { local, queue } = buildRefactorPersonPlan(survey, entries);
    expect(local.map((c) => c.name)).toEqual(["小華"]);
    expect(queue).toEqual([{ name: "亞瑟", uids: ["1", "2"], is_player: false }]);
  });

  it("唯一來源但那條被別人共用（合集）：不算專屬，一樣進展開佇列", () => {
    const survey = makeSurvey({
      persons: [
        { name: "霍玄", uids: ["4"], is_player: false },
        { name: "長老", uids: ["4"], is_player: false },
      ],
    });
    const { local, queue } = buildRefactorPersonPlan(survey, entries);
    expect(local).toHaveLength(0);
    expect(queue.map((item) => item.name)).toEqual(["霍玄", "長老"]);
  });

  it("本地轉換找不到對應條目（資料不一致）：退回展開佇列，不悄悄漏掉這個人", () => {
    const survey = makeSurvey({ persons: [{ name: "查無此人", uids: ["999"], is_player: false }] });
    const { local, queue } = buildRefactorPersonPlan(survey, entries);
    expect(local).toHaveLength(0);
    expect(queue).toEqual([{ name: "查無此人", uids: ["999"], is_player: false }]);
  });
});

describe("buildSharedEntryDraws", () => {
  it("uid 只被一人列為來源＝專屬，不算共用，不出現在清單裡", () => {
    const survey = makeSurvey({ persons: [{ name: "小華", uids: ["3"], is_player: false }] });
    expect(buildSharedEntryDraws(survey)).toEqual([]);
  });

  it("uid 被兩人以上列為來源＝共用，整理成「已被誰抽走」清單", () => {
    const survey = makeSurvey({
      persons: [
        { name: "霍玄", uids: ["4", "5"], is_player: false },
        { name: "長老", uids: ["4"], is_player: false },
      ],
    });
    expect(buildSharedEntryDraws(survey)).toEqual([{ uid: "4", drawn_by: ["霍玄", "長老"] }]);
  });
});

describe("assembleRefactorOutcome", () => {
  it("三段呼叫的候選＋收尾判定組成最終產物，介面走多條合併規則", () => {
    const outcome = assembleRefactorOutcome({
      characters: [makeCharacter(["1"])],
      interfaces: [{ state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" }],
      mechanisms: [makeMechanism("19")],
      deletableSharedUids: ["4"],
    });
    expect(outcome).toEqual({
      characters: [makeCharacter(["1"])],
      interface: { state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" },
      mechanisms: [makeMechanism("19")],
      deletable_shared_uids: ["4"],
    });
  });

  it("三段都空＝空殼 outcome（介面 null，其餘空陣列）", () => {
    expect(assembleRefactorOutcome({ characters: [], interfaces: [], mechanisms: [], deletableSharedUids: [] })).toEqual(
      makeOutcome(),
    );
  });
});

describe("parseRefactorOutcome", () => {
  it("完整 JSON 原樣解析", () => {
    const outcome = parseRefactorOutcome(JSON.stringify(makeOutcome({ characters: [makeCharacter(["12"])] })));
    expect(outcome.characters).toHaveLength(1);
    expect(outcome.characters[0].source_uids).toEqual(["12"]);
  });

  it("缺頂層鍵比照後端 #[serde(default)] 補空陣列／null", () => {
    expect(parseRefactorOutcome("{}")).toEqual(makeOutcome());
  });

  it("格式錯誤的 JSON 丟例外", () => {
    expect(() => parseRefactorOutcome("{not json")).toThrow();
  });
});

describe("defaultRefactorSelection", () => {
  it("全勾：角色與機制 indices 是 0..N-1，有介面產物就 apply_interface=true，沒人疑似玩家就 player_index=null", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter(["12"]), makeCharacter(["12"]), makeCharacter(["30"])],
      interface: { state_fields: {}, source_uids: ["8"], raw: "" },
      mechanisms: [makeMechanism("19"), makeMechanism("20")],
    });
    expect(defaultRefactorSelection(outcome)).toEqual({
      character_indices: [0, 1, 2],
      apply_interface: true,
      mechanism_indices: [0, 1],
      player_index: null,
    });
  });

  it("沒有介面產物＝ apply_interface 預設不勾", () => {
    expect(defaultRefactorSelection(makeOutcome()).apply_interface).toBe(false);
  });

  it("有人被盤點階段標記疑似玩家：預設指定他為玩家卡", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter(["1"]), makeCharacter(["2"], { suspected_player: true })],
    });
    expect(defaultRefactorSelection(outcome).player_index).toBe(1);
  });
});

describe("refactorSummaryCounts", () => {
  it("三區都有產物", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter(["12"])],
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

describe("sourceEntryTitles", () => {
  const entries = [
    { uid: 12, title: "人物设定" },
    { uid: 13, title: "性格" },
  ];

  it("多條來源逐條查標題後用「、」接起來", () => {
    expect(sourceEntryTitles(entries, ["12", "13"])).toBe("人物设定、性格");
  });

  it("單一來源就是單一標題", () => {
    expect(sourceEntryTitles(entries, ["12"])).toBe("人物设定");
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

describe("setPlayerIndex", () => {
  const base = { character_indices: [0], apply_interface: false, mechanism_indices: [], player_index: null };

  it("指定已勾選的角色為玩家：character_indices 不變", () => {
    expect(setPlayerIndex(base, 0)).toEqual({ ...base, player_index: 0 });
  });

  it("指定還沒勾選的角色為玩家：順手把他加進 character_indices（沒有卡不能是玩家卡）", () => {
    expect(setPlayerIndex(base, 1)).toEqual({ ...base, character_indices: [0, 1], player_index: 1 });
  });

  it("index=null＝不指定玩家，character_indices 不變", () => {
    expect(setPlayerIndex({ ...base, player_index: 0 }, null)).toEqual({ ...base, player_index: null });
  });
});

describe("unselectCharacter", () => {
  it("取消勾選的角色不是目前指定的玩家：player_index 不受影響", () => {
    const selection = { character_indices: [0, 1], apply_interface: false, mechanism_indices: [], player_index: 0 };
    expect(unselectCharacter(selection, 1)).toEqual({ ...selection, character_indices: [0], player_index: 0 });
  });

  it("取消勾選的角色正是目前指定的玩家：一併清掉玩家指定", () => {
    const selection = { character_indices: [0, 1], apply_interface: false, mechanism_indices: [], player_index: 1 };
    expect(unselectCharacter(selection, 1)).toEqual({ ...selection, character_indices: [0], player_index: null });
  });
});
