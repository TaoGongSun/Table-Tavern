// 桌面 QuickJS 入口（quickjs-entry.ts）在 Node 上的行為，以及 Node 與 QuickJS 共用的對照基準：
// fixtures/zod-corpus-golden.json 是本入口在 Node 跑 41 案的完整輸出；cargo 測試（src-tauri/src/mvu_engine/tests.rs）
// 拿同一份比 QuickJS。基準本身與上游一致由 source-zod.test.ts 的逐案預期把關。
// 重產：UPDATE_ZOD_GOLDEN=1 npx vitest run src/features/mvu-engine/quickjs-entry.test.ts
import { readFileSync, writeFileSync } from "node:fs";
import YAML from "yaml";
import { z } from "zod";
import { describe, expect, it } from "vitest";
import { buildMvuEngineSource } from "./engine-source";
import type { MvuCore } from "./host-contract";
import { REAL_CARDS, cardsDir, hasRealCards, realCard } from "./fixtures/real-cards";
import { createDesktopEngine, isQuickJsFatal, utf8Length } from "./quickjs-entry";
import { runEval } from "./value-parser";

const lodashName = "lodash";
const lodash = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

const fixtureUrl = (name: string) => new URL(`./fixtures/${name}`, import.meta.url);
const fixture = (name: string) => readFileSync(fixtureUrl(name), "utf8");

/** 卡片 schema 腳本的測試改寫（import 換成參數、拿掉 export）；QuickJS 由 loader 對應墊片 */
function loadSchema(register: (input: unknown) => void, source: string) {
  const pattern = /^import \{ registerMvuSchema(?: as (\w+))? \} from '[^']+';/m;
  const alias = pattern.exec(source)?.[1] ?? "registerMvuSchema";
  const body = source.replace(pattern, "").replace(/export const/g, "const");
  new Function("z", "_", "$", alias, body)(z, lodash, (fn: () => void) => fn(), register);
}

const MACROS = JSON.stringify({ user: "玩家", char: null });

async function corpusOutput() {
  const engine = createDesktopEngine();
  loadSchema(engine.register, fixture("clean-zod-schema.txt"));
  const initvar: unknown = YAML.parse(fixture("clean-zod-initvar.yaml"));
  const init = JSON.parse(engine.initialize(JSON.stringify(initvar))) as { outcome: unknown; stat_data: unknown };
  const corpus = JSON.parse(fixture("zod-corpus.json")) as { name: string; message: string }[];
  const cases = [];
  for (const { name, message } of corpus) {
    const detailed = JSON.parse(await engine.parse(message, JSON.stringify({ stat_data: init.stat_data }), MACROS)) as Record<string, unknown>;
    cases.push({ name, message, ...detailed });
  }
  return { initvar, init, cases };
}

describe("Node 與 QuickJS 的共用基準", () => {
  it("41 案輸出與 fixtures/zod-corpus-golden.json 一致", async () => {
    const output = await corpusOutput();
    if (process.env.UPDATE_ZOD_GOLDEN) writeFileSync(fixtureUrl("zod-corpus-golden.json"), `${JSON.stringify(output, null, 1)}\n`);
    expect(output).toEqual(JSON.parse(fixture("zod-corpus-golden.json")));
    expect(output.cases).toHaveLength(41);
  });
});

// TestCards/ 是 gitignore 的本機測試卡：有才跑，CI 上略過。輸出 TestCards/zod-cards-golden.json 給 cargo 比 QuickJS
describe.skipIf(!hasRealCards)("真卡基準（TestCards 本機限定）", () => {
  it("Node 跑三張真卡的開局與一批指令", async () => {
    const output = [];
    for (const { file, message } of REAL_CARDS) {
      const card = realCard(file);
      const engine = createDesktopEngine();
      loadSchema(engine.register, card.schema);
      const init = JSON.parse(engine.initialize(JSON.stringify(card.initvar))) as { outcome: unknown; stat_data: Record<string, unknown> };
      expect(init.outcome).toMatchObject({ ok: true, zod: true });
      const text = message(init.stat_data);
      const detailed: unknown = JSON.parse(await engine.parse(text, JSON.stringify({ stat_data: init.stat_data }), MACROS));
      output.push({ name: file, schema: card.schema, initvar: card.initvar, init, message: text, detailed });
    }
    writeFileSync(new URL("zod-cards-golden.json", cardsDir), JSON.stringify(output));
  });
});

