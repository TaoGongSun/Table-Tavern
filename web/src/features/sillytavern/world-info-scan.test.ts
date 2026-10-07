// World Info 觸發（ST 06bde939 checkWorldInfo）：同一組逐字稿與世界書在不同條件下觸發哪些條目、放在哪；
// 遞迴、預算、計時（sticky／cooldown／delay 與回退）、機率、群組。
import { describe, expect, it } from "vitest";
import { worldInfoEntries } from "./world-info-book";
import { checkWorldInfo, parseRegexFromString, type WiResult, type WiTimed } from "./world-info-scan";

type V2 = { keys?: unknown; content: string; [field: string]: unknown };
const entry = ({ keys = [], ...rest }: V2) => ({ keys, enabled: true, insertion_order: 100, position: "before_char", ...rest });

/** 每條配 `e<序號>`。 */
const bookOf = (entries: V2[]) =>
  worldInfoEntries(
    { character_book: { entries: entries.map(entry) } },
    entries.map((_, index) => ({ id: `e${index}`, key: String(index) })),
  );

const NO_TIMED: WiTimed = { sticky: {}, cooldown: {} };

function scan(
  entries: V2[],
  chat: string[],
  options: { timed?: WiTimed; maxContext?: number; random?: () => number; trigger?: string } = {},
): WiResult {
  return checkWorldInfo(bookOf(entries), {
    chat: [...chat].reverse(),
    maxContext: options.maxContext ?? Number.POSITIVE_INFINITY,
    globalScan: { personaDescription: "", characterDescription: "守夜人瑟拉", characterPersonality: "", characterDepthPrompt: "", scenario: "", creatorNotes: "" },
    trigger: options.trigger ?? "normal",
    timed: options.timed ?? NO_TIMED,
    substitute: (text) => text.replace(/\{\{user\}\}/g, "旅人"),
    regex: (text) => text,
    // 一個字元算一個 token，方便算預算
    countTokens: (text) => text.length,
    random: options.random ?? (() => 0),
  });
}

/** 掃描的訊息（舊到新），照 ST 的「名字: 內容」。 */
const CHAT = ["瑟拉: 燈籠驛的爐火還亮著。", "旅人: 外面在下雪，有湯嗎？", "瑟拉: 先把門關上。", "旅人: 我推開門，雪灌了進來。"];

