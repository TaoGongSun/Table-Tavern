// 集合區段：先綁路徑、再由既有填值器一次替換；北境驿站骨架改成區段後，填出來與舊式字串清單逐位元相同。

import { describe, expect, it } from "vitest";
import yaml from "js-yaml";
import { fillSkeletonPlaceholders, type StateNode } from "./refactor-shell";
import { orderedItemKeys, splitSections, type Collections } from "./refactor-shell-sections";

const TAGS = ["Status_block"];

// 北境驿站重構產物（舊式）：清單欄整串字串、整行佔位符
const OLD_SHELL = `<Status_block>
状态栏:
  地点: "📍 {{地點}}"
  人物列表:
{{在場人物}}
  行动选项:
    名字: "👤 林远"
    选项:
{{行動選項}}
</Status_block>`;

const OLD_TREE: Record<string, StateNode> = {
  地點: "北境驿站 前院",
  在場人物: [
    "    - 人物:",
    '        名字: "👤 林远"',
    '        身份: "🪪 新任驿丞"',
    '        体力: "💪 55 赶了三天路，疲惫"',
    "    - 人物:",
    '        名字: "👤 贺铮"',
    '        身份: "🪪 老驿长"',
    '        体力: "💪 70 腿伤畏寒"',
  ].join("\n"),
  行動選項: [
    '      - "1. 接过热茶，先去驿丞房安顿"',
    '      - "2. 当场核对马匹与驿马簿"',
  ].join("\n"),
};

// 同一張卡改成集合：項目格式（外殼、鍵名、縮排、引號、emoji、序號）全在骨架
const SHELL = `<Status_block>
状态栏:
  地点: "📍 {{地點}}"
  人物列表:
{{#在場人物}}
    - 人物:
        名字: "👤 {{@key}}"
        身份: "🪪 {{身份}}"
        体力: "💪 {{体力}}"
{{/在場人物}}
  行动选项:
    名字: "👤 林远"
    选项:
{{#行動選項}}
      - "{{@n}}. {{.}}"
{{/行動選項}}
</Status_block>`;

const COLLECTIONS: Collections = {
  在場人物: { keyed: true, item: "object", fields: { 身份: "text", 体力: "text" } },
  行動選項: { keyed: false, item: "scalar", type: "text" },
};

function tree(people: Record<string, StateNode>, options: string[]): Record<string, StateNode> {
  return {
    地點: "北境驿站 前院",
    在場人物: people,
    行動選項: Object.fromEntries(options.map((option, index) => [String(index), option])),
  };
}

const PEOPLE: Record<string, StateNode> = {
  贺铮: { _序: "1", 身份: "老驿长", 体力: "70 腿伤畏寒" },
  林远: { _序: "0", 身份: "新任驿丞", 体力: "55 赶了三天路，疲惫" },
};

