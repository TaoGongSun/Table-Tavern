// 沙盒的 Mvu.parseMessage 走宿主（card-shim-no-zod）：zod 桌交宿主的 zod 引擎只算不寫、非 zod 桌照舊自己跑核心；
// 判定失敗與逾時 reject 不退回 MagVarUpdate；補發的通知事件；真卡 17 條與宿主引擎的結果表逐字相同（保留鍵序）。
import { writeFileSync } from "node:fs";
import { z } from "zod";
import { afterEach, describe, expect, it, vi } from "vitest";
import { buildMvuShimSource } from "./card-mvu-shim-source";
import { type CardMvu } from "./card-mvu-shim";
import { runEval } from "../../mvu-engine/value-parser";
import { createDesktopEngine } from "../../mvu-engine/quickjs-entry";
import { REAL_CARDS, cardsDir, hasRealCards, realCard } from "../../mvu-engine/fixtures/real-cards";

const lodashName = "lodash";
const lodash: unknown = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

type Table = Record<string, unknown>;
type Posted = { kind: string; requestId: string; message?: string; old?: string; op?: "value" | "patch"; text?: string };
type Listener = (event: { source: unknown; data: unknown }) => void;
interface Sandbox {
  eventOn: (name: string, fn: (...args: unknown[]) => unknown) => void;
  Mvu: { events: Record<string, string>; parseMessage: (message: unknown, old: unknown) => Promise<Table> };
}

const snapshot: CardMvu = {
  currentId: 0,
  latestId: 0,
  states: [{ stat_data: { 金: 1 } }],
  floorState: [0],
  targets: [{ key: "e0", rev: "r0" }],
  active: true,
  generation: 1,
  scene: 0,
  layers: {},
  characterId: "c1",
  macros: { user: "玩家", char: null },
};

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** 假宿主：`answer` 收到 mvu-parse 時回什麼（null＝先不回，留在 held 裡）；值解析一律同進程跑 */
function sandbox(answer: (request: Posted) => Promise<Table | null> | Table | null) {
  const posted: Posted[] = [];
  const listeners: Listener[] = [];
  const held: Posted[] = [];
  const deliver = (data: Table) => {
    for (const listener of listeners) listener({ source: parent, data: { source: "table-tavern-host", token: "tok", ...data } });
  };
  const parent = {
    postMessage: (message: Posted) => {
      posted.push(message);
      if (message.kind === "mvu-eval") {
        queueMicrotask(() => deliver({ kind: "mvu-eval-result", requestId: message.requestId, ...runEval(message.op!, message.text!) }));
      } else if (message.kind === "mvu-parse") {
        void Promise.resolve(answer(message)).then((reply) => {
          if (reply === null) held.push(message);
          else deliver({ kind: "mvu-parse-result", requestId: message.requestId, ...reply });
        });
      }
    },
  };
  const win = {
    parent,
    _: lodash,
    addEventListener(type: string, fn: Listener) {
      if (type === "message") listeners.push(fn);
    },
  };
  new Function("window", buildMvuShimSource(snapshot, "tok"))(win);
  return {
    win: win as unknown as Sandbox,
    posted,
    held,
    reply: (request: Posted, data: Table) => deliver({ kind: "mvu-parse-result", requestId: request.requestId, ...data }),
  };
}

const parses = (posted: Posted[]) => posted.filter((message) => message.kind === "mvu-parse");

afterEach(() => {
  vi.useRealTimers();
});