describe("which entries fire, and where they go (same transcript, same book)", () => {
  it.each<[string, V2[], Partial<Record<keyof WiResult, unknown>>]>([
    ["constant, before/after", [{ content: "常駐前", constant: true }, { content: "常駐後", constant: true, position: "after_char" }], { before: "常駐前", after: "常駐後" }],
    // 掃描深度 2：只看最後兩則（「門」「雪」在，「爐火」「湯」在更早的訊息）
    ["scan depth 2", [{ keys: ["雪"], content: "雪" }, { keys: ["爐火"], content: "爐火" }], { before: "雪" }],
    ["entry scan depth overrides", [{ keys: ["爐火"], content: "爐火", extensions: { scan_depth: 4 } }], { before: "爐火" }],
    // 全字比對：中文前後都是非 \w 字元，照樣算一個字；英文黏在單字裡不算
    ["whole words with CJK", [{ keys: ["推開"], content: "推開" }], { before: "推開" }],
    ["regex key", [{ keys: ["/推.門/"], content: "regex" }], { before: "regex" }],
    ["case-insensitive", [{ keys: ["SNOW"], content: "snow" }], { before: "" }],
    ["disabled entry", [{ keys: ["雪"], content: "雪", enabled: false }], { before: "" }],
    ["no keys", [{ keys: [], content: "空" }], { before: "" }],
    // 次要鍵四種邏輯（selective 開）：AND_ANY／NOT_ALL／NOT_ANY／AND_ALL
    ["AND_ANY", [{ keys: ["門"], secondary_keys: ["山賊", "雪"], selective: true, content: "任一", extensions: { selectiveLogic: 0 } }], { before: "任一" }],
    ["NOT_ALL", [{ keys: ["門"], secondary_keys: ["山賊", "雪"], selective: true, content: "不全", extensions: { selectiveLogic: 1 } }], { before: "不全" }],
    ["NOT_ANY", [{ keys: ["門"], secondary_keys: ["山賊", "雪"], selective: true, content: "都不", extensions: { selectiveLogic: 2 } }], { before: "" }],
    ["AND_ALL", [{ keys: ["門"], secondary_keys: ["推開", "雪"], selective: true, content: "全部", extensions: { selectiveLogic: 3 } }], { before: "全部" }],
    // selective 沒開就不看次要鍵
    ["secondary ignored when not selective", [{ keys: ["門"], secondary_keys: ["山賊"], content: "主鍵就夠", extensions: { selectiveLogic: 3 } }], { before: "主鍵就夠" }],
    ["match character description", [{ keys: ["守夜人"], content: "描述", extensions: { match_character_description: true } }], { before: "描述" }],
    ["generation-type trigger filter", [{ keys: ["雪"], content: "只在重生", extensions: { triggers: ["regenerate"] } }], { before: "" }],
    [
      "positions: depth, AN top/bottom, examples, outlet",
      [
        { keys: ["雪"], content: "深度", extensions: { position: 4, depth: 1, role: 1 } },
        { keys: ["雪"], content: "註記上", extensions: { position: 2 } },
        { keys: ["雪"], content: "註記下", extensions: { position: 3 } },
        { keys: ["雪"], content: "範例上", extensions: { position: 5 } },
        { keys: ["雪"], content: "範例下", extensions: { position: 6 } },
        { keys: ["雪"], content: "門外", extensions: { position: 7, outlet_name: "door" } },
      ],
      {
        depth: [{ depth: 1, role: 1, entries: ["深度"] }],
        anTop: ["註記上"],
        anBottom: ["註記下"],
        examples: [
          { position: "after", content: "範例下" },
          { position: "before", content: "範例上" },
        ],
        outlets: { door: ["門外"] },
      },
    ],
    // 插入順序：order 大的離對話近（同位置裡排在後面）；@@ 裝飾
    [
      "order and decorators",
      [
        { keys: ["雪"], content: "order 1", insertion_order: 1 },
        { keys: ["雪"], content: "order 50", insertion_order: 50 },
        { content: "@@activate\n強制", insertion_order: 10 },
        { keys: ["雪"], content: "@@dont_activate\n不准", insertion_order: 20 },
      ],
      { before: "order 1\n強制\norder 50" },
    ],
  ])("%s", (_name, entries, expected) => {
    expect(scan(entries, CHAT)).toMatchObject(expected);
  });

  it("same order follows ST's load order: entry ids as object keys (integers ascending, duplicates overwrite in place)", () => {
    const sameOrder = (ids: unknown[]) =>
      worldInfoEntries(
        { character_book: { entries: ids.map((id, index) => entry({ keys: ["雪"], content: `#${index}`, insertion_order: 5, ...(id === undefined ? {} : { id }) })) } },
        ids.map((_, index) => ({ id: `e${index}`, key: String(index) })),
      ).map((item) => item.id);
    // id 20、10：ST 先 10 再 20（卡內陣列順序是 20 在前）
    expect(sameOrder([20, 10])).toEqual(["e1", "e0"]);
    // 字串 id 排在整數之後、照出現順序；沒 id 的用陣列索引
    expect(sameOrder(["b", undefined, "a", 0])).toEqual(["e3", "e1", "e0", "e2"]);
    // 同 id：後面那條蓋掉前面、位置不變
    expect(sameOrder([7, 3, 7])).toEqual(["e1", "e2"]);
    // 掃描與插入跟著這個順序
    const book: V2[] = [
      { id: 20, keys: ["雪"], content: "A", insertion_order: 5 },
      { id: 10, keys: ["雪"], content: "B", insertion_order: 5 },
    ];
    expect(scan(book, CHAT).before).toBe("A\nB");
    expect(scan(book, CHAT).activated).toEqual(["e1", "e0"]);
  });

  it("a key that is not a valid /regex/ is matched as plain text", () => {
    // 「/[/」不是合法正則：當字面字串找（全字比對時前後要是非 \w 字元）
    expect(scan([{ keys: ["/[/"], content: "字面" }], ["旅人: 在牆上寫 /[/ 記號"]).activated).toEqual(["e0"]);
    expect(scan([{ keys: ["/[/"], content: "字面" }], ["旅人: 沒有記號"]).activated).toEqual([]);
  });

  it("parses /pattern/flags keys like ST (unescaped slash is not a regex)", () => {
    expect(parseRegexFromString("/a.c/i")?.flags).toBe("i");
    expect(parseRegexFromString("/a/b/")).toBeNull();
    expect(parseRegexFromString("/a\\/b/")?.source).toBe("a\\/b");
    expect(parseRegexFromString("plain")).toBeNull();
  });
});

