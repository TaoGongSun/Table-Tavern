// @vitest-environment happy-dom
// zod 卡（卡上有註冊 schema 的腳本）在一桌裡：開局（initvar → 開場白覆寫 → cleanUpMetadata → zodInitialize，只做一次）、
// 每則回覆走 zod 迴圈；整次失敗不退回 MagVarUpdate——那一則不寫變數、顯示可清除的提示，玩家下一次操作才重試。
// 隔離 iframe／Worker 換成同一份引擎的測試替身（zod/test-runner.ts）；隔離本身由 web/e2e/zod-isolation.mjs 在真瀏覽器測。
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { runEval } from "@desktop/features/mvu-engine/value-parser";
import { playCardFromValue } from "../cards/play-card";
import type { ChatEntry } from "../chat/chat-turn";
import { createChatVariables } from "../sillytavern/variables";
import { setLang } from "../../i18n";
import { useTableMvu, type SavedMvu, type TableMvu } from "./useTableMvu";
import { createTestZodRunner } from "./zod/test-runner";
import { ZodRunError, type ZodRunner } from "./zod/frame-host";

const zod = vi.hoisted(() => ({
  runners: [] as (ZodRunner & { calls: { parse: number; initialize: number } })[],
  failParse: 0,
  failInitialize: 0,
}));
vi.mock("./zod/frame-host", async (original) => {
  const actual = await original<typeof import("./zod/frame-host")>();
  return {
    ...actual,
    createZodRunner: ({ schema }: { schema: string }) => {
      const inner = createTestZodRunner(schema);
      const runner = {
        calls: inner.calls,
        parse: async (...args: Parameters<ZodRunner["parse"]>) => {
          if (zod.failParse > 0) {
            zod.failParse -= 1;
            throw new actual.ZodRunError("deadline", "超過 5 秒期限");
          }
          return inner.parse(...args);
        },
        initialize: async (stat: Record<string, unknown>) => {
          if (zod.failInitialize > 0) {
            zod.failInitialize -= 1;
            throw new actual.ZodRunError("schema-load", "載入失敗");
          }
          return inner.initialize(stat);
        },
        dispose: () => {},
      };
      zod.runners.push(runner);
      return runner;
    },
  };
});
vi.mock("./evaluate", () => ({ createEvaluator: () => ({ run: async (op: "value" | "patch", text: string) => runEval(op, text), dispose: () => {} }) }));

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});
beforeEach(() => {
  zod.runners.length = 0;
  zod.failParse = 0;
  zod.failInitialize = 0;
  vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => vi.restoreAllMocks());

const MVU_ZOD = "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";
// 非冪等 transform：每過一次 schema 計數加一（開局只能過一次）
const SCHEMA = `import { registerMvuSchema } from '${MVU_ZOD}';
export const Schema = z.object({ 計數: z.number().transform(v => v + 1), 燈火: z.enum(['暗', '亮']), 金幣: z.coerce.number().transform(v => _.clamp(v, 0, 999)) });
$(() => { registerMvuSchema(Schema); });`;

function zodCard(initvar = "計數: 0\n燈火: 暗\n金幣: 100\n", schema = SCHEMA) {
  return playCardFromValue("json", {
    spec: "chara_card_v2",
    spec_version: "2.0",
    data: {
      name: "莫拉",
      first_mes: "歡迎。<UpdateVariable>_.set('燈火', '亮');</UpdateVariable>",
      extensions: {
        tavern_helper: {
          scripts: [
            { type: "script", enabled: true, name: "MVU", id: "mvu-1", content: "import 'https://example.invalid/MagVarUpdate/bundle.js';" },
            { type: "script", enabled: true, name: "变量结构", id: "schema-1", content: schema },
          ],
        },
      },
      character_book: { name: "旅店書", entries: [{ keys: [], content: initvar, comment: "[initvar]", enabled: false, insertion_order: 1 }] },
    },
  });
}

let mvu!: TableMvu;
let entries: ChatEntry[] = [];

async function mount(options: { saved?: SavedMvu | null; resumed?: boolean; start?: ChatEntry[]; card?: ReturnType<typeof zodCard> } = {}) {
  const card = options.card ?? zodCard();
  entries = options.start ?? [{ id: "e0", role: "char", text: "歡迎。<UpdateVariable>_.set('燈火', '亮');</UpdateVariable>", opening: true }];
  const entriesRef = { current: entries };
  const variables = createChatVariables();
  function Probe() {
    mvu = useTableMvu({
      card,
      userName: "玩家",
      variables,
      saved: options.saved,
      resumed: options.resumed ?? false,
      entriesRef,
      setEntries: (update) => {
        entries = update(entries);
      },
      substitute: (text) => text,
    });
    return null;
  }
  const root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Probe />));
  // 等掛載時的載入與開局做完（不呼叫 ready：那是玩家送出前才有的一步）；QuickJS 開機約 0.3 秒，負載下更久
  for (let i = 0; i < 2000 && entries[0]?.vars === undefined && mvu.zodFailure === null && mvu.initError === null && !(options.resumed ?? false); i++) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5));
    });
  }
  if (options.resumed) await act(async () => mvu.ready());
  return { entriesRef, unmount: () => act(async () => root.unmount()) };
}

