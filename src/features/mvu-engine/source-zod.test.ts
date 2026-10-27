// zod 版 MVU 卡（mvu_zod）：開局驗證、逐條套用與回傳契約。預期值照上游 StageDog/tavern_resource `util/mvu_zod.ts`
// （29becf3）逐案推導後手寫，不是拿本實作的輸出回填；另在本機直接執行上游程式比對過（上游授權不能入版控，對照
// 程式不在 repo）。夾具是自製的乾淨 schema（fixtures/，構件照真卡）。
import { existsSync, readFileSync } from "node:fs";
import YAML from "yaml";
import { z } from "zod";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { buildMvuEngineSource } from "./engine-source";
import type { MvuCore, MvuDetailedResult } from "./host-contract";
import { runEval } from "./value-parser";

const lodashName = "lodash";
const lodash = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

const createMvuCore = new Function(`${buildMvuEngineSource()}\nreturn createMvuCore;`)() as (env: unknown) => MvuCore;
const fixture = (name: string) => readFileSync(new URL(`./fixtures/${name}`, import.meta.url), "utf8");

beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "error").mockImplementation(() => {});
});

type Table = Record<string, any>;

function newCore(withZod = true): MvuCore {
  return createMvuCore({
    z: withZod ? z : undefined,
    lodash,
    events: {},
    emit: async () => {},
    host: {
      evaluate: (op: "value" | "patch" | "zod-value", text: string, done: (reply: unknown) => void) => {
        done(runEval(op, text));
        return null;
      },
      utf8Length: (text: string) => new TextEncoder().encode(text).length,
      macros: () => ({ user: "玩家", char: null }),
    },
  });
}

/** 卡片 schema 腳本：測試用改寫（import 換成參數、拿掉 export）；真宿主由 loader 對應（P3／P5） */
function loadSchemaScript(core: MvuCore, source: string) {
  const pattern = /^import \{ registerMvuSchema(?: as (\w+))? \} from '[^']+';/m;
  const alias = pattern.exec(source)?.[1] ?? "registerMvuSchema";
  const body = source.replace(pattern, "").replace(/export const/g, "const");
  new Function("z", "_", "$", alias, body)(z, lodash, (fn: () => void) => fn(), core.registerMvuSchema);
}

function initialized(core: MvuCore): Table {
  const stat = YAML.parse(fixture("clean-zod-initvar.yaml")) as Table;
  core.cleanUpMetadata(stat);
  const variables: Table = { stat_data: stat };
  expect(core.zodInitialize(variables)).toEqual({ ok: true, zod: true });
  return variables;
}

function zodCore(): MvuCore {
  const core = newCore();
  loadSchemaScript(core, fixture("clean-zod-schema.txt"));
  return core;
}

describe("開局（VARIABLE_INITIALIZED）", () => {
  it("補預設、轉型、夾值、自訂 transform；schema 沒列的頂層鍵保留", () => {
    const core = zodCore();
    const variables = initialized(core);
    expect(variables.stat_data).toEqual({
      世界: { 天氣: "陰", 威脅: 30, 設定: {} },
      隊員: {
        甲: {
          名字: "甲",
          職業: "法師",
          體力: "80/100",
          士氣: 60,
          背包: { 金幣: { 說明: "錢", 數量: 5 }, 藥水: { 說明: "回復", 數量: 2 } },
        },
        乙: { 名字: "乙", 職業: "盜賊", 體力: "50/50", 士氣: 100, 背包: {} },
      },
      檔案: { 編號: "B7", 等級: 5 },
      標籤: ["起點", "森林"],
      稱號: "見習",
    });
    const loose: Table = { stat_data: { ...variables.stat_data, 額外: 1 } };
    core.zodInitialize(loose);
    expect(loose.stat_data.額外).toBe(1);
  });

  it("驗不過：照原值、回錯誤；沒有 schema 時不是 zod 卡", () => {
    const core = zodCore();
    const variables: Table = { stat_data: { 檔案: { 等級: 1 } } };
    const outcome = core.zodInitialize(variables);
    expect(outcome).toMatchObject({ ok: false, zod: true, issueCount: 1 });
    expect(variables.stat_data).toEqual({ 檔案: { 等級: 1 } });
    expect(newCore().zodInitialize({ stat_data: {} })).toEqual({ ok: true, zod: false });
  });
});

