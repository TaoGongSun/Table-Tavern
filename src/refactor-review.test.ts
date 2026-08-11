import { describe, expect, it } from "vitest";
import {
  assembleRefactorOutcome,
  buildRefactorPersonPlan,
  defaultRefactorSelection,
  localConvertPerson,
  mergeRefactorInterfaces,
  parseRefactorOutcome,
  refactorSummaryCounts,
  REFACTOR_IMPORT_INVALID,
  restoreDropped,
  setPlayerIndex,
  sourceEntryTitle,
  sourceEntryTitles,
  toggleIndex,
  unselectCharacter,
  type RefactorCharacter,
  type RefactorMechanism,
  type RefactorNewEntry,
  type RefactorOutcome,
  type RefactorSelection,
  type RefactorSurveyOutcome,
  type RefactorSurveyPerson,
} from "./refactor-review";

function makeOutcome(overrides: Partial<RefactorOutcome> = {}): RefactorOutcome {
  return {
    characters: [],
    interface: null,
    entries: [],
    mechanisms: [],
    deletable_shared_uids: [],
    dropped: [],
    unabsorbed: [],
    audit: [],
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

function makeEntry(overrides: Partial<RefactorNewEntry> = {}): RefactorNewEntry {
  return { title: "酒館規矩", kind: "setting", content: "晚間禁鬥毆。", source_uids: ["9"], rules: {}, triggers: [], ...overrides };
}

function makeSurvey(overrides: Partial<RefactorSurveyOutcome> = {}): RefactorSurveyOutcome {
  return {
    persons: [],
    interface_uids: [],
    playable_interface_uids: [],
    verdicts: [],
    splits: [],
    groups: [],
    fields: [],
    raw: "",
    ...overrides,
  };
}

function makePerson(uids: string[], overrides: Partial<RefactorSurveyPerson> = {}): RefactorSurveyPerson {
  return { name: "阿福", uids, is_player: false, mode: "", spans: [], private_spans: [], ...overrides };
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

  it("渲染殼取最後一個非空的（整份 HTML 沒得合併）", () => {
    const merged = mergeRefactorInterfaces([
      { state_fields: {}, source_uids: [], raw: "", shell: "<p>舊</p>" },
      { state_fields: {}, source_uids: [], raw: "" },
      { state_fields: {}, source_uids: [], raw: "", shell: "<p>新</p>" },
    ]);
    expect(merged?.shell).toBe("<p>新</p>");
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
    const character = localConvertPerson(makePerson(["12"], { name: "亞瑟" }), entries);
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
    const character = localConvertPerson(makePerson(["12"], { name: "亞瑟", is_player: true }), entries);
    expect(character?.suspected_player).toBe(true);
  });

  it("多來源（uids 長度 >1）不是本地轉換的範圍，回 null", () => {
    expect(localConvertPerson(makePerson(["30", "31"], { name: "霍玄" }), entries)).toBeNull();
  });

  it("uid 對不到任何條目（條目已刪或資料不一致）回 null", () => {
    expect(localConvertPerson(makePerson(["999"], { name: "查無此人" }), entries)).toBeNull();
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
    const survey = makeSurvey({ persons: [makePerson(["3"], { name: "小華" })] });
    const { local, queue } = buildRefactorPersonPlan(survey, entries, []);
    expect(local).toHaveLength(1);
    expect(local[0].name).toBe("小華");
    expect(queue).toHaveLength(0);
  });

  it("多來源的人（自己專屬條目＋共用速覽）進展開佇列，帶齊全部來源 uid", () => {
    const survey = makeSurvey({
      persons: [makePerson(["1", "2"], { name: "亞瑟" }), makePerson(["3"], { name: "小華" })],
    });
    const { local, queue } = buildRefactorPersonPlan(survey, entries, []);
    expect(local.map((c) => c.name)).toEqual(["小華"]);
    expect(queue).toEqual([{ name: "亞瑟", uids: ["1", "2"], is_player: false }]);
  });

  it("唯一來源但那條被別人共用（合集）：不算專屬，一樣進展開佇列", () => {
    const survey = makeSurvey({
      persons: [makePerson(["4"], { name: "霍玄" }), makePerson(["4"], { name: "長老" })],
    });
    const { local, queue } = buildRefactorPersonPlan(survey, entries, []);
    expect(local).toHaveLength(0);
    expect(queue.map((item) => item.name)).toEqual(["霍玄", "長老"]);
  });

  it("本地轉換找不到對應條目（資料不一致）：退回展開佇列，不悄悄漏掉這個人", () => {
    const survey = makeSurvey({ persons: [makePerson(["999"], { name: "查無此人" })] });
    const { local, queue } = buildRefactorPersonPlan(survey, entries, []);
    expect(local).toHaveLength(0);
    expect(queue).toEqual([{ name: "查無此人", uids: ["999"], is_player: false }]);
  });

  it("cleanNames 裡的人已由本地零呼叫組裝（mode=clean）產出卡片，整個跳過不重複處理", () => {
    const survey = makeSurvey({
      persons: [makePerson(["1", "2"], { name: "亞瑟", mode: "clean" }), makePerson(["3"], { name: "小華" })],
    });
    const { local, queue } = buildRefactorPersonPlan(survey, entries, ["亞瑟"]);
    expect(local.map((c) => c.name)).toEqual(["小華"]);
    expect(queue).toHaveLength(0);
  });
});

describe("assembleRefactorOutcome", () => {
  it("人物、世界書條目與介面候選組成最終產物，介面走多條合併規則", () => {
    const outcome = assembleRefactorOutcome({
      characters: [makeCharacter(["1"])],
      interfaces: [{ state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" }],
      entries: [makeEntry()],
    });
    expect(outcome).toEqual({
      characters: [makeCharacter(["1"])],
      interface: { state_fields: { hp: 10 }, source_uids: ["8"], raw: "介面段" },
      entries: [makeEntry()],
      mechanisms: [],
      deletable_shared_uids: [],
      dropped: [],
      unabsorbed: [],
      audit: [],
    });
  });

  it("三段都空＝空殼 outcome（介面 null，其餘空陣列）", () => {
    expect(assembleRefactorOutcome({ characters: [], interfaces: [], entries: [] })).toEqual(
      makeOutcome(),
    );
  });

  it("dropped/unabsorbed/audit 透傳；省略時預設空陣列", () => {
    const outcome = assembleRefactorOutcome({
      characters: [],
      interfaces: [],
      entries: [],
      dropped: [{ uid: "5", span: "", title: "舊版本標記", content: "v1.2 更新", rule: 2 }],
      unabsorbed: [{ uid: "16", span: "16#s6", title: "戰鬥流程", note: "擲骰檢定" }],
      audit: [{ kind: "coverage", uid: "9", span: "", detail: "漏網自動補照搬" }],
    });
    expect(outcome.dropped).toEqual([{ uid: "5", span: "", title: "舊版本標記", content: "v1.2 更新", rule: 2 }]);
    expect(outcome.unabsorbed).toEqual([{ uid: "16", span: "16#s6", title: "戰鬥流程", note: "擲骰檢定" }]);
    expect(outcome.audit).toEqual([{ kind: "coverage", uid: "9", span: "", detail: "漏網自動補照搬" }]);
  });
});

describe("parseRefactorOutcome", () => {
  it("完整 JSON 原樣解析", () => {
    const outcome = parseRefactorOutcome(JSON.stringify(makeOutcome({ characters: [makeCharacter(["12"])] })));
    expect(outcome.characters).toHaveLength(1);
    expect(outcome.characters[0].source_uids).toEqual(["12"]);
  });

  it("角色缺選配欄位補空字串／false，不當成壞檔", () => {
    const outcome = parseRefactorOutcome(JSON.stringify({ characters: [{ name: "阿福", source_uids: ["1"] }] }));
    expect(outcome.characters[0]).toEqual(makeCharacter(["1"], { emoji: "", public_md: "", private_md: "" }));
  });

  it("介面的渲染殼一路保留到產物（套用時才寫得出 HTML 殼）", () => {
    const json = JSON.stringify({ interface: { state_fields: { hp: 1 }, source_uids: ["3"], raw: "", shell: "<p>殼</p>" } });
    expect(parseRefactorOutcome(json).interface?.shell).toBe("<p>殼</p>");
  });

  it("新世界書條目解析，省略 rules／triggers 時補預設", () => {
    expect(parseRefactorOutcome(JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"] }] })).entries)
      .toEqual([makeEntry({ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"] })]);
  });

  it("carry 型條目的 meta 是物件就原樣通過", () => {
    const meta = { keys: ["hook"], constant: true, order: 3, disabled: false, visibility: { type: "gm" }, is_person: false };
    const json = JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"], meta }] });
    expect(parseRefactorOutcome(json).entries[0].meta).toEqual(meta);
  });

  it("條目沒有 meta（AI 重寫的條目）＝不帶這欄，不當成壞檔", () => {
    const json = JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"] }] });
    expect(parseRefactorOutcome(json).entries[0].meta).toBeUndefined();
  });

  it("舊版卡沒有 entries 仍照舊解析", () => {
    expect(parseRefactorOutcome(JSON.stringify({ characters: [makeCharacter(["12"])] })).entries).toEqual([]);
  });

  it("dropped/unabsorbed/audit 三欄逐欄解析，rule 是數字", () => {
    const json = JSON.stringify({
      entries: [{ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"] }],
      dropped: [{ uid: "5", span: "", title: "舊版本標記", content: "v1.2 更新", rule: 2 }],
      unabsorbed: [{ uid: "16", span: "16#s6", title: "戰鬥流程", note: "擲骰檢定" }],
      audit: [{ kind: "coverage", uid: "9", span: "", detail: "漏網自動補照搬" }],
    });
    const outcome = parseRefactorOutcome(json);
    expect(outcome.dropped).toEqual([{ uid: "5", span: "", title: "舊版本標記", content: "v1.2 更新", rule: 2 }]);
    expect(outcome.unabsorbed).toEqual([{ uid: "16", span: "16#s6", title: "戰鬥流程", note: "擲骰檢定" }]);
    expect(outcome.audit).toEqual([{ kind: "coverage", uid: "9", span: "", detail: "漏網自動補照搬" }]);
  });

  it("舊產物 JSON（包 4 之前存的重構卡）沒有 dropped/unabsorbed/audit 三欄，仍照舊可解", () => {
    const json = JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "不可違反", source_uids: ["3"] }] });
    const outcome = parseRefactorOutcome(json);
    expect(outcome.dropped).toEqual([]);
    expect(outcome.unabsorbed).toEqual([]);
    expect(outcome.audit).toEqual([]);
  });

  it.each([
    ["格式錯誤的 JSON", "{not json"],
    ["空殼：三區全空＝根本不是產物", "{}"],
    ["頂層不是物件", "[]"],
    ["角色不是物件", JSON.stringify({ characters: ["阿福"] })],
    ["角色缺名字", JSON.stringify({ characters: [{ source_uids: ["1"] }] })],
    ["角色沒有來源條目（舊版單數 source_uid 格式）", JSON.stringify({ characters: [{ name: "阿福", source_uid: "1" }] })],
    ["characters 不是陣列", JSON.stringify({ characters: { name: "阿福" } })],
    ["source_uids 混進非字串", JSON.stringify({ characters: [{ name: "阿福", source_uids: [1] }] })],
    ["介面缺 state_fields", JSON.stringify({ interface: { source_uids: ["3"], raw: "" } })],
    ["機制缺 source_uid", JSON.stringify({ mechanisms: [{ rules: {} }] })],
    ["新條目 kind 不合法", JSON.stringify({ entries: [{ title: "規矩", kind: "other", content: "內容", source_uids: ["3"] }] })],
    ["條目 meta 不是物件", JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "內容", source_uids: ["3"], meta: "壞掉" }] })],
    ["dropped 不是陣列", JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "內容", source_uids: ["3"] }], dropped: {} })],
    ["dropped 元素缺 rule 數字", JSON.stringify({ entries: [{ title: "規矩", kind: "mechanism", content: "內容", source_uids: ["3"] }], dropped: [{ uid: "5", title: "x", content: "y" }] })],
  ])("拒收：%s", (_label, json) => {
    expect(() => parseRefactorOutcome(json)).toThrow(REFACTOR_IMPORT_INVALID);
  });
});