describe("入口介面", () => {
  it("沒註冊時 status 回未註冊；註冊後重複的只計數", () => {
    const engine = createDesktopEngine();
    expect(JSON.parse(engine.status())).toEqual({ registered: false, duplicates: 0 });
    engine.register(z.object({}));
    engine.register(z.object({}));
    expect(JSON.parse(engine.status())).toEqual({ registered: true, duplicates: 1 });
  });

  it("巨集只在該次 parse 生效", async () => {
    const engine = createDesktopEngine();
    engine.register(z.object({ a: z.string() }));
    const first = JSON.parse(await engine.parse("_.set('a', '{{user}}');", JSON.stringify({ stat_data: { a: "" } }), MACROS));
    expect(first.table.stat_data).toEqual({ a: "玩家" });
    const second = JSON.parse(await engine.parse("_.set('a', '{{user}}');", JSON.stringify({ stat_data: { a: "" } }), "null"));
    expect(second.table.stat_data.a).not.toBe("玩家");
  });

  it("種子：各條 initvar 依序深合併（陣列整個取代）、巨集代換、保序，再過 schema", () => {
    const engine = createDesktopEngine();
    engine.register(z.object({ 名字: z.string(), 數值: z.coerce.number().transform((v) => Math.min(v, 100)) }).loose());
    const initvars = ["<initvar>\n乙: 1\n名字: '{{user}}和{{char}}'\n清單: [1, 2]\n</initvar>", "數值: '500'\n清單: [3]\n甲: 2"];
    const seeded = JSON.parse(engine.seed(JSON.stringify(initvars), "", MACROS, "null")) as { outcome: unknown; stat_data: Record<string, unknown> };
    expect(seeded.outcome).toEqual({ ok: true, zod: true });
    expect(seeded.stat_data).toEqual({ 乙: 1, 名字: "玩家和{{char}}", 清單: [3], 數值: 100, 甲: 2 });
    expect(Object.keys(seeded.stat_data)).toEqual(["乙", "名字", "清單", "數值", "甲"]);
  });

  it("種子：開場白的 <initvar> 整份取代世界書初始值", () => {
    const engine = createDesktopEngine();
    engine.register(z.object({ 名字: z.string().prefault("無名") }).loose());
    const seeded = JSON.parse(engine.seed(JSON.stringify(["名字: 甲\n舊: 1"]), "開場<initvar>\n新: 2\n</initvar>", MACROS, "null"));
    expect(seeded.stat_data).toEqual({ 新: 2, 名字: "無名" });
  });

  it("種子：沒有 initvar 時以樹物化的表為底（不再代換巨集），一樣過 schema、開場白 <initvar> 照樣取代", () => {
    const engine = createDesktopEngine();
    engine.register(z.object({ 名字: z.string().prefault("無名"), 數值: z.coerce.number().transform((v) => Math.min(v, 100)) }).loose());
    const base = JSON.stringify({ 數值: "500", 名字: "{{user}}", 甲: 1 });
    const seeded = JSON.parse(engine.seed("[]", "", MACROS, base));
    expect(seeded.stat_data).toEqual({ 數值: 100, 名字: "{{user}}", 甲: 1 });
    expect(Object.keys(seeded.stat_data)).toEqual(["數值", "名字", "甲"]);
    const overridden = JSON.parse(engine.seed("[]", "<initvar>\n數值: 7\n</initvar>", MACROS, base));
    expect(overridden.stat_data).toEqual({ 數值: 7, 名字: "無名" });
  });

  it("種子：沒註冊 schema（非 zod 桌）只做開場白 <initvar> 取代與清元資料，applied 標出有沒有套上", () => {
    const engine = createDesktopEngine();
    const base = JSON.stringify({ 金幣: 10, 地點: "村莊" });
    const none = JSON.parse(engine.seed("[]", "開場<initvar>*&:|*[</initvar>", MACROS, base));
    expect(none).toEqual({ outcome: { ok: true, zod: false }, stat_data: { 金幣: 10, 地點: "村莊" }, applied: false });
    const same = JSON.parse(engine.seed("[]", "<initvar>\n地點: 村莊\n金幣: 10\n$meta: {strictSet: true}\n</initvar>", MACROS, base));
    expect(same.applied).toBe(true);
    expect(Object.keys(same.stat_data)).toEqual(["地點", "金幣"]);
  });

  it("UTF-8 長度與 TextEncoder 相同（含代理對與落單代理）", () => {
    for (const text of ["", "abc", "中文字", "😀x", "a\ud800b", "\udc00", "éࠀ￿"]) {
      expect(utf8Length(text)).toBe(new TextEncoder().encode(text).length);
    }
  });
});

describe("資源類錯誤（host.fatal）", () => {
  it("判定：InternalError、堆疊溢位、null 是；一般錯誤不是", () => {
    const internal = Object.assign(new Error("out of memory"), { name: "InternalError" });
    expect(isQuickJsFatal(internal)).toBe(true);
    expect(isQuickJsFatal(new RangeError("Maximum call stack size exceeded"))).toBe(true);
    expect(isQuickJsFatal(null)).toBe(true);
    expect(isQuickJsFatal(new RangeError("數值超出範圍"))).toBe(false);
    expect(isQuickJsFatal(new Error("x"))).toBe(false);
    expect(isQuickJsFatal("字串")).toBe(false);
  });

  const createMvuCore = new Function(`${buildMvuEngineSource()}\nreturn createMvuCore;`)() as (env: unknown) => MvuCore;
  function core(fatal?: (error: unknown) => boolean): MvuCore {
    return createMvuCore({
      z,
      lodash,
      events: {},
      emit: async () => {},
      host: {
        evaluate: (op: "zod-value", text: string, done: (reply: unknown) => void) => {
          done(runEval(op, text));
          return null;
        },
        utf8Length,
        macros: () => null,
        fatal,
      },
    });
  }
  const boom = () => {
    throw Object.assign(new Error("out of memory"), { name: "InternalError" });
  };

  it("schema 拋資源類錯誤：整次失敗；沒給 fatal 的宿主照舊當單條拒收", async () => {
    const schema = z.object({ a: z.any().transform((v) => (v === "炸" ? boom() : v)) });
    const strict = core(isQuickJsFatal);
    strict.registerMvuSchema(schema);
    await expect(strict.parseMessageDetailed("_.set('a', '炸');", { stat_data: { a: 1 } })).rejects.toThrow("out of memory");
    const plain = core();
    plain.registerMvuSchema(schema);
    const detailed = await plain.parseMessageDetailed("_.set('a', '炸');", { stat_data: { a: 1 } });
    expect(detailed.results[0]).toMatchObject({ status: "rejected", reason: "schema" });
  });

  it("開局拋資源類錯誤：照拋，不回驗證失敗", () => {
    const strict = core(isQuickJsFatal);
    strict.registerMvuSchema(z.object({ a: z.any().transform(boom) }));
    expect(() => strict.zodInitialize({ stat_data: { a: 1 } })).toThrow("out of memory");
  });
});