interface Expectation {
  applied: number[];
  rejected?: number[];
  ignored?: number[];
  noChange?: number[];
  check?: (stat: Table, start: Table) => void;
}

const unchanged = (stat: Table, start: Table) => expect(stat).toEqual(start);

const EXPECTED: Record<string, Expectation> = {
  "set 夾值": { applied: [0], check: (s) => expect(s.世界.威脅).toBe(100) },
  "set 三參數取最後": { applied: [0], check: (s) => expect(s.世界.威脅).toBe(20) },
  "set 字串欄寫數字拒收": { applied: [], rejected: [0], check: unchanged },
  "set 轉不成數字拒收、保留舊值": { applied: [], rejected: [0], check: unchanged },
  "set 數字字串轉型": { applied: [0], check: (s) => expect(s.世界.威脅).toBe(42) },
  "set 沒有數學式": { applied: [], rejected: [0], check: unchanged },
  "set 物件字面量": { applied: [0], check: (s) => expect(s.世界.設定).toEqual({ 難度: "高", 回合: 3 }) },
  "set 路徑前綴剝掉": {
    applied: [0, 1],
    check: (s) => expect([s.世界.天氣, s.世界.威脅, s.stat_data]).toEqual(["雨", 5, undefined]),
  },
  "set 路徑不存在照建": {
    applied: [0],
    check: (s) => expect(s.隊員.丙).toEqual({ 名字: "丙", 職業: "戰士", 體力: "100/100", 士氣: 0, 背包: {} }),
  },
  "set 頂層新鍵保留": { applied: [0], check: (s) => expect(s.筆記).toBe("自由欄") },
  唯讀路徑靜默忽略: { applied: [], ignored: [0, 1, 2, 3], check: unchanged },
  "add 數字與夾值": { applied: [0, 1], check: (s) => expect(s.世界.威脅).toBe(100) },
  "add 舊值不是數字拒收": { applied: [], rejected: [0, 1], check: unchanged },
  "add 增量是字串": { applied: [0], check: (s) => expect(s.世界.威脅).toBe(37) },
  "insert 補預設值": {
    applied: [0],
    check: (s) => expect(s.隊員.丁).toEqual({ 名字: "丁", 職業: "戰士", 體力: "100/100", 士氣: 0, 背包: {} }),
  },
  "insert 缺必填拒收": { applied: [], rejected: [0], check: unchanged },
  "insert enum 錯拒收": { applied: [], rejected: [0], check: unchanged },
  "insert 容器不存在先物件": { applied: [0], check: (s) => expect(s.新區).toEqual({ 鑰匙: 1 }) },
  "insert 容器不存在物件不過改陣列": { applied: [0, 1], check: (s) => expect(s.標籤).toEqual(["新標"]) },
  "insert 陣列索引與尾端": { applied: [0, 1], check: (s) => expect(s.標籤).toEqual(["頭", "起點", "森林", "尾"]) },
  "insert 目標是原始型別忽略": { applied: [], ignored: [0], check: unchanged },
  "insert 合併物件": { applied: [0], check: (s) => expect(s.隊員.甲.士氣).toBe(100) },
  "巢狀未列鍵被剝掉＝沒變": { applied: [], noChange: [0, 1], check: unchanged },
  "delete record 與兩參數": { applied: [0, 1], check: (s) => expect(s.隊員).toEqual({}) },
  "delete 陣列元素 pullAt": { applied: [0], check: (s) => expect(s.標籤).toEqual(["森林"]) },
  "delete 必填拒收": { applied: [], rejected: [0], check: unchanged },
  // 稱號有 prefault：刪掉後整份驗證又補回預設值
  "delete 頂層鍵": { applied: [0], check: (s) => expect(s.稱號).toBe("新人") },
  "move 搬移": {
    applied: [0],
    check: (s, start) => expect(s.隊員).toEqual({ 乙: start.隊員.乙, 庚: start.隊員.甲 }),
  },
  "move 陣列": {
    applied: [0],
    check: (s) => expect([s.標籤, s.世界.設定]).toEqual([["森林"], { 舊標: "起點" }]),
  },
  "move 來源不存在拒收": { applied: [], rejected: [0], check: unchanged },
  "move 頂層": { applied: [0], check: (s) => expect([s.稱號, s.舊稱號]).toEqual(["新人", "見習"]) },
  // unset 世界.設定：prefault 補回 {}，與原值相同＝沒變
  "別名 assign remove unset": {
    applied: [0, 1],
    noChange: [2],
    check: (s) => expect(Object.keys(s.隊員)).toEqual(["甲", "壬"]),
  },
  "JSONPatch 各種": {
    applied: [0, 1, 2, 4],
    noChange: [3],
    check: (s) => {
      expect(s.世界.天氣).toBe("霧");
      expect(s.隊員.甲.士氣).toBe(75);
      expect(s.隊員.癸.名字).toBe("癸");
      expect(s.隊員.乙.背包).toEqual({});
      expect(s.世界.設定).toEqual({ "a/b": 1 });
    },
  },
  "自訂 transform 背包整理": {
    applied: [0, 1, 2, 3, 4, 5],
    check: (s) =>
      expect(s.隊員.甲.背包).toEqual({
        金幣: { 說明: "錢", 數量: 5 },
        銀幣: { 說明: "無", 數量: 9 },
        火把: { 說明: "無", 數量: 1 },
        地圖: { 說明: "無", 數量: 1 },
        乾糧: { 說明: "無", 數量: 2 },
      }),
  },
  "自訂 transform 體力字串": {
    applied: [0, 1],
    check: (s) => expect([s.隊員.甲.體力, s.隊員.乙.體力]).toEqual(["999/999", "500/500"]),
  },
  拒收後續指令照跑: {
    applied: [0, 2, 4],
    rejected: [1, 3],
    check: (s) => expect([s.世界.天氣, s.世界.威脅, s.稱號, s.隊員.缺]).toEqual(["雪", 31, "老手", undefined]),
  },
  "set 同值沒變": { applied: [], noChange: [0], check: unchanged },
  "or literal": { applied: [0], rejected: [1], check: (s) => expect(s.稱號).toBe(0) },
  // 根路徑：結果淺合併回去，新值沒有的頂層鍵（標籤）照舊留著
  根路徑整份換掉: {
    applied: [0],
    check: (s, start) =>
      expect(s).toEqual({
        世界: { 天氣: "晴", 威脅: 0, 設定: {} },
        隊員: {},
        檔案: { 編號: "A0", 等級: 1 },
        標籤: start.標籤,
        稱號: "新人",
      }),
  },
  沒有指令: { applied: [], check: unchanged },
  巨集: { applied: [0], check: (s) => expect(s.世界.天氣).toBe("玩家的雨") },
};