describe("recursion", () => {
  const book: V2[] = [
    { keys: ["雪"], content: "雪會引來狼。", insertion_order: 50 },
    { keys: ["狼"], content: "狼群怕火。", insertion_order: 40 },
    { keys: ["火"], content: "火是驛站的命。", insertion_order: 30 },
    { keys: ["狼"], content: "排除遞迴：不該出現", insertion_order: 20, extensions: { exclude_recursion: true } },
    { keys: ["雪"], content: "只在遞迴才觸發：雪", insertion_order: 10, extensions: { delay_until_recursion: true } },
  ];

  it("activates entries mentioned by other activated entries, level by level", () => {
    const result = scan(book, CHAT);
    expect(result.activated.sort()).toEqual(["e0", "e1", "e2", "e4"]);
    expect(result.before).toBe("只在遞迴才觸發：雪\n火是驛站的命。\n狼群怕火。\n雪會引來狼。");
  });

  it("an entry already activated or that failed its roll is not reconsidered in later recursion steps", () => {
    const rolls: number[] = [];
    const random = () => {
      rolls.push(rolls.length);
      return 0.9;
    };
    const book: V2[] = [
      { keys: ["雪"], content: "雪裡有狼，雪很深。", insertion_order: 50 },
      { keys: ["雪", "狼"], content: "半數", insertion_order: 40, extensions: { probability: 50 } },
    ];
    const result = scan(book, CHAT, { random });
    // 第 0 條內容又提到「雪」，遞迴時不重複觸發；第 1 條第一輪擲輸，遞迴時命中「狼」也不再擲
    expect(result.activated).toEqual(["e0"]);
    expect(result.before).toBe("雪裡有狼，雪很深。");
    expect(rolls).toHaveLength(1);
  });

  it("prevent_recursion keeps an entry's content out of the recursion buffer", () => {
    const blocked = book.map((item, index) => (index === 0 ? { ...item, extensions: { prevent_recursion: true } } : item));
    // 「狼」只靠第 0 條的內容才會命中；擋掉之後沒有遞迴，只在遞迴才觸發的那條也不會來
    expect(scan(blocked, CHAT).activated).toEqual(["e0"]);
  });
});

describe("budget (25% of the prompt budget, counted by the model's estimator)", () => {
  const book: V2[] = [
    { keys: ["雪"], content: "一二三四五六七八九", insertion_order: 30 },
    { keys: ["雪"], content: "甲乙丙丁戊", insertion_order: 20 },
    { keys: ["雪"], content: "預算外", insertion_order: 10, extensions: { ignore_budget: true } },
    { keys: ["雪"], content: "ABC", insertion_order: 5 },
  ];

  it.each([
    // 預算＝round(25% × maxContext)；累計內容（每條後面接換行，ignore_budget 的也算進去）達到預算的那條與之後的
    // 都不放，ignore_budget 照放。累計：10、16、20、24
    [Number.POSITIVE_INFINITY, ["e0", "e1", "e2", "e3"]],
    [100, ["e0", "e1", "e2", "e3"]],
    [96, ["e0", "e1", "e2"]],
    [64, ["e0", "e2"]],
    [40, ["e2"]],
  ])("maxContext %s", (maxContext, expected) => {
    expect(scan(book, CHAT, { maxContext }).activated).toEqual(expected);
  });

  it("an overflow stops recursion", () => {
    const recursive: V2[] = [
      { keys: ["雪"], content: "狼來了喔喔喔", insertion_order: 30 },
      { keys: ["雪"], content: "超過預算", insertion_order: 20 },
      { keys: ["狼"], content: "遞迴才到", insertion_order: 10 },
    ];
    // 預算 10：第二條累計 12 爆掉，遞迴不再往下
    expect(scan(recursive, CHAT, { maxContext: 40 }).activated).toEqual(["e0"]);
    expect(scan(recursive, CHAT).activated).toEqual(["e0", "e1", "e2"]);
  });
});