describe("沙盒 parseMessage 走宿主", () => {
  it("宿主回 zod:false（確認不是 zod 桌）：照舊在沙盒跑核心，值解析走 Worker", async () => {
    const { win, posted } = sandbox(() => ({ ok: true, zod: false }));
    const result = await win.Mvu.parseMessage("_.set('金', 2);", { stat_data: { 金: 1 } });
    expect((result.stat_data as Table).金).toBe(2);
    expect(parses(posted)).toHaveLength(1);
    expect(parses(posted)[0]).toMatchObject({ message: "_.set('金', 2);", old: '{"stat_data":{"金":1}}' });
    expect(posted.some((message) => message.kind === "mvu-eval")).toBe(true);
  });

  it("zod 桌：用宿主回的表（保留鍵序）；補發 STARTED（舊表）→ ENDED → ENDED_for_zod（最終表, 舊表），都是拷貝", async () => {
    const { win, posted } = sandbox(() => ({ ok: true, zod: true, table: '{"stat_data":{"乙":2,"甲":100},"schema":"x"}' }));
    const seen: [string, unknown[]][] = [];
    for (const name of ["mag_variable_update_started", "mag_variable_update_ended", "mag_variable_update_ended_for_zod", "mag_command_parsed", "mag_variable_updated"]) {
      win.eventOn(name, (...args: unknown[]) => {
        seen.push([name, JSON.parse(JSON.stringify(args)) as unknown[]]);
        // 監聽器改不到結果：兩個參數都改
        for (const arg of args) (arg as Table).stat_data = "被改";
      });
    }
    const result = await win.Mvu.parseMessage("_.set('甲', 500);", { stat_data: { 乙: 2, 甲: 1 } });
    expect(JSON.stringify(result)).toBe('{"stat_data":{"乙":2,"甲":100},"schema":"x"}');
    const final = { stat_data: { 乙: 2, 甲: 100 }, schema: "x" };
    // 同核心：STARTED 帶更新前的表、ENDED 系列帶 (新表, 舊表)：監聽器讀得到 parse 前的值；前一支改過的參數也不會傳到下一支
    const before = { stat_data: { 乙: 2, 甲: 1 } };
    expect(seen).toEqual([
      ["mag_variable_update_started", [before]],
      ["mag_variable_update_ended", [final, before]],
      ["mag_variable_update_ended_for_zod", [final, before]],
    ]);
    // 沒有在沙盒跑核心
    expect(posted.some((message) => message.kind === "mvu-eval")).toBe(false);
  });

  it("同一事件掛兩支監聽器：第一支改了兩個參數，第二支拿到的仍是原值", async () => {
    const { win } = sandbox(() => ({ ok: true, zod: true, table: '{"stat_data":{"甲":100}}' }));
    const second: unknown[][] = [];
    win.eventOn("mag_variable_update_ended", (after: unknown, before: unknown) => {
      (after as Table).stat_data = "被改";
      ((before as Table).stat_data as Table).甲 = "被改";
    });
    win.eventOn("mag_variable_update_ended", (...args: unknown[]) => {
      second.push(JSON.parse(JSON.stringify(args)) as unknown[]);
    });
    const result = await win.Mvu.parseMessage("_.set('甲', 500);", { stat_data: { 甲: 1 } });
    expect(second).toEqual([[{ stat_data: { 甲: 100 } }, { stat_data: { 甲: 1 } }]]);
    expect(result).toEqual({ stat_data: { 甲: 100 } });
  });

  it("宿主回錯（判定失敗、還沒載好、整次失敗）：reject，不退回 MagVarUpdate 在沙盒跑", async () => {
    const { win, posted } = sandbox(() => ({ ok: false, error: "detect: 讀不到桌" }));
    await expect(win.Mvu.parseMessage("_.set('金', 2);", { stat_data: { 金: 1 } })).rejects.toMatchObject({
      code: "host-failed",
      message: expect.stringContaining("detect: 讀不到桌"),
    });
    expect(posted.some((message) => message.kind === "mvu-eval")).toBe(false);
    const broken = sandbox(() => ({ ok: true, zod: true, table: "[1]" }));
    await expect(broken.win.Mvu.parseMessage("x", {})).rejects.toMatchObject({ code: "host-failed" });
  });

  it("送宿主前先擋：參數型別、訊息與整表上限（超過不送）", async () => {
    const { win, posted } = sandbox(() => ({ ok: true, zod: false }));
    await expect(win.Mvu.parseMessage(1, {})).rejects.toThrow(/message 必須是字串/);
    await expect(win.Mvu.parseMessage("x", [])).rejects.toThrow(/old_data 必須是物件/);
    await expect(win.Mvu.parseMessage("x".repeat(256 * 1024 + 1), {})).rejects.toMatchObject({ code: "message-too-large" });
    await expect(win.Mvu.parseMessage("x", { big: "字".repeat(700 * 1024) })).rejects.toMatchObject({ code: "too-large" });
    expect(parses(posted)).toHaveLength(0);
  });

  it("同一支 iframe 同時只送一筆：第二筆等第一筆回來才送", async () => {
    const { win, posted, held, reply } = sandbox(() => null);
    const first = win.Mvu.parseMessage("_.set('金', 2);", { stat_data: { 金: 1 } });
    const second = win.Mvu.parseMessage("_.set('金', 3);", { stat_data: { 金: 1 } });
    await flush();
    expect(parses(posted)).toHaveLength(1);
    reply(held[0], { ok: true, zod: true, table: '{"stat_data":{"金":2}}' });
    await expect(first).resolves.toEqual({ stat_data: { 金: 2 } });
    await flush();
    expect(parses(posted)).toHaveLength(2);
    reply(held[1], { ok: true, zod: true, table: '{"stat_data":{"金":3}}' });
    await expect(second).resolves.toEqual({ stat_data: { 金: 3 } });
  });

  it("ENDED 監聽器裡 await 下一次 parseMessage 不會互等（先放名額再補事件）", async () => {
    let count = 0;
    const { win } = sandbox(() => {
      count += 1;
      return { ok: true, zod: true, table: JSON.stringify({ stat_data: { 次: count } }) };
    });
    let inner: Table | null = null;
    let nested = false;
    win.eventOn("mag_variable_update_ended", async () => {
      if (nested) return;
      nested = true;
      inner = await win.Mvu.parseMessage("_.set('次', 0);", {});
    });
    const outer = await win.Mvu.parseMessage("_.set('次', 0);", {});
    expect(outer).toEqual({ stat_data: { 次: 1 } });
    expect(inner).toEqual({ stat_data: { 次: 2 } });
  });

  it("宿主 10 秒沒回：reject（逾時）；之後晚到的結果不認，下一筆照常", async () => {
    vi.useFakeTimers();
    const { win, posted, held, reply } = sandbox(() => null);
    const pending = win.Mvu.parseMessage("_.set('金', 2);", { stat_data: { 金: 1 } });
    const caught = pending.catch((error: unknown) => error);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(await caught).toMatchObject({ code: "timeout" });
    const late = held[0];
    const next = win.Mvu.parseMessage("_.set('金', 3);", { stat_data: { 金: 1 } });
    await vi.advanceTimersByTimeAsync(0);
    expect(parses(posted)).toHaveLength(2);
    // 晚到的舊回覆不會被當成新請求的結果
    reply(late, { ok: true, zod: true, table: '{"stat_data":{"金":"舊"}}' });
    reply(held[1], { ok: true, zod: true, table: '{"stat_data":{"金":3}}' });
    await expect(next).resolves.toEqual({ stat_data: { 金: 3 } });
  });
});