describe("逐條套用（COMMAND_PARSED_for_zod）：上游對照", () => {
  const corpus = JSON.parse(fixture("zod-corpus.json")) as { name: string; message: string }[];

  it("對照清單與預期一一對應", () => {
    expect(corpus.map((entry) => entry.name).sort()).toEqual(Object.keys(EXPECTED).sort());
  });

  for (const { name, message } of corpus) {
    it(name, async () => {
      const core = zodCore();
      const start = initialized(core);
      const detailed: MvuDetailedResult = await core.parseMessageDetailed(message, structuredClone(start));
      const expected = EXPECTED[name]!;
      const indices = (status: string) => detailed.results.filter((entry) => entry.status === status).map((entry) => entry.index);
      expect(detailed.zod).toBe(true);
      expect(indices("applied")).toEqual(expected.applied);
      expect(indices("rejected")).toEqual(expected.rejected ?? []);
      expect(indices("ignored")).toEqual(expected.ignored ?? []);
      expect(indices("no-change")).toEqual(expected.noChange ?? []);
      expected.check?.(detailed.table.stat_data as Table, start.stat_data);
    });
  }
});

describe("回傳契約與收尾", () => {
  it("新表照上游收尾：schema 換佔位、不留 display／delta；變動是逐葉路徑", async () => {
    const core = zodCore();
    const start = initialized(core);
    const detailed = await core.parseMessageDetailed("_.set('世界.威脅', 150);_.set('世界.天氣', 7);", start);
    expect(detailed.table.schema).toBe("没有用别管这个");
    expect(detailed.table.display_data).toBeUndefined();
    expect(detailed.table.delta_data).toBeUndefined();
    expect(detailed.changes).toEqual([{ path: ["世界", "威脅"], before: 30, after: 100 }]);
    expect(detailed.changesTruncated).toBe(false);
    const rejected = detailed.results[1]!;
    expect(rejected).toMatchObject({ index: 1, op: "set", path: ["世界", "天氣"], status: "rejected", reason: "schema", issueCount: 1 });
    expect(rejected.issues![0]).toContain("世界.天氣");
  });

  it("schema 拒收的錯誤文字限條數與字數", async () => {
    const core = zodCore();
    const start = initialized(core);
    const many = JSON.stringify(Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`k${i}`, { 職業: "x".repeat(900) }])));
    const detailed = await core.parseMessageDetailed(`_.set('隊員', ${many});`, start);
    const entry = detailed.results[0]!;
    expect(entry.status).toBe("rejected");
    expect(entry.issueCount).toBeGreaterThan(5);
    expect(entry.issues).toHaveLength(5);
    for (const issue of entry.issues!) expect(issue.length).toBeLessThanOrEqual(500);
    expect(entry.issues!.some((issue) => issue.endsWith("…"))).toBe(true);
  });

  it("變動超過上限就截斷；單筆值太大只記路徑", async () => {
    const core = newCore();
    core.registerMvuSchema(z.object({}));
    const wide = Object.fromEntries(Array.from({ length: 2100 }, (_, i) => [`k${i}`, i]));
    const detailed = await core.parseMessageDetailed(`_.set('寬', ${JSON.stringify(wide)});_.set('長', '${"字".repeat(5000)}');`, {
      stat_data: { 寬: {}, 長: "" },
    });
    expect(detailed.changes).toHaveLength(2000);
    expect(detailed.changesTruncated).toBe(true);
    const long = await core.parseMessageDetailed(`_.set('長', '${"字".repeat(5000)}');`, { stat_data: { 長: "" } });
    expect(long.changes).toEqual([{ path: ["長"], before: "", truncated: true }]);
  });

  it("非冪等 transform：每次整份驗證都再跑一次，所以每條通過的指令都算變動", async () => {
    const core = newCore();
    core.registerMvuSchema(z.object({ 計數: z.coerce.number().transform((v) => v + 1), 名: z.string() }));
    const detailed = await core.parseMessageDetailed("_.set('名', 'a');_.set('名', 'a');_.set('名', 1);", {
      stat_data: { 計數: 0, 名: "a" },
    });
    expect(detailed.results.map((entry) => entry.status)).toEqual(["applied", "applied", "rejected"]);
    expect(detailed.table.stat_data).toEqual({ 計數: 2, 名: "a" });
  });

  it("schema 以函式註冊：註冊時與每批各展開一次；第二份註冊只計數；展開拋錯不算註冊", async () => {
    const core = newCore();
    const factory = vi.fn(() => z.object({ a: z.number() }));
    core.registerMvuSchema(factory);
    core.registerMvuSchema(z.object({}));
    expect(core.zodStatus()).toEqual({ registered: true, duplicates: 1 });
    await core.parseMessageDetailed("_.set('a', 1);_.set('a', 2);", { stat_data: { a: 0 } });
    expect(factory).toHaveBeenCalledTimes(2);
    const broken = newCore();
    expect(() =>
      broken.registerMvuSchema(() => {
        throw new Error("壞掉的 schema");
      }),
    ).toThrow("壞掉的 schema");
    expect(broken.zodStatus()).toEqual({ registered: false, duplicates: 0 });
  });

  it("沒有 z（iframe）或沒註冊：照 MagVarUpdate 跑，results 為空", async () => {
    const plain = newCore(false);
    plain.registerMvuSchema(z.object({ a: z.number() }));
    const detailed = await plain.parseMessageDetailed("_.set('a', '5');", { stat_data: { a: 1 } });
    expect(detailed.zod).toBe(false);
    expect(detailed.results).toEqual([]);
    expect(detailed.table.stat_data).toEqual({ a: 5 });
    expect(detailed.table.display_data).toEqual({ a: "1->5 " });
  });

  it("transform 產生的非有限數存成 null（同上游 JSON 存檔）", async () => {
    const core = newCore();
    core.registerMvuSchema(z.object({ 值: z.any().transform((v) => (v === "炸" ? Infinity : v)) }));
    const detailed = await core.parseMessageDetailed("_.set('值', '炸');", { stat_data: { 值: 1 } });
    expect(detailed.table.stat_data).toEqual({ 值: null });
  });

  it("宿主值解析回錯（逾時、值超過上限）：整次失敗，前面成功的指令也不提交", async () => {
    for (const error of ["timeout", "value-limit:too-deep"]) {
      let calls = 0;
      const core = createMvuCore({
        z,
        lodash,
        events: {},
        emit: async () => {},
        host: {
          evaluate: (op: "zod-value", text: string, done: (reply: unknown) => void) => {
            calls += 1;
            done(calls === 2 ? { ok: false, error } : runEval(op, text));
            return null;
          },
          utf8Length: (text: string) => text.length,
          macros: () => null,
        },
      });
      core.registerMvuSchema(z.object({ a: z.any(), b: z.any() }));
      await expect(core.parseMessageDetailed("_.set('a', 1);_.set('b', 2);", { stat_data: { a: 0, b: 0 } })).rejects.toMatchObject({
        code: "eval-failed",
      });
    }
  });

  it("物件字面量含運算式不求值（JSON5 解不了，YAML 當字串值）", async () => {
    const core = newCore();
    core.registerMvuSchema(z.object({ b: z.any() }));
    const detailed = await core.parseMessageDetailed("_.set('b', {x: 1+1});", { stat_data: { b: 0 } });
    expect(detailed.table.stat_data).toEqual({ b: { x: "1+1" } });
  });

  it("路徑只走自有屬性：constructor／prototype／__proto__ 碰不到原型，拒收後原型仍乾淨", async () => {
    const before = Object.getOwnPropertyNames(Object.prototype).sort();
    const core = zodCore();
    const start = initialized(core);
    const messages = [
      "_.insert('constructor.prototype', 'p2_probe', 123);",
      "_.insert('隊員.constructor.prototype', 'p2_probe', 123);",
      "_.set('constructor.prototype.p2_probe', 1);",
      "_.add('constructor.prototype.toString', 1);",
      "_.insert('__proto__', 'p2_probe', 1);",
      "_.insert('隊員', '__proto__', {名字: '原型'});",
      '<JSONPatch>[{"op":"add","path":"/__proto__/p2_probe","value":1},{"op":"move","from":"/__proto__","path":"/搬"}]</JSONPatch>',
      "_.delete('constructor.prototype.hasOwnProperty');",
    ];
    const statuses: string[][] = [];
    for (const message of messages) {
      const detailed = await core.parseMessageDetailed(message, structuredClone(start));
      statuses.push(detailed.results.map((entry) => entry.status));
    }
    expect(({} as Record<string, unknown>).p2_probe).toBeUndefined();
    expect(Object.getOwnPropertyNames(Object.prototype).sort()).toEqual(before);
    expect(typeof Object.prototype.hasOwnProperty).toBe("function");
    // 隊員底下多出的 constructor 項沒有名字 → 拒收；__proto__ 只是一般的自有鍵
    expect(statuses[1]).toEqual(["rejected"]);
    const own = await core.parseMessageDetailed("_.insert('新區', '__proto__', 5);", structuredClone(start));
    const area = own.table.stat_data as Table;
    expect(Object.getOwnPropertyDescriptor(area.新區, "__proto__")?.value).toBe(5);
    expect(Object.getPrototypeOf(area.新區)).toBe(Object.prototype);
  });

  it("結果路徑是分段陣列：鍵名含點也對得到；move 另記目的地", async () => {
    const core = zodCore();
    const start = initialized(core);
    const detailed = await core.parseMessageDetailed(
      '_.set(\'世界.設定["a.b"]\', 1);<JSONPatch>[{"op":"move","from":"/隊員/甲","path":"/隊員/x.y"}]</JSONPatch>',
      structuredClone(start),
    );
    expect(detailed.results.map((entry) => [entry.path, entry.to])).toEqual([
      [["世界", "設定", "a.b"], undefined],
      [["隊員", "甲"], ["隊員", "x.y"]],
    ]);
    expect((detailed.table.stat_data as Table).世界.設定).toEqual({ "a.b": 1 });
  });

  it("字面鍵與巢狀鍵同時存在：results 的路徑分段和實際寫入、changes 一致", async () => {
    const core = newCore();
    core.registerMvuSchema(z.object({}));
    const detailed = await core.parseMessageDetailed("_.set('a.b', 3);_.set('a.c', 4);", {
      stat_data: { "a.b": 1, a: { b: 2 } },
    });
    expect(detailed.table.stat_data).toEqual({ "a.b": 3, a: { b: 2, c: 4 } });
    expect(detailed.results.map((entry) => entry.path)).toEqual([["a.b"], ["a", "c"]]);
    expect(detailed.changes.map((change) => change.path)).toEqual([["a.b"], ["a", "c"]]);
  });

  it("move 刪來源就失敗：原因記刪除那一步，不是之後對 null 的 set", async () => {
    const core = zodCore();
    const start = initialized(core);
    const detailed = await core.parseMessageDetailed(
      '<JSONPatch>[{"op":"move","from":"/檔案/編號","path":"/舊編號"}]</JSONPatch>',
      structuredClone(start),
    );
    expect(detailed.results[0]).toMatchObject({ status: "rejected", reason: "move-source-schema" });
    expect(detailed.results[0]!.issues![0]).toContain("檔案.編號");
  });

  it("錯誤文字路徑在前、只截輸入那段：輸入很長也對得到欄位", async () => {
    const core = zodCore();
    const start = initialized(core);
    const detailed = await core.parseMessageDetailed(`_.set('世界.天氣', [${'"長",'.repeat(400)}"尾"]);`, structuredClone(start));
    const issue = detailed.results[0]!.issues![0]!;
    expect(issue.startsWith("路径: 世界.天氣 ✖ ")).toBe(true);
    expect(issue.length).toBeLessThanOrEqual(500);
    expect(issue.endsWith("…")).toBe(true);
    const add = await core.parseMessageDetailed("_.add('世界.天氣', 1);", structuredClone(start));
    expect(add.results[0]!.issues![0]).toMatch(/^路径: 世界\.天氣 ✖ 不能对非数字类型 'string' 进行加减运算 → 原值: "陰"$/);
  });

  it("寫入路徑遇到函式就換成新容器：碰不到函式的 prototype", async () => {
    const core = newCore();
    const probe = function probe() {};
    core.registerMvuSchema(z.object({ f: z.any().transform((v) => (v === "函式" ? probe : v)), g: z.any() }));
    const detailed = await core.parseMessageDetailed("_.set('f', '函式');_.set('f.prototype.p2_fn', 1);", { stat_data: { f: 0, g: 0 } });
    expect(detailed.results.map((entry) => entry.status)).toEqual(["applied", "applied"]);
    expect((probe.prototype as Record<string, unknown>).p2_fn).toBeUndefined();
    expect((detailed.table.stat_data as Table).f).toEqual({ prototype: { p2_fn: 1 } });
  });

  it("開局的 transform 非有限數同樣存成 null", () => {
    const core = newCore();
    core.registerMvuSchema(z.object({ 值: z.any().transform(() => Infinity) }));
    const variables: Table = { stat_data: { 值: 1 } };
    expect(core.zodInitialize(variables)).toEqual({ ok: true, zod: true });
    expect(variables.stat_data).toEqual({ 值: null });
  });
});