describe("timed effects (counted in messages, ST timedWorldInfo)", () => {
  const sticky: V2[] = [{ keys: ["雪"], content: "黏著", extensions: { sticky: 3, cooldown: 2 } }];
  const quiet = (length: number) => Array.from({ length }, (_, index) => `x: 第${index}則`);
  const withSnow = (length: number) => [...quiet(length - 1), "旅人: 雪"];
  const sticky3: V2[] = [{ keys: ["雪"], content: "黏著", extensions: { sticky: 3 } }];

  it("sticky, expiry into a protected cooldown, and rollback when the chat goes back", () => {
    const first = scan(sticky, withSnow(2));
    expect(first.activated).toEqual(["e0"]);
    expect(first.timed).toEqual({ sticky: { e0: { start: 2, end: 5, protected: false } }, cooldown: { e0: { start: 2, end: 4, protected: false } } });
    // 再往前兩則、沒有關鍵字：sticky 照樣觸發；舊冷卻到期，這次觸發又記一筆冷卻（ST 觸發時兩種計時都記）
    const held = scan(sticky, quiet(4), { timed: first.timed });
    expect(held.activated).toEqual(["e0"]);
    expect(held.timed).toEqual({ sticky: { e0: first.timed.sticky.e0 }, cooldown: { e0: { start: 4, end: 6, protected: false } } });
    // 刪除／重新生成／送出失敗讓對話退回 2 則（≤ start）：未受保護的計時撤掉，跟沒觸發過一樣
    expect(scan(sticky, quiet(2), { timed: first.timed })).toMatchObject({ activated: [], timed: { sticky: {}, cooldown: {} } });
    // 到 5 則 sticky 到期：接著冷卻（受保護），這時關鍵字命中也不觸發
    const cooled = scan(sticky, withSnow(5), { timed: held.timed });
    expect(cooled).toMatchObject({ activated: [], timed: { sticky: {}, cooldown: { e0: { start: 5, end: 7, protected: true } } } });
    // 受保護的冷卻：對話沒往前也不撤
    expect(scan(sticky, withSnow(5), { timed: cooled.timed }).timed.cooldown.e0).toEqual({ start: 5, end: 7, protected: true });
    // 冷卻到期：關鍵字再命中就重新黏
    expect(scan(sticky, withSnow(7), { timed: cooled.timed }).activated).toEqual(["e0"]);
  });

  // 編輯／刪除／重新生成照 ST：計時只看對話則數。第 4 則觸發（start 4）之後下一次組提示時的則數：
  it.each([
    ["重新生成（換掉回覆，則數回到觸發時）", 4, {}],
    ["刪掉玩家句與回覆", 3, {}],
    ["編輯最後一則（則數不變）後送下一句", 6, { e0: { start: 4, end: 7, protected: false } }],
    ["送出失敗收回（則數回到觸發前）", 3, {}],
  ])("%s", (_name, length, sticky) => {
    const triggered = scan(sticky3, withSnow(4)).timed;
    expect(scan(sticky3, quiet(length), { timed: triggered }).timed.sticky).toEqual(sticky);
  });

  it("delay holds an entry until the chat has that many messages", () => {
    const delayed: V2[] = [{ keys: ["雪"], content: "晚到", extensions: { delay: 3 } }];
    expect(scan(delayed, withSnow(2)).activated).toEqual([]);
    expect(scan(delayed, withSnow(3)).activated).toEqual(["e0"]);
  });

  it("drops effects for entries that lost their timer", () => {
    const plain: V2[] = [{ keys: ["雪"], content: "沒有計時" }];
    expect(scan(plain, quiet(4), { timed: { sticky: { e0: { start: 1, end: 9, protected: false } }, cooldown: {} } }).timed.sticky).toEqual({});
  });
});