describe("defaultRefactorSelection", () => {
  it("全勾：角色與機制 indices 是 0..N-1，有介面產物就 apply_interface=true，沒人疑似玩家就 player_index=null", () => {
    const outcome = makeOutcome({
      characters: [makeCharacter(["12"]), makeCharacter(["12"]), makeCharacter(["30"])],
      interface: { state_fields: {}, source_uids: ["8"], raw: "" },
      mechanisms: [makeMechanism("19"), makeMechanism("20")],
      entries: [makeEntry()],
    });
    expect(defaultRefactorSelection(outcome)).toEqual({
      character_indices: [0, 1, 2],
      apply_interface: true,
      mechanism_indices: [0, 1],
      entry_indices: [0],
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
    expect(refactorSummaryCounts(outcome)).toEqual({ characters: 1, hasInterface: true, mechanisms: 1, entries: 0 });
  });

  it("空產物三區皆零／false", () => {
    expect(refactorSummaryCounts(makeOutcome())).toEqual({ characters: 0, hasInterface: false, mechanisms: 0, entries: 0 });
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
  const base = { character_indices: [0], apply_interface: false, mechanism_indices: [], entry_indices: [], player_index: null };

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
    const selection = { character_indices: [0, 1], apply_interface: false, mechanism_indices: [], entry_indices: [], player_index: 0 };
    expect(unselectCharacter(selection, 1)).toEqual({ ...selection, character_indices: [0], player_index: 0 });
  });

  it("取消勾選的角色正是目前指定的玩家：一併清掉玩家指定", () => {
    const selection = { character_indices: [0, 1], apply_interface: false, mechanism_indices: [], entry_indices: [], player_index: 1 };
    expect(unselectCharacter(selection, 1)).toEqual({ ...selection, character_indices: [0], player_index: null });
  });
});

describe("restoreDropped", () => {
  const baseSelection: RefactorSelection = {
    character_indices: [],
    apply_interface: false,
    mechanism_indices: [],
    entry_indices: [0],
    player_index: null,
  };

  it("整條放回（span 空字串）：標題原樣，轉成 setting 條目附加到 entries 尾端並勾選", () => {
    const outcome = makeOutcome({
      entries: [makeEntry()],
      dropped: [{ uid: "5", span: "", title: "舊版本標記", content: "v1.2 更新", rule: 2 }],
    });
    const { outcome: next, selection } = restoreDropped(outcome, baseSelection, 0);
    expect(next.dropped).toEqual([]);
    expect(next.entries).toEqual([
      makeEntry(),
      { title: "舊版本標記", kind: "setting", content: "v1.2 更新", source_uids: ["5"], rules: {}, triggers: [] },
    ]);
    expect(selection.entry_indices).toEqual([0, 1]);
  });

  it("段放回（span 非空）：標題後綴 span 的 sN 段當區分標記", () => {
    const outcome = makeOutcome({
      entries: [],
      dropped: [{ uid: "16", span: "16#s6", title: "戰鬥流程", content: "擲骰檢定內容", rule: 3 }],
    });
    const { outcome: next, selection } = restoreDropped(outcome, { ...baseSelection, entry_indices: [] }, 0);
    expect(next.entries).toEqual([
      { title: "戰鬥流程 ⟦s6⟧", kind: "setting", content: "擲骰檢定內容", source_uids: ["16"], rules: {}, triggers: [] },
    ]);
    expect(selection.entry_indices).toEqual([0]);
  });

  it("回傳新的 outcome／selection 物件，不動原本傳入的那份（不可變更新）", () => {
    const outcome = makeOutcome({ dropped: [{ uid: "1", span: "", title: "t", content: "c", rule: 1 }] });
    const selection: RefactorSelection = { ...baseSelection, entry_indices: [] };
    const result = restoreDropped(outcome, selection, 0);
    expect(result.outcome).not.toBe(outcome);
    expect(result.selection).not.toBe(selection);
    expect(outcome.dropped).toHaveLength(1);
    expect(selection.entry_indices).toEqual([]);
  });
});