describe("集合區段", () => {
  it("北境驿站：區段填出來與舊式字串清單逐位元相同（_序 決定順序，不照字碼）", () => {
    const expected = fillSkeletonPlaceholders(OLD_SHELL, OLD_TREE, TAGS);
    const actual = fillSkeletonPlaceholders(
      SHELL,
      tree(PEOPLE, ["接过热茶，先去驿丞房安顿", "当场核对马匹与驿马簿"]),
      TAGS,
      {},
      COLLECTIONS,
    );
    expect(actual).toBe(expected);
  });

  it("值含 {{…}}、引號、換行不被二次解讀，YAML 讀回原值", () => {
    const people = { 林远: { _序: "0", 身份: '{{地點}} 與 "引號"', 体力: "第一行\n第二行" } };
    const out = fillSkeletonPlaceholders(SHELL, tree(people, ["{{@n}} 不是序號"]), TAGS, {}, COLLECTIONS);
    const body = out.replace(/^<Status_block>\n|<\/Status_block>$/g, "");
    const parsed = yaml.load(body) as { 状态栏: { 人物列表: { 人物: Record<string, string> }[]; 行动选项: { 选项: string[] } } };
    expect(parsed.状态栏.人物列表[0].人物).toEqual({
      名字: "👤 林远",
      身份: '🪪 {{地點}} 與 "引號"',
      体力: "💪 第一行\n第二行",
    });
    expect(parsed.状态栏.行动选项.选项).toEqual(["1. {{@n}} 不是序號"]);
  });

  it("序列超過 10 項照索引數值排，序號從 1 起", () => {
    const options = Array.from({ length: 12 }, (_, index) => `選項${index}`);
    const out = fillSkeletonPlaceholders(SHELL, tree(PEOPLE, options), TAGS, {}, COLLECTIONS);
    const lines = out.split("\n").filter((line) => line.startsWith("      - "));
    expect(lines).toEqual(options.map((option, index) => `      - "${index + 1}. ${option}"`));
  });

  it("空集合：區段前一行是值為空的鍵就補 []；樹上整支不見也一樣", () => {
    const out = fillSkeletonPlaceholders(SHELL, tree({}, []), TAGS, {}, COLLECTIONS);
    expect(out).toContain("  人物列表: []\n  行动选项:");
    expect(out).toContain("    选项: []\n</Status_block>");
    const missing = fillSkeletonPlaceholders(SHELL, { 地點: "前院" }, TAGS, {}, COLLECTIONS);
    expect(missing).toContain("  人物列表: []\n");
  });

  it("非 YAML 區的行內區段照項目重複，空集合不補 []", () => {
    const shell = "<道具>{{#道具}}<i>{{.}}</i>{{/道具}}</道具>";
    const collections: Collections = { 道具: { keyed: false, item: "scalar", type: "text" } };
    expect(fillSkeletonPlaceholders(shell, { 道具: { "0": "火把", "1": "繩子" } }, [], {}, collections)).toBe(
      "<道具><i>火把</i><i>繩子</i></道具>",
    );
    expect(fillSkeletonPlaceholders(shell, { 道具: {} }, [], {}, collections)).toBe("<道具></道具>");
  });

  it("數字欄位照描述的型別裸放，文字欄位會被轉型就加引號", () => {
    const shell = "<Status_block>\n隊伍:\n{{#隊伍}}\n  - 名: {{@key}}\n    等級: {{等級}}\n    稱號: {{稱號}}\n{{/隊伍}}\n</Status_block>";
    const collections: Collections = { 隊伍: { keyed: true, item: "object", fields: { 等級: "number", 稱號: "text" } } };
    const out = fillSkeletonPlaceholders(shell, { 隊伍: { 亞瑟: { _序: "0", 等級: "7", 稱號: "12" } } }, TAGS, {}, collections);
    expect(out).toContain("  - 名: 亞瑟\n    等級: 7\n    稱號: \"12\"\n");
  });

  it("沒有集合描述：不展開，標記當查不到的路徑填空", () => {
    const out = fillSkeletonPlaceholders("<道具>{{#道具}}x{{/道具}}</道具>", { 道具: { "0": "a" } }, [], {}, {});
    expect(out).toBe("<道具>x</道具>");
  });
});

describe("orderedItemKeys", () => {
  it("具名看 _序、序列看索引數值、沒名次的接最後依鍵排，底線鍵不是項目", () => {
    expect(orderedItemKeys({ 乙: { _序: "5" }, 甲: { _序: "2" }, 丙: { x: "1" }, _x: "1" })).toEqual(["甲", "乙", "丙"]);
    expect(orderedItemKeys({ "10": "a", "2": "b", "0": "c" })).toEqual(["0", "2", "10"]);
  });
});

describe("splitSections", () => {
  it("取出集合路徑與區段外文字；不成對、巢狀回 null；酒館註解不算收尾", () => {
    expect(splitSections("a{{#甲}}x{{y}}{{/甲}}b{{// 註}}{{#乙}}{{/乙}}")).toEqual({ paths: ["甲", "乙"], outside: "ab{{// 註}}" });
    expect(splitSections("{{#甲}}{{#乙}}{{/乙}}{{/甲}}")).toBeNull();
    expect(splitSections("{{#甲}}x")).toBeNull();
    expect(splitSections("x{{/甲}}")).toBeNull();
  });
});
