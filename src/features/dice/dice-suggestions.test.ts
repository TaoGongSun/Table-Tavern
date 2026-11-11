import { expect, it } from "vitest";
import { rankSuggestions, rolledFormula } from "./dice-suggestions";

const suggestions = (raw: string, name = "") => rankSuggestions(raw, name).map((x) => x.value);

it("player formulas obey bounds and never salvage partial expressions", () => {
  expect(suggestions("沃恩請擲d20、2D6 + 3，或1d20-2；再擲2d6+3。")).toEqual(["1d20", "2d6+3", "1d20-2"]);
  for (const bad of ["0d6", "101d6", "1d1", "1d1001", "1d6+10000", "1d6-10000", "2d20kh1", "1d6+1d4", "1d6 + 1d4", "abc1d20", "1d6!", "999999999999999d6"])
    expect(suggestions(bad), bad).toEqual([]);
  expect(suggestions("請擲 1d20＋3，或 1d6－1")).toEqual(["1d20+3", "1d6-1"]);
  expect(suggestions("(d4#1=2)（d20 # 2=19）＋2。")).toEqual([]);
  expect(suggestions("**命中骰：1d20+7**，**2d6**")).toEqual(["1d20+7", "2d6"]);
  expect(suggestions("1d6**2 與 2**1d6")).toEqual([]);
  expect(suggestions("1d2 1d3 1d4 1d5 1d6 1d7 1d8 1d9 1d10")).toHaveLength(8);
  expect(suggestions(`${"沃".repeat(32768)}1d20`)).toEqual([]);
});

it("puts the formula from the player's own clause first", () => {
  const text = "雷克請擲1d20+1，沃恩請擲1d20+3，傷害2d6。";
  expect(suggestions(text, "沃恩")).toEqual(["1d20+3", "1d20+1", "2d6"]);
  expect(suggestions(text)).toEqual(["1d20+1", "1d20+3", "2d6"]);
  expect(suggestions(text, "　")).toEqual(["1d20+1", "1d20+3", "2d6"]);
});

it("marks formulas meant for another character", () => {
  const rank = (text: string) => rankSuggestions(text, "沃恩", ["雷克", "沃恩"]);
  expect(rank("雷克請擲1d20+1，沃恩請擲1d20+3，傷害2d6。")).toEqual([
    { value: "1d20+3", other: false },
    { value: "2d6", other: false },
    { value: "1d20+1", other: true },
  ]);
  expect(rank("雷克，請擲1d20+1。")).toEqual([{ value: "1d20+1", other: true }]);
  expect(rank("沃恩，請擲1d20+3。")).toEqual([{ value: "1d20+3", other: false }]);
  expect(rank("雷克擲1d6。沃恩也擲1d6。")).toEqual([{ value: "1d6", other: false }]);
});

it("GM 回頭提到剛擲的算式時，同級裡排到後面", () => {
  const gm = "你擲出 5 與 1：2d6 不是命中骰。請擲命中 `1d20+5`，命中後擲傷害 `1d8+3`。";
  expect(rankSuggestions(gm, "", [], ["2d6"]).map((x) => x.value)).toEqual(["1d20+5", "1d8+3", "2d6"]);
  expect(rankSuggestions("再攻擊一次 1d20+5", "", [], ["1d20+5"]).map((x) => x.value)).toEqual(["1d20+5"]);
});

it("擲骰紀錄讀得出算式", () => {
  expect(rolledFormula("格魯 2d6: [5, 1] = 6")).toBe("2d6");
  expect(rolledFormula("1d20+5: [7] +5 = 12")).toBe("1d20+5");
  expect(rolledFormula("阿 虎 1d8-1: [1] -1 = 0")).toBe("1d8-1");
  expect(rolledFormula("隨口說 2d6")).toBeUndefined();
});