describe("probability and inclusion groups", () => {
  it("rolls once per entry; a sticky entry does not re-roll", () => {
    const book: V2[] = [{ keys: ["雪"], content: "半數", extensions: { probability: 50, useProbability: true, sticky: 2 } }];
    expect(scan(book, CHAT, { random: () => 0.6 }).activated).toEqual([]);
    const passed = scan(book, CHAT, { random: () => 0.4 });
    expect(passed.activated).toEqual(["e0"]);
    expect(scan(book, [...CHAT, "x: 下一則"], { random: () => 0.99, timed: passed.timed }).activated).toEqual(["e0"]);
  });

  it("one winner per group: override by order, otherwise weighted roll", () => {
    const group = (extra: object): V2[] => [
      { keys: ["雪"], content: "A", insertion_order: 10, extensions: { group: "天氣", group_weight: 30, ...extra } },
      { keys: ["雪"], content: "B", insertion_order: 20, extensions: { group: "天氣", group_weight: 70 } },
    ];
    // 群組裡照排好的順序（order 大的先）累加權重：B 70、A 30
    expect(scan(group({}), CHAT, { random: () => 0.5 }).activated).toEqual(["e1"]);
    expect(scan(group({}), CHAT, { random: () => 0.8 }).activated).toEqual(["e0"]);
    expect(scan(group({ group_override: true }), CHAT, { random: () => 0.9 }).activated).toEqual(["e0"]);
  });

  it("equal weights: the roll walks the group in order", () => {
    const group: V2[] = [
      { keys: ["雪"], content: "A", insertion_order: 10, extensions: { group: "g" } },
      { keys: ["雪"], content: "B", insertion_order: 10, extensions: { group: "g" } },
    ];
    expect(scan(group, CHAT, { random: () => 0.5 }).activated).toEqual(["e0"]);
    expect(scan(group, CHAT, { random: () => 0.51 }).activated).toEqual(["e1"]);
  });

  it("group scoring keeps only the members matching the most keys", () => {
    const group: V2[] = [
      { keys: ["雪", "門"], content: "兩個鍵", insertion_order: 10, extensions: { group: "g", use_group_scoring: true } },
      { keys: ["雪"], content: "一個鍵", insertion_order: 20, extensions: { group: "g", use_group_scoring: true } },
    ];
    expect(scan(group, CHAT, { random: () => 0.99 }).activated).toEqual(["e0"]);
  });

  it("an entry in two groups removed twice does not take another entry with it (D31)", () => {
    const book: V2[] = [
      { keys: ["雪"], content: "A", insertion_order: 30, extensions: { group: "g1, g2" } },
      { keys: ["雪"], content: "B", insertion_order: 20, extensions: { group: "g1", group_override: true } },
      { keys: ["雪"], content: "C", insertion_order: 10, extensions: { group: "g2" } },
    ];
    // g1 由 B 勝出、拿掉 A；g2 擲到 C，再拿 A 時已經不在就不刪（ST 的 splice(-1) 會誤刪 C）
    expect(scan(book, CHAT, { random: () => 0.9 }).activated).toEqual(["e1", "e2"]);
  });

  it("a sticky member keeps winning its group", () => {
    const book: V2[] = [
      { keys: ["雪"], content: "A", extensions: { group: "g", sticky: 5 } },
      { keys: ["雪"], content: "B", extensions: { group: "g" } },
    ];
    const timed: WiTimed = { sticky: { e0: { start: 1, end: 9, protected: false } }, cooldown: {} };
    expect(scan(book, CHAT, { timed, random: () => 0.99 }).activated).toEqual(["e0"]);
  });
});