// TestCards/ 是 gitignore 的本機測試卡：有才跑，CI 上略過
const cardsDir = new URL("../../../TestCards/", import.meta.url);
const CARDS = ["DongeonMaster.png", "bcd368cacbc56b2cc29010e0cbad5f09.png", "HeroTrainingUnderSide_stats.png"];
const hasCards = CARDS.every((file) => existsSync(new URL(file, cardsDir)));

/** 原卡的「变量结构」腳本與 [initvar] 條目 */
function realCard(file: string): { schema: string; initvar: string } {
  const bytes = readFileSync(new URL(file, cardsDir));
  let offset = 8;
  let json = "";
  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.toString("latin1", offset + 4, offset + 8);
    if (type === "tEXt") {
      const chunk = bytes.subarray(offset + 8, offset + 8 + length);
      const split = chunk.indexOf(0);
      const key = chunk.toString("latin1", 0, split);
      if (key === "ccv3" || (key === "chara" && json === "")) json = Buffer.from(chunk.toString("latin1", split + 1), "base64").toString("utf8");
    }
    offset += 12 + length;
  }
  const data = (JSON.parse(json) as { data: Table }).data;
  const scripts = data.extensions.tavern_helper.scripts as Table[];
  const schema = scripts.find((script) => String(script.content).includes("registerMvuSchema"))!.content as string;
  const entries = data.character_book.entries as Table[];
  const initvar = entries.find((entry) => /initvar/i.test(String(entry.comment ?? entry.name)))!.content as string;
  return { schema, initvar };
}

