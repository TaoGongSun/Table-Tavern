import { describe, expect, it } from "vitest";
import { inspectPool } from "./dice-pool";
import copy from "./dice-copy.json";

const pool = "d6:[4,2]\nd20:[19,3]";
describe("骰池對帳", () => {
  it("接受無文字、全半形括號與空白；玩家數字不誤算池引用", () => {
    const result = inspectPool(
      pool,
      "沃恩 19；（ d20 # 1 = 19 ） (d20#2=3) ( d6 # 1 = 4 )",
    )!;
    expect(result.references).toBe(3);
    expect(
      result.rows[1].dice.every((d) => d.used && d.issues.length === 0),
    ).toBe(true);
    expect(result.rows[0].dice[1].used).toBe(false);
  });
  it("錯面、跳號、重號及不存在的格都有提示，保留原面", () => {
    const result = inspectPool(pool, "(d20#2=20) (d20#2=3) (d8#1=4)")!;
    expect(result.rows[1].dice[1]).toEqual({
      value: 3,
      used: true,
      issues: ["face", "repeat", "order"],
      reported: [20],
    });
    expect(result.unknown).toEqual(["d8#1=4"]);
  });
  it("照結果排序寫不算跳號，一次跳號不連累後面", () => {
    const d20 = "d20:[5,3,18,5,9,20]";
    const sorted = inspectPool(d20, "(d20#3=18) (d20#4=5) (d20#1=5) (d20#2=3) (d20#5=9) (d20#6=20)")!;
    expect(sorted.rows[0].dice.every((d) => d.used && d.issues.length === 0)).toBe(true);
    const skipped = inspectPool(d20, "(d20#1=5) (d20#3=18) (d20#4=5)")!;
    expect(skipped.rows[0].dice.map((d) => d.issues)).toEqual([[], [], ["order"], ["order"], [], []]);
  });
  it("不加括號或合寫一個括號也認得", () => {
    const bare = inspectPool(pool, "命中 d20#1=19，傷害 d6#1=4＋2")!;
    expect(bare.references).toBe(2);
    expect(bare.rows[1].dice[0].used).toBe(true);
    const merged = inspectPool(pool, "(d6#1=4，d6#2=2)")!;
    expect(merged.rows[0].dice.every((d) => d.used)).toBe(true);
    expect(inspectPool(pool, "add20#1=19")!.references).toBe(0);
  });
  it("未引用仍回傳池；再次解析不沿用已用狀態", () => {
    expect(inspectPool(pool, "(d20#1=19)")!.rows[1].dice[0].used).toBe(true);
    const result = inspectPool(pool, "沃恩停下腳步")!;
    expect(result.references).toBe(0);
    expect(result.rows.flatMap((r) => r.dice).every((d) => !d.used)).toBe(true);
  });
  it.each([
    "d20:[0]",
    "d20:[21]",
    "d20:[1]\nd20:[2]",
    "<script>",
    "d3:[1]",
    "x".repeat(513),
  ])("壞池只回無法讀取：%s", (s) => {
    expect(inspectPool(s, "")).toBeNull();
  });
  it("上限 64 顆：剛好讀得出，多一顆回無法讀取", () => {
    const dice = (n: number) => `d6:[${Array(n).fill(1).join(",")}]`;
    expect(inspectPool(dice(64), "")!.rows[0].dice).toHaveLength(64);
    expect(inspectPool(dice(65), "")).toBeNull();
  });
  it("十語系字串齊全", () => {
    expect(Object.keys(copy).length).toBe(10);
    for (const labels of Object.values(copy)) {
      expect(Object.keys(labels)).toEqual(Object.keys(copy.en));
      expect(Object.values(labels).every((s) => s.length > 0)).toBe(true);
    }
  });
});
