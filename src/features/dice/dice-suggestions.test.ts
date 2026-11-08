import { expect, it } from "vitest";
import { rankSuggestions, suggestions } from "./dice-suggestions";

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