describe.skipIf(!hasCards)("真卡 schema（TestCards 本機限定）", () => {
  async function run(file: string, message: (stat: Table) => string) {
    const card = realCard(file);
    const core = newCore();
    loadSchemaScript(core, card.schema);
    const stat = YAML.parse(card.initvar.replace(/<\/?Initial_Variables>/g, "").replace(/\{\{user\}\}/g, "玩家")) as Table;
    core.cleanUpMetadata(stat);
    const variables: Table = { stat_data: stat };
    expect(core.zodInitialize(variables)).toMatchObject({ ok: true, zod: true });
    const detailed = await core.parseMessageDetailed(message(variables.stat_data), variables);
    return { stat: detailed.table.stat_data as Table, status: detailed.results.map((entry) => entry.status), start: variables.stat_data };
  }

  it("DongeonMaster：夾值、字串欄拒收數字、轉不成數字拒收、巢狀未列鍵沒變", async () => {
    let adventurer = "";
    const { stat, status } = await run("DongeonMaster.png", (start) => {
      adventurer = Object.keys(start.冒险者 ?? {})[0] ?? "测试冒险者";
      const create = start.冒险者?.[adventurer] ? "" : `_.insert('冒险者', '${adventurer}', {});`;
      return `${create}_.add('冒险者.${adventurer}.生理与心理数值.恶堕值', 500);_.set('基础信息.时间', 7);_.set('基础信息.已覆灭小队数', 'x');_.insert('冒险者.${adventurer}.穿着', '披风', 'y');`;
    });
    expect(status.slice(-4)).toEqual(["applied", "rejected", "rejected", "no-change"]);
    expect(stat.冒险者[adventurer].生理与心理数值.恶堕值).toBe(100);
  });

  it("bcd368：漏必填與 enum 錯拒收、0–5 夾值、數字字串轉型", async () => {
    const { stat, status } = await run(
      "bcd368cacbc56b2cc29010e0cbad5f09.png",
      () =>
        "_.insert('便器数据库', 'T999', {基础档案: {姓名: 'a'}});_.set('便器数据库.T001.基础档案.评级', '普通');_.set('便器数据库.T001.配种信息.开发阶段', 9);_.set('用户数据.积分', 'abc');_.set('用户数据.积分', '250');",
    );
    expect(status).toEqual(["rejected", "rejected", "applied", "rejected", "applied"]);
    expect([stat.便器数据库.T001.配种信息.开发阶段, stat.用户数据.积分]).toEqual([5, 250]);
  });

  it("HeroTraining：兩個自訂 transform（物品整理、xx/xx 數值）與夾值", async () => {
    const items = Object.fromEntries(Array.from({ length: 11 }, (_, i) => [`杂物${i + 1}`, { 描述: "x", 数量: 1 }]));
    const inventory = JSON.stringify({ 金龙: { 描述: "钱", 数量: 5 }, 破布: { 描述: "y", 数量: 0 }, ...items });
    const { stat, status } = await run(
      "HeroTrainingUnderSide_stats.png",
      () =>
        `_.set('Player.HP', '12000/10000');_.set('Player.MP', 500);_.set('Player.Inventory', ${inventory});_.add('World.Invasion', 500);_.set('World.Roll20', 'abc');_.insert('Heroes', '新勇者', {Name: 'A'});_.set('Player._hidden', 1);`,
    );
    expect(status).toEqual(["applied", "applied", "applied", "applied", "rejected", "applied", "ignored"]);
    expect([stat.Player.HP, stat.Player.MP, stat.World.Invasion]).toEqual(["10000/10000", "500/500", 100]);
    expect(Object.keys(stat.Player.Inventory)).toEqual(["金龙", ...Array.from({ length: 10 }, (_, i) => `杂物${i + 2}`)]);
    expect([stat.Heroes.新勇者.HP, stat.Heroes.新勇者.Desire, stat.Heroes.新勇者.Outfit.上装]).toEqual(["10000/10000", 0, "无"]);
  });
});