/** 卡片 schema 腳本的測試改寫（同 quickjs-entry.test.ts）：import 換成參數、拿掉 export */
function loadSchema(register: (input: unknown) => void, source: string) {
  const pattern = /^import \{ registerMvuSchema(?: as (\w+))? \} from '[^']+';/m;
  const alias = pattern.exec(source)?.[1] ?? "registerMvuSchema";
  const body = source.replace(pattern, "").replace(/export const/g, "const");
  new Function("z", "_", "$", alias, body)(z, lodash, (fn: () => void) => fn(), register);
}

const MACROS = { user: "玩家", char: null };

// TestCards/ 本機限定（CI 略過）。三張真卡 17 條指令逐條：沙盒經 mvu-parse 通道拿到的表與宿主引擎的表逐字相同（保留鍵序）。
// 案例與預期寫進 TestCards/card-parse-golden.json，cargo（QuickJS run_batch）與網頁 zod runner 拿同一份比
describe.skipIf(!hasRealCards)("真卡：沙盒 parseMessage 與宿主 zod 引擎逐字相同（TestCards 本機限定）", () => {
  it("17 條", async () => {
    const golden = [];
    for (const { file, message } of REAL_CARDS) {
      const card = realCard(file);
      const engine = createDesktopEngine();
      loadSchema(engine.register, card.schema);
      const init = JSON.parse(engine.initialize(JSON.stringify(card.initvar))) as { stat_data: Table };
      const old = JSON.stringify({ stat_data: init.stat_data, schema: "没有用别管这个", display_data: {}, delta_data: {} });
      const commands = message(init.stat_data)
        .split(/;(?=_\.)/)
        .map((command) => (command.endsWith(";") ? command : `${command};`));
      // 宿主：同 Rust card_parse，結果表照引擎輸出的鍵序序列化
      const host = async (request: Posted) => {
        const detailed = JSON.parse(await engine.parse(request.message!, request.old!, JSON.stringify(MACROS))) as { table: Table };
        return { ok: true, zod: true, table: JSON.stringify(detailed.table) };
      };
      const { win } = sandbox(host);
      for (const command of commands) {
        const expected = (await host({ kind: "mvu-parse", requestId: "x", message: command, old })).table;
        const got = await win.Mvu.parseMessage(command, JSON.parse(old) as Table);
        expect(JSON.stringify(got), `${file} ${command}`).toBe(expected);
        golden.push({ name: file, schema: card.schema, old, message: command, table: expected });
      }
    }
    expect(golden).toHaveLength(17);
    writeFileSync(new URL("card-parse-golden.json", cardsDir), JSON.stringify(golden));
  });
});