const stat = (vars: Record<string, unknown> | undefined) => (vars as { stat_data?: Record<string, unknown> } | undefined)?.stat_data;
const reply = (text: string): ChatEntry => ({ id: `r${Math.random()}`, role: "char", text });

describe("zod 桌", () => {
  it("開局只過一次 schema（非冪等 transform），開場白的指令走 zod 迴圈；續玩不重新開局", async () => {
    const { unmount } = await mount();
    const runner = zod.runners[0];
    expect(runner.calls.initialize).toBe(1);
    // 開局 計數 0→1；開場白一條指令再過一次整表 schema（上游語意）→ 2
    expect(stat(entries[0].vars)).toEqual({ 計數: 2, 燈火: "亮", 金幣: 100 });
    expect(entries[0].vars?.schema).toBe("没有用别管这个");
    const saved = mvu.forSave();
    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 5000);</UpdateVariable>")]);
    expect(stat(after[1].vars)).toEqual({ 計數: 3, 燈火: "亮", 金幣: 999 });
    await unmount();

    // 接著玩：存檔帶種子與每則的表 → 不開局、不重跑 transform
    await mount({ saved, resumed: true, start: after });
    expect(zod.runners.length === 1 || zod.runners[1].calls.initialize === 0).toBe(true);
    expect(stat(entries[1].vars)).toEqual({ 計數: 3, 燈火: "亮", 金幣: 999 });
    expect(mvu.zodFailure).toBeNull();
  });

  it("更新整次失敗：這則不寫變數、顯示可清除的提示；下一次成功就清掉", async () => {
    await mount();
    zod.failParse = 1;
    const failed = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 1);</UpdateVariable>")]);
    expect(failed[1].vars).toBeUndefined();
    await act(async () => {});
    expect(mvu.zodFailure).toBe("update");
    expect(mvu.initError).toBeNull();
    const ok = await mvu.afterReply([...entries, reply("再來。<UpdateVariable>_.set('金幣', 2);</UpdateVariable>")]);
    expect(stat(ok[1].vars)?.金幣).toBe(2);
    await act(async () => {});
    expect(mvu.zodFailure).toBeNull();
  });

  it("開局整次失敗：不掛表、提示開局失敗（不是初始值讀不懂）；下一次送出前（ready）就重試，組好的提示已經有開場白變數", async () => {
    zod.failInitialize = 1;
    await mount();
    expect(entries[0].vars).toBeUndefined();
    expect(mvu.zodFailure).toBe("init");
    expect(mvu.initError).toBeNull();
    // 玩家下一次送出：useChat 先等 ready() 才組提示
    await act(async () => {
      await mvu.ready();
    });
    // 第一次在替身外層就失敗，重試這次才真的過 schema（只過一次）
    expect(zod.runners[0].calls.initialize).toBe(1);
    expect(stat(entries[0].vars)).toEqual({ 計數: 2, 燈火: "亮", 金幣: 100 });
    expect(mvu.zodFailure).toBeNull();
    const prompt = mvu.prepareMessages([{ role: "system", content: "金幣={{get_message_variable::stat_data.金幣}}" }], entries, createChatVariables());
    expect(prompt[0].content).toBe("金幣=100");
    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 7);</UpdateVariable>")]);
    expect(stat(after[1].vars)).toEqual({ 計數: 3, 燈火: "亮", 金幣: 7 });
  });

  it("開局重試又失敗：同一輪的 afterReply 不再自動重試，這則不寫變數；再下一次送出前才再試", async () => {
    zod.failInitialize = 2;
    await mount();
    await act(async () => {
      await mvu.ready();
    });
    expect(mvu.zodFailure).toBe("init");
    zod.failInitialize = 1; // 若 afterReply 又重試就會吃掉這次失敗
    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 7);</UpdateVariable>")]);
    expect(after[1].vars).toBeUndefined();
    expect(zod.failInitialize).toBe(1);
    expect(zod.runners[0].calls.initialize).toBe(0);
  });

  it("開場白的更新整次失敗：開場那一樓不寫變數、種子停在開局值、提示開場白沒套用", async () => {
    zod.failParse = 1;
    await mount();
    expect(entries[0].vars).toBeUndefined();
    expect(mvu.zodFailure).toBe("opening");
    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 3);</UpdateVariable>")]);
    // 從開局值（計數 1）起算
    expect(stat(after[1].vars)).toEqual({ 計數: 2, 燈火: "暗", 金幣: 3 });
  });

  it("初始值沒過 schema：照原值開局（cleanUpMetadata 過）、另一句提示；不是「沒有卡片變數」", async () => {
    await mount({ card: zodCard("計數: 0\n燈火: 黃昏\n金幣: 1\n$meta:\n  extensible: false\n") });
    expect(mvu.zodInitInvalid).toBe(true);
    expect(mvu.initError).toBeNull();
    expect(zod.runners[0].calls.initialize).toBe(1);
    // 開場白那條 set 讓整表過得了 schema
    expect(stat(entries[0].vars)).toEqual({ 計數: 1, 燈火: "亮", 金幣: 1 });
  });

  it("schema 腳本 import 白名單外的模組：整次失敗、不退回 MagVarUpdate", async () => {
    const bad = SCHEMA.replace(MVU_ZOD, "https://evil.example/x.js");
    await mount({ card: zodCard(undefined, bad) });
    expect(entries[0].vars).toBeUndefined();
    expect(mvu.zodFailure).toBe("init");
    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 3);</UpdateVariable>")]);
    expect(after[1].vars).toBeUndefined();
    expect(ZodRunError).toBeDefined();
  });

  it("初始值讀不懂：沿用舊橫幅", async () => {
    await mount({ card: zodCard("金幣: [1, *沒有這個錨點") });
    expect(mvu.initError).toBe("[initvar]");
    expect(mvu.zodFailure).toBeNull();
  });

  it("逐條被擋下的回饋掛在那則回覆上，組提示時給一則 system（與桌面同一份句子、標題跟著語系）；重 Roll 掉那則就不帶", async () => {
    await mount({ start: [{ id: "e0", role: "char", text: "歡迎。<UpdateVariable>_.set('燈火', '黃昏');_.set('_秘密', 1);</UpdateVariable>", opening: true }] });
    // 開場白的指令被擋：第一輪就回饋
    expect(entries[0].mvuNotes).toEqual([
      expect.stringContaining("燈火 的 set 被卡片 schema 拒收、維持原值"),
      "_秘密 裡的「_秘密」以 _ 開頭，是唯讀欄位，這條 set 沒有套用。",
    ]);
    expect(mvu.feedback(entries)).toBe(["## 上一輪被系統擋下的更新（請照這些現值修正）", ...entries[0].mvuNotes!].join("\n"));
    setLang("en");
    try {
      expect(mvu.feedback(entries)?.split("\n")[0]).toBe("## Updates the system rejected last turn (correct them using these current values)");
    } finally {
      setLang("zh-TW");
    }
    // 回饋改由 composePrompt 算進預算，送模前的代換不再附加
    expect(mvu.prepareMessages([{ role: "system", content: "主提示" }], entries, createChatVariables())).toHaveLength(1);

    const after = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.set('金幣', 5);</UpdateVariable>")]);
    expect(after[1].mvuNotes).toBeUndefined();
    // 最後一則回覆沒被擋：不再帶開場白那一輪的回饋
    expect(mvu.feedback(after)).toBeUndefined();
    const blocked = await mvu.afterReply([...entries, reply("好的。<UpdateVariable>_.add('燈火', 1);</UpdateVariable>")]);
    expect(blocked[1].mvuNotes).toEqual([expect.stringContaining("燈火 的 add 沒有套用、維持原值")]);
  });
});

