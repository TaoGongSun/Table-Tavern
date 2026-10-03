// parseMessage 的值解析引擎（計畫 8.9）：值解析六段、受限數學式、JSONPatch 區塊文字解析、值上限。
import { describe, expect, it } from "vitest";
import { MAX_EXPRESSION_CHARS, parseCommandValue, parsePatchBlock, runEval, valueLimitProblem } from "./card-mvu-parse-engine";

describe("值解析六段", () => {
  it("字面量與 JSON", () => {
    expect(parseCommandValue(" true ")).toBe(true);
    expect(parseCommandValue("false")).toBe(false);
    expect(parseCommandValue("null")).toBeNull();
    expect(parseCommandValue("undefined")).toBeUndefined();
    expect(parseCommandValue("30")).toBe(30);
    expect(parseCommandValue('"a b"')).toBe("a b");
    expect(parseCommandValue('{"a":[1,2]}')).toEqual({ a: [1, 2] });
  });

  it("JSON5 只收物件與陣列：未加引號的鍵、單引號、尾逗號、註解", () => {
    expect(parseCommandValue("{a: 1, 'b': 'x', }")).toEqual({ a: 1, b: "x" });
    expect(parseCommandValue("[1, 2, /* c */ 3,]")).toEqual([1, 2, 3]);
  });

  it("單引號文字走 YAML：雙單引號是跳脫", () => {
    expect(parseCommandValue("'it''s'")).toBe("it's");
    expect(parseCommandValue("'a b'")).toBe("a b");
  });

  it("受限數學式：四則、函式、Math.／math. 前綴、浮點誤差修正", () => {
    expect(parseCommandValue("10 + 2")).toBe(12);
    expect(parseCommandValue("1+2*3")).toBe(7);
    expect(parseCommandValue("sqrt(16)")).toBe(4);
    expect(parseCommandValue("Math.floor(2.7)")).toBe(2);
    expect(parseCommandValue("Math.PI")).toBeCloseTo(3.14159265359, 10);
    expect(parseCommandValue("math.pow(2, 3)")).toBe(8);
    expect(parseCommandValue("0.1 + 0.2")).toBe(0.3);
    expect(parseCommandValue("1/0")).toBe(Infinity);
  });

  it("數學式的複數與矩陣轉字串；布林與單位結果不當數學（落到 YAML／字串）", () => {
    expect(parseCommandValue("sqrt(-4)")).toBe("2i");
    expect(typeof parseCommandValue("matrix([1, 2])")).toBe("string");
    expect(parseCommandValue("1 < 2")).toBe("1 < 2");
    expect(parseCommandValue("5 cm")).toBe("5 cm");
  });

  it("單一單字不是算式：回字串；其餘文字走 YAML；最後去頭尾引號與空白", () => {
    expect(parseCommandValue("hello_world")).toBe("hello_world");
    expect(parseCommandValue("hello world")).toBe("hello world");
    expect(parseCommandValue("a: b")).toEqual({ a: "b" });
    expect(parseCommandValue("yes")).toBe("yes");
    // 同上游 yaml 套件預設：顯式標籤的時間戳解成 Date（沙盒再轉 ISO）；命令值不開合併鍵
    expect(parseCommandValue("!!timestamp 2024-01-01")).toEqual(new Date("2024-01-01T00:00:00Z"));
    // 數學式排在 YAML 前面（上游順序）：純日期被當減法，帶時間的才落到字串
    expect(parseCommandValue("2024-01-01")).toBe(2022);
    expect(parseCommandValue("2024-01-01T10:00:00Z")).toBe("2024-01-01T10:00:00Z");
    expect(parseCommandValue("")).toBe("");
    expect(parseCommandValue("  `x y` ")).toBe("x y");
    expect(parseCommandValue("{ 壞掉")).toBe("{ 壞掉");
  });

  it("賦值、改宿主物件的算式被擋在隔離實例或失敗：不影響 Math", () => {
    const random = Math.random;
    expect(parseCommandValue("Math.random = 0")).toBe("Math.random = 0");
    expect(Math.random).toBe(random);
    expect(parseCommandValue("a = 5")).toBe(5);
    expect(parseCommandValue("f(x) = x^2")).toBe("f(x) = x^2");
  });

  it("超過 1000 字的算式不求值", () => {
    const long = Array.from({ length: 600 }, () => "1").join("+");
    expect(long.length).toBeGreaterThan(MAX_EXPRESSION_CHARS);
    expect(parseCommandValue(long)).toBe(long);
  });
});

describe("JSONPatch 區塊解析", () => {
  it("YAML、JSON5、殘缺 JSON 修復", () => {
    expect(parsePatchBlock("- op: replace\n  path: /a\n  value: 1")).toEqual([{ op: "replace", path: "/a", value: 1 }]);
    expect(parsePatchBlock("[{op: 'add', path: '/b', value: [1,],}]")).toEqual([{ op: "add", path: "/b", value: [1] }]);
    expect(parsePatchBlock('[{"op":"remove","path":"/c"}')).toEqual([{ op: "remove", path: "/c" }]);
  });

  it("YAML 合併鍵可用", () => {
    expect(parsePatchBlock("base: &b {op: add}\nx:\n  <<: *b\n  path: /y")).toEqual({
      base: { op: "add" },
      x: { op: "add", path: "/y" },
    });
  });
});

describe("值上限與 runEval", () => {
  it("深度、循環、字串、非有限以外的上限", () => {
    expect(valueLimitProblem({ a: [1, "x"] })).toBeNull();
    let deep: unknown = 1;
    for (let i = 0; i < 40; i++) deep = [deep];
    expect(valueLimitProblem(deep)).toBe("too-deep");
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(valueLimitProblem(loop)).toBe("circular");
    expect(valueLimitProblem("字".repeat(30000))).toBe("string-too-long");
    expect(valueLimitProblem(Array.from({ length: 10_001 }, () => 0))).toBe("too-many-children");
  });

  it("runEval：成功帶值；結果超限、解析失敗回錯", () => {
    expect(runEval("value", "1+1")).toEqual({ ok: true, value: 2 });
    const deep = `${"[".repeat(40)}1${"]".repeat(40)}`;
    expect(runEval("value", deep)).toEqual({ ok: false, error: "value-limit:too-deep" });
    expect(runEval("patch", "[1, 2")).toMatchObject({ ok: true });
  });
});