/** 卡片 iframe 的 parseMessage（card-shim-no-zod）：TableMvu.parse 收到的請求，回覆收在這裡 */
async function askParse(message: string, old: unknown): Promise<Record<string, unknown>> {
  return new Promise((resolve) => mvu.parse({ requestId: "p1", message, old: JSON.stringify(old) }, resolve));
}

describe("卡片 iframe 的 parseMessage", () => {
  it("zod 桌交這桌的 zod 引擎只算不寫（同一支 runner）；結果表照引擎鍵序", async () => {
    const { unmount } = await mount();
    const before = JSON.stringify(entries);
    const answer = await askParse("_.set('金幣', 5000);_.set('燈火', '紅');", { stat_data: { 計數: 2, 燈火: "亮", 金幣: 100 }, schema: "没有用别管这个" });
    expect(answer).toMatchObject({ kind: "mvu-parse-result", requestId: "p1", ok: true, zod: true });
    expect(answer.table).toBe('{"stat_data":{"計數":3,"燈火":"亮","金幣":999},"schema":"没有用别管这个"}');
    expect(zod.runners).toHaveLength(1);
    expect(JSON.stringify(entries)).toBe(before);
    // 整次失敗：回錯，iframe 據以 reject
    zod.failParse = 1;
    expect(await askParse("_.set('金幣', 1);", { stat_data: {} })).toMatchObject({ ok: false, error: expect.stringContaining("deadline") });
    await unmount();
  });

  it("zod 桌執行期還沒載好：回錯（not-ready），不當成不是 zod 桌；非 zod 桌回 zod:false", async () => {
    const root = createRoot(document.createElement("div"));
    function Probe() {
      mvu = useTableMvu({
        card: zodCard(),
        userName: "玩家",
        variables: createChatVariables(),
        saved: undefined,
        resumed: false,
        entriesRef: { current: [] },
        setEntries: () => {},
        substitute: (text) => text,
      });
      return null;
    }
    act(() => root.render(<Probe />));
    expect(await askParse("_.set('金幣', 1);", { stat_data: {} })).toMatchObject({ ok: false, error: expect.stringContaining("not-ready") });
    await act(async () => root.unmount());
    const plain = zodCard(undefined, "console.log('沒有 schema');");
    const { unmount } = await mount({ card: plain });
    expect(await askParse("_.set('金幣', 1);", { stat_data: {} })).toEqual({ kind: "mvu-parse-result", requestId: "p1", ok: true, zod: false });
    await unmount();
  });
});

// TestCards/ 本機限定（CI 略過）：三張真卡 17 條，網頁 zod runner 的結果表與桌面宿主、沙盒通道逐字相同（保留鍵序）。
// 基準由 src/features/card-interface/mvu/card-mvu-parse.test.ts 寫出
// 網頁測試在 web/ 底下跑（happy-dom 環境的 import.meta.url 不是檔案路徑）
const GOLDEN = resolve(process.cwd(), "../TestCards/card-parse-golden.json");
describe.skipIf(!existsSync(GOLDEN))("真卡：網頁 zod runner 與桌面宿主逐字相同（TestCards 本機限定）", () => {
  it("17 條", async () => {
    const golden = JSON.parse(readFileSync(GOLDEN, "utf8")) as { name: string; schema: string; old: string; message: string; table: string }[];
    expect(golden).toHaveLength(17);
    for (const item of golden) {
      const runner = createTestZodRunner(item.schema);
      const result = await runner.parse(item.message, JSON.parse(item.old) as Record<string, unknown>, { user: "玩家", char: null });
      expect(JSON.stringify(result.table), `${item.name} ${item.message}`).toBe(item.table);
    }
    // 每條各起一支 QuickJS（每支開機約 0.3 秒）
  }, 60_000);
});

