// Mvu.parseMessage（包 2c，計畫 8.9）：沙盒端的指令抽取、套用、schema、事件分階段與上限。值解析走真的引擎
// （card-mvu-parse-engine.ts），只是把宿主的 Worker 換成同進程呼叫。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { buildMvuShimSource } from "./card-mvu-shim-source";
import { runEval } from "./card-mvu-parse-engine";
import { type CardMvu } from "./card-mvu-shim";
import replaceParity from "../../../shared/contracts/mvu-replace-parity.json";

const lodashName = "lodash";
const lodash: unknown = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

type Table = Record<string, any>;
type Listener = (event: { source: unknown; data: unknown }) => void;
interface Posted {
  kind: string;
  op?: string;
  text?: string;
  requestId?: string;
}
interface Sandbox {
  eventOn: (name: string, fn: (...args: any[]) => unknown) => void;
  Mvu: {
    events: Record<string, string>;
    parseMessage: (message: string, old: unknown) => Promise<Table>;
    setMvuVariable: (data: unknown, path: string, value: unknown, option?: unknown) => Promise<boolean>;
  };
}

// 套用失敗與 schema 型別不符都會 console.warn／error（照上游）；測試裡不要洗版
beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "error").mockImplementation(() => {});
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const snapshot: CardMvu = {
  currentId: 0,
  latestId: 0,
  states: [{ stat_data: { 金: 1 } }],
  floorState: [0],
  targets: [{ key: "e0", rev: "r0" }],
  active: true,
  generation: 3,
  scene: 0,
  layers: {},
  characterId: "c1",
  macros: { user: "玩家", char: "旅人" },
};

/** answer：宿主怎麼回值解析請求；預設用真的引擎，傳函式可改寫（回 null＝不回應） */
function sandbox(answer?: (op: string, text: string) => Record<string, unknown> | null, mvu: CardMvu = snapshot) {
  const posted: Posted[] = [];
  const listeners: Listener[] = [];
  const parent = {
    postMessage: (message: Posted) => {
      posted.push(message);
      if (message.kind !== "mvu-eval") return;
      const reply = answer ? answer(message.op!, message.text!) : runEval(message.op as "value" | "patch", message.text!);
      if (reply === null) return;
      void Promise.resolve().then(() => {
        for (const listener of listeners) {
          listener({
            source: parent,
            data: { source: "table-tavern-host", token: "tok", kind: "mvu-eval-result", requestId: message.requestId, ...reply },
          });
        }
      });
    },
  };
  const win = {
    parent,
    _: lodash,
    addEventListener(type: string, fn: Listener) {
      if (type === "message") listeners.push(fn);
    },
  };
  new Function("window", buildMvuShimSource(mvu, "tok"))(win);
  return {
    win: win as unknown as Sandbox,
    posted,
    deliver: async (data: Record<string, unknown>) => {
      for (const listener of listeners) listener({ source: parent, data: { source: "table-tavern-host", token: "tok", ...data } });
      await flush();
      await flush();
    },
  };
}

/** 照 MVU 初始化的樣子替資料生一份 schema：物件與陣列都可擴充（真資料的 schema 由 initvar 產生） */
function autoSchema(data: unknown): Table {
  if (Array.isArray(data)) {
    return { type: "array", extensible: true, recursiveExtensible: true, elementType: data.length > 0 ? autoSchema(data[0]) : { type: "any" } };
  }
  if (data !== null && typeof data === "object") {
    const properties: Table = {};
    for (const [key, value] of Object.entries(data)) properties[key] = { ...autoSchema(value), required: false };
    return { type: "object", extensible: true, recursiveExtensible: true, properties };
  }
  return { type: typeof data === "string" || typeof data === "number" || typeof data === "boolean" ? typeof data : "any" };
}
const withSchema = (stat_data: Table): Table => ({ stat_data, schema: autoSchema(stat_data) });

const base = (): Table => ({
  initialized_lorebooks: {},
  ...withSchema({
    金: 10,
    名: "阿明",
    屬性: [5, "說明"],
    空: null,
    物品: ["劍"],
    角色: { 甲: { 好感: 3 }, 乙: { 好感: 4 } },
    時間: "2024-01-01T00:00:00.000Z",
  }),
});

describe("parseMessage：set", () => {
  it("字串轉成數字欄位的 Number；display／delta 是「舊->新 (理由)」；回新資料、不改傳入的 old", async () => {
    const { win } = sandbox();
    const old = base();
    const result = await win.Mvu.parseMessage("<UpdateVariable>\n_.set('金', '25');//買東西\n</UpdateVariable>", old);
    expect(result.stat_data.金).toBe(25);
    expect(result.display_data.金).toBe("10->25 (買東西)");
    expect(result.delta_data).toEqual({ 金: "10->25 (買東西)" });
    expect(result.display_data.名).toBe("阿明");
    expect(result.stat_data.$internal).toBeUndefined();
    expect(old.stat_data.金).toBe(10);
  });

  it("[值, 說明]：改第 0 項；舊值是數字時新值轉 Number、null 例外；三參數取最後一個當新值", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage("_.set('屬性', 5, '7');//升級", base());
    expect(result.stat_data.屬性).toEqual([7, "說明"]);
    expect(result.display_data.屬性).toBe("5->7 (升級)");
    const cleared = await win.Mvu.parseMessage("_.set('屬性', null);", base());
    expect(cleared.stat_data.屬性).toEqual([null, "說明"]);
    const text = await win.Mvu.parseMessage('_.set("名", "新名");', base());
    expect(text.stat_data.名).toBe("新名");
    expect(text.display_data.名).toBe("阿明->新名 ");
  });

  it("路徑不存在不套用；舊值是 null 的欄位不轉型；物件與數學式", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage(
      "_.set('沒有', 1);_.set('空', 5);_.set('角色.甲', {好感: 9,});_.set('金', 10 + 2 * 3);",
      base(),
    );
    expect(result.stat_data.沒有).toBeUndefined();
    expect(result.stat_data.空).toBe(5);
    expect(result.stat_data.角色.甲).toEqual({ 好感: 9 });
    expect(result.stat_data.金).toBe(16);
  });

  it("set 到非數字字串變 NaN：照上游留到整批結束、序列化前轉 null；其他來源的非有限數照舊整批拒絕", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage("_.set('金', 'abc');_.set('屬性', 'abc');", base());
    expect(result.stat_data.金).toBeNull();
    expect(result.stat_data.屬性).toEqual([null, "說明"]);
    // 同一批後面再 set 數字字串：舊值仍是 NaN（number），照樣轉數字
    expect((await win.Mvu.parseMessage("_.set('金', 'abc');_.set('金', '7');", base())).stat_data.金).toBe(7);
    await expect(
      win.Mvu.parseMessage("_.add('金', 1.7e308);_.add('金', 1.7e308);", base()),
    ).rejects.toMatchObject({ code: "non-finite" });
  });

  it("非有限數標記依最後一次寫入的來源：被別的指令改寫、父欄覆蓋、刪除後就失效，兄弟欄的寫入不影響", async () => {
    const { win } = sandbox();
    const nonFinite = { code: "non-finite" };
    // set 出 NaN → set 有限值 → add 出 Infinity：最後寫入的是 add，整批拒收
    await expect(
      win.Mvu.parseMessage("_.set('金', '很多');_.set('金', 1e308);_.add('金', 1e308);", base()),
    ).rejects.toMatchObject(nonFinite);
    // 父欄 set 覆蓋子欄 → 子欄再被 add 成 Infinity
    const nested = () => withSchema({ P: { a: 1, b: 2 } });
    await expect(
      win.Mvu.parseMessage("_.set('P.a', '很多');_.set('P', {\"a\": 1, \"b\": 2});_.add('P.a', 1e308);_.add('P.a', 1e308);", nested()),
    ).rejects.toMatchObject(nonFinite);
    // 刪除後同一處再插入 Infinity
    await expect(
      win.Mvu.parseMessage("_.set('P.a', '很多');_.delete('P.a');_.insert('P', 'a', 1e999);", nested()),
    ).rejects.toMatchObject(nonFinite);
    // 只動兄弟欄：set 產生的 NaN 照樣在序列化時轉 null
    const sibling = await win.Mvu.parseMessage("_.set('P.a', '很多');_.insert('P', 'c', 3);_.add('P.b', 1);", nested());
    expect(sibling.stat_data.P).toEqual({ a: null, b: 3, c: 3 });
    // 巢狀合併：只撤銷 deep merge 實際覆寫的位置，沒被動到的兄弟欄保留標記
    const deep = () => withSchema({ P: { child: { n: 10, x: 0 } } });
    const merged = await win.Mvu.parseMessage("_.set('P.child.n', '很多');_.insert('P', {\"child\": {\"x\": 1}});", deep());
    expect(merged.stat_data.P).toEqual({ child: { n: null, x: 1 } });
    // 合併真的覆寫到那一格，之後再 add 出 Infinity：整批拒收
    await expect(
      win.Mvu.parseMessage(
        "_.set('P.child.n', '很多');_.insert('P', {\"child\": {\"n\": 1e308}});_.add('P.child.n', 1e308);",
        deep(),
      ),
    ).rejects.toMatchObject(nonFinite);
  });

  it("路徑修正：帶引號與空白的點分欄位、括號裡的裸數字是索引", async () => {
    const { win } = sandbox();
    const old = withSchema({ a: { "b c": { d: 1 } }, 列: [1, 2, 3], 字: { "1": "x" } });
    const result = await win.Mvu.parseMessage("_.set('a.\"b c\".d', 5);_.set('列[1]', 9);_.set('字[\"1\"]', 'y');", old);
    expect(result.stat_data.a["b c"].d).toBe(5);
    expect(result.stat_data.列).toEqual([1, 9, 3]);
    expect(result.stat_data.字["1"]).toBe("y");
  });
});

describe("parseMessage：insert／assign／delete／add", () => {
  it("assign 到陣列追加、指定位置；到物件合併與指定鍵；不存在的路徑建立", async () => {
    const { win } = sandbox();
    const old = withSchema({ 物品: ["劍"], 角色: { 甲: 1 }, 空: null });
    const result = await win.Mvu.parseMessage(
      "_.assign('物品', '盾');_.insert('物品', 0, '帽');_.assign('角色', {乙: 2});_.assign('角色', '丙', 3);_.assign('空', 'k', 1);",
      old,
    );
    expect(result.stat_data.物品).toEqual(["帽", "劍", "盾"]);
    expect(result.stat_data.角色).toEqual({ 甲: 1, 乙: 2, 丙: 3 });
    // 只有目標是 null 時才會建立新物件（目標不存在會被當成「原始型別」擋掉，同上游）
    expect(result.stat_data.空).toEqual({ k: 1 });
  });

  it("insert 的目標是原始型別、把陣列合併進物件：整條略過", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage("_.assign('金', 1);_.assign('角色', [1]);_.assign('不存在', 1);", base());
    expect(result.stat_data.金).toBe(10);
    expect("不存在" in result.stat_data).toBe(false);
    expect(result.stat_data.角色.甲).toEqual({ 好感: 3 });
  });

  it("schema 規則：不可擴充物件擋合併與未知鍵、不可擴充陣列擋插入、$meta.extensible 放行", async () => {
    const { win } = sandbox();
    const closed = {
      stat_data: { 角色: { 甲: 1 }, 列: [1] },
      schema: {
        type: "object",
        properties: {
          角色: { type: "object", extensible: false, properties: { 甲: { type: "number", required: true } } },
          列: { type: "array", extensible: false, elementType: { type: "number" } },
        },
      },
    };
    const blocked = await win.Mvu.parseMessage("_.assign('角色', {乙: 1});_.assign('角色', '丙', 1);_.assign('列', 2);", closed);
    expect(blocked.stat_data).toEqual({ 角色: { 甲: 1 }, 列: [1] });
    // 沒有 schema 時：新鍵插在單層路徑下會因「找不到父路徑」被擋（上游的行為），有 schema 才放行
    const bare = await win.Mvu.parseMessage("_.assign('開', 'k', 1);", { stat_data: { 開: {} } });
    expect(bare.stat_data.開).toEqual({});
  });

  it("模板：assign 新元素時套 $meta.template，值的屬性優先；陣列模板用 $arrayMeta 元素宣告", async () => {
    const { win } = sandbox();
    const old = {
      stat_data: { 隊伍: {}, 列: [] },
      schema: {
        type: "object",
        properties: {
          隊伍: { type: "object", extensible: true, template: { hp: 10, 名: "?" }, properties: {} },
          列: { type: "array", extensible: true, template: ["預設"], elementType: { type: "any" } },
        },
      },
    };
    const first = await win.Mvu.parseMessage("_.assign('隊伍', '甲', {名: '阿甲'});_.assign('列', ['a']);", old);
    expect(first.stat_data.隊伍).toEqual({ 甲: { hp: 10, 名: "阿甲" } });
    expect(first.stat_data.列).toEqual([["a", "預設"]]);
  });

  it("delete：路徑、陣列索引（兩種寫法）、鍵、值；必填鍵與不可擴充陣列擋下", async () => {
    const { win } = sandbox();
    const old = withSchema({ a: 1, 列: ["x", "y", "z"], 物: { k: 1, j: 2 }, 巢: { m: [1, 2] } });
    const result = await win.Mvu.parseMessage(
      "_.remove('a');_.delete('列[0]');_.unset('列', 'z');_.remove('物', 'k');_.remove('巢.m', 1);_.remove('沒有');",
      old,
    );
    // 單一數字參數是索引（不是值）：m 移掉第 1 項
    expect(result.stat_data).toEqual({ 列: ["y"], 物: { j: 2 }, 巢: { m: [1] } });
    const locked = {
      stat_data: { 物: { k: 1 }, 列: [1, 2] },
      schema: {
        type: "object",
        properties: {
          物: { type: "object", properties: { k: { type: "number", required: true } } },
          列: { type: "array", extensible: false, elementType: { type: "number" } },
        },
      },
    };
    const kept = await win.Mvu.parseMessage("_.remove('物', 'k');_.remove('列', 0);", locked);
    expect(kept.stat_data).toEqual({ 物: { k: 1 }, 列: [1, 2] });
  });

  it("add：數字、[值, 說明]、日期（毫秒）；非數字增量與不支援的值略過", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage(
      "_.add('金', 0.2);_.add('屬性', -1);//扣;\n_.add('時間', 3600000);_.add('名', 1);_.add('金', 'x');",
      base(),
    );
    expect(result.stat_data.金).toBe(10.2);
    expect(result.stat_data.屬性).toEqual([4, "說明"]);
    expect(result.stat_data.時間).toBe("2024-01-01T01:00:00.000Z");
    expect(result.stat_data.名).toBe("阿明");
    expect(result.display_data.屬性).toBe("5->4 (扣;)");
  });

  it("參數個數不合的指令不算指令；括號配對與引號內的括號、分號", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage(
      "_.set('名');_.add('金');_.set('名', \"a);_.set('金', 99);b\");_.set('金', 5)",
      base(),
    );
    expect(result.stat_data.名).toBe("a);_.set('金', 99);b");
    // 最後一條沒有分號：不算
    expect(result.stat_data.金).toBe(10);
  });
});

describe("parseMessage：JSONPatch", () => {
  it("replace／add（含 -）／remove／delta；move 照上游沒有對應的套用；與一般指令依出現位置排序", async () => {
    const { win } = sandbox();
    const message = [
      "<UpdateVariable>",
      "_.set('名', '先');",
      "<JSONPatch>",
      '[{"op":"replace","path":"/金","value":20},{"op":"add","path":"/物品/-","value":"盾"},{"op":"remove","path":"/空"},',
      '{"op":"delta","path":"/金","value":5},{"op":"move","from":"/名","path":"/新名"}]',
      "</JSONPatch>",
      "_.set('名', '後');",
      "</UpdateVariable>",
    ].join("\n");
    const result = await win.Mvu.parseMessage(message, base());
    expect(result.stat_data.金).toBe(25);
    expect(result.stat_data.物品).toEqual(["劍", "盾"]);
    expect("空" in result.stat_data).toBe(false);
    expect(result.stat_data.名).toBe("後");
    expect(result.stat_data.新名).toBeUndefined();
    expect(result.display_data.金).toBe("20->25 (json_patch)");
  });

  it("YAML 寫法、程式碼圍欄、損壞的區塊略過", async () => {
    const { win } = sandbox();
    const yamlBlock = "<json_patch>\n```json\n- op: replace\n  path: /金\n  value: 7\n```\n</json_patch>";
    expect((await win.Mvu.parseMessage(yamlBlock, base())).stat_data.金).toBe(7);
    const bad = "<JSONPatch>{{{{ 不是 patch </JSONPatch><JSONPatch>[{\"op\":\"nope\"}]</JSONPatch>";
    expect((await win.Mvu.parseMessage(bad, base())).stat_data).toEqual(base().stat_data);
  });

  it("路徑含點、括號、~ 轉義時照 JSON Pointer 原樣，不被路徑修正改壞", async () => {
    const { win } = sandbox();
    const old = withSchema({ "a.b": { "c/d": 1 }, "e~f": 1 });
    const patch = '<JSONPatch>[{"op":"replace","path":"/a.b/c~1d","value":2},{"op":"replace","path":"/e~0f","value":3}]</JSONPatch>';
    const result = await win.Mvu.parseMessage(patch, old);
    expect(result.stat_data).toEqual({ "a.b": { "c/d": 2 }, "e~f": 3 });
  });
});

describe("parseMessage：巨集與值解析", () => {
  it("{{user}}／{{char}} 先代換再抽指令", async () => {
    const { win } = sandbox();
    const result = await win.Mvu.parseMessage("_.set('名', '{{user}}遇見{{char}}');", base());
    expect(result.stat_data.名).toBe("玩家遇見旅人");
  });

  it("值解析逐條送宿主（op=value），不送寫入", async () => {
    const { win, posted } = sandbox();
    await win.Mvu.parseMessage("_.set('金', 1);_.set('名', 'x');", base());
    expect(posted.map((message) => message.kind)).toEqual(["mvu-eval", "mvu-eval"]);
    expect(posted.every((message) => message.op === "value")).toBe(true);
  });

  it("宿主回錯（逾時、超限）算那一條命令失敗，其他條照常", async () => {
    const { win } = sandbox((op, text) => (text === "999" ? { ok: false, error: "timeout" } : runEval(op as "value", text)));
    const result = await win.Mvu.parseMessage("_.set('金', 999);_.set('名', 'ok');", base());
    expect(result.stat_data.金).toBe(10);
    expect(result.stat_data.名).toBe("ok");
  });
});

describe("parseMessage：事件分階段", () => {
  it("順序與參數：STARTED → COMMAND_PARSED（含兩個 _for_zod）→ 逐條 SINGLE → ENDED → ENDED_for_zod；沒有 BEFORE_MESSAGE_UPDATE", async () => {
    const { win } = sandbox();
    const E = win.Mvu.events;
    const seen: string[] = [];
    const args: Record<string, any[]> = {};
    for (const name of [
      E.VARIABLE_UPDATE_STARTED,
      E.COMMAND_PARSED,
      `${E.COMMAND_PARSED}_for_zod`,
      `${E.COMMAND_PARSED}_ended_for_zod`,
      E.SINGLE_VARIABLE_UPDATED,
      E.VARIABLE_UPDATE_ENDED,
      `${E.VARIABLE_UPDATE_ENDED}_for_zod`,
      E.BEFORE_MESSAGE_UPDATE,
    ]) {
      win.eventOn(name, (...a: any[]) => {
        seen.push(name);
        args[name] = a;
        if (name === E.VARIABLE_UPDATE_STARTED) expect(Object.keys(a[0].stat_data.$internal)).toEqual(["display_data", "delta_data"]);
        if (name === E.VARIABLE_UPDATE_ENDED) expect(a[0].stat_data.$internal).toBeDefined();
        if (name === `${E.VARIABLE_UPDATE_ENDED}_for_zod`) expect(a[0].stat_data.$internal).toBeUndefined();
      });
    }
    const message = "_.set('金', 11);_.assign('物品', '盾');";
    const result = await win.Mvu.parseMessage(message, base());
    expect(seen).toEqual([
      E.VARIABLE_UPDATE_STARTED,
      E.COMMAND_PARSED,
      `${E.COMMAND_PARSED}_for_zod`,
      `${E.COMMAND_PARSED}_ended_for_zod`,
      E.SINGLE_VARIABLE_UPDATED,
      E.SINGLE_VARIABLE_UPDATED,
      E.VARIABLE_UPDATE_ENDED,
      `${E.VARIABLE_UPDATE_ENDED}_for_zod`,
    ]);
    expect(args[E.COMMAND_PARSED][0]).toBe(args[E.VARIABLE_UPDATE_STARTED][0]);
    expect(args[E.COMMAND_PARSED][1].map((c: Table) => [c.type, c.args[0]])).toEqual([["set", "金"], ["insert", "物品"]]);
    expect(args[E.COMMAND_PARSED][2]).toBe(message);
    // SINGLE：(stat_data, path, 舊, 新)；ENDED：(新資料, 更新前)
    expect(args[E.SINGLE_VARIABLE_UPDATED].slice(1)).toEqual(["物品", ["劍"], ["劍", "盾"]]);
    expect(args[E.VARIABLE_UPDATE_ENDED][1].stat_data.金).toBe(10);
    expect(args[E.VARIABLE_UPDATE_ENDED][0]).toBe(result);
  });

  it("COMMAND_PARSED 的監聽器可以改、加、刪指令（非字串參數原樣放行）", async () => {
    const { win } = sandbox();
    win.eventOn(win.Mvu.events.COMMAND_PARSED, (_vars: unknown, commands: Table[]) => {
      commands[0].args[0] = "名";
      commands.push({ type: "set", full_match: "_.set(...)", args: ["金", 77], reason: "腳本" });
      commands.push({ type: "remove", full_match: "x", args: ["空"], reason: "" });
    });
    const result = await win.Mvu.parseMessage("_.set('金', 1);", base());
    expect(result.stat_data.名).toBe(1);
    expect(result.stat_data.金).toBe(77);
    expect("空" in result.stat_data).toBe(false);
    expect(result.display_data.金).toBe("10->77 (腳本)");
  });

  it("STARTED／SINGLE／ENDED 的監聽器改資料會影響後續；ENDED 的修改進結果與 schema 調和", async () => {
    const { win } = sandbox();
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, (vars: Table) => {
      vars.stat_data.金 = 100;
    });
    win.eventOn(win.Mvu.events.SINGLE_VARIABLE_UPDATED, (stat: Table, path: string) => {
      if (path === "金") stat.名 = "被改";
    });
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, (vars: Table) => {
      if (vars.stat_data.金 > 50) vars.stat_data.金 = 50;
      vars.stat_data.新欄 = 1;
    });
    const result = await win.Mvu.parseMessage("_.add('金', 5);", base());
    expect(result.stat_data.金).toBe(50);
    expect(result.stat_data.名).toBe("被改");
    expect(result.display_data.金).toBe("100->105 ");
    expect(result.schema.properties.新欄).toMatchObject({ type: "number" });
  });

  it("監聽器丟錯不影響其他監聽器與後續流程", async () => {
    const { win } = sandbox();
    const ok = vi.fn();
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, () => {
      throw new Error("壞監聽器");
    });
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, ok);
    const result = await win.Mvu.parseMessage("_.set('金', 2);", base());
    expect(ok).toHaveBeenCalled();
    expect(result.stat_data.金).toBe(2);
  });

  it("監聽器裡再呼叫 parseMessage 不會卡死（值解析結果不排進序列佇列）", async () => {
    const { win, deliver } = sandbox();
    let inner: Table | undefined;
    let entered = false;
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, async () => {
      if (entered) return;
      entered = true;
      inner = await win.Mvu.parseMessage("_.set('金', 3);", base());
    });
    // 推一份資料變了的快照，ENDED 監聽器就在序列佇列裡跑
    await deliver({ kind: "chat", mvu: { ...snapshot, states: [{ stat_data: { 金: 2 } }] } });
    expect(inner?.stat_data.金).toBe(3);
  });
});

describe("parseMessage：schema 調和", () => {
  it("資料有變動才調和：用目前資料重生 schema、保留旗標；沒變動 schema 原樣", async () => {
    const { win } = sandbox();
    const old = { stat_data: { 金: 1, 列: [1], $meta: { strictSet: true } }, schema: { type: "object", properties: {}, strictTemplate: true } };
    const same = await win.Mvu.parseMessage("_.set('金', 1);", old);
    expect(same.schema).toEqual(old.schema);
    const changed = await win.Mvu.parseMessage("_.set('金', 2);", old);
    expect(changed.schema).toMatchObject({ type: "object", strictTemplate: true, strictSet: true });
    expect(changed.schema.properties.金).toEqual({ type: "number", required: true });
    expect(changed.schema.properties.列).toMatchObject({ type: "array", elementType: { type: "number" }, required: true });
  });

  it("strictSet 關掉 [值, 說明] 的特殊處理：整個陣列被取代", async () => {
    const { win } = sandbox();
    const old = { stat_data: { 屬性: [5, "說明"] }, schema: { strictSet: true } };
    const result = await win.Mvu.parseMessage("_.set('屬性', [1, '新']);", old);
    expect(result.stat_data.屬性).toEqual([1, "新"]);
  });
});

describe("parseMessage：不落檔與上限", () => {
  it("只送值解析請求，不送任何寫入", async () => {
    const { win, posted } = sandbox();
    await win.Mvu.parseMessage("_.set('金', 1);<JSONPatch>[]</JSONPatch>", base());
    expect(posted.every((message) => message.kind === "mvu-eval")).toBe(true);
  });

  it("訊息超過 256 KB、指令超過 1000 條：整次拒絕", async () => {
    const { win } = sandbox();
    await expect(win.Mvu.parseMessage("x".repeat(256 * 1024 + 1), base())).rejects.toMatchObject({ code: "message-too-large" });
    const many = "_.set('金', 1);".repeat(1001);
    await expect(win.Mvu.parseMessage(many, base())).rejects.toMatchObject({ code: "too-many-commands" });
    await expect(win.Mvu.parseMessage("_.set('金', 1);".repeat(1000), base())).resolves.toBeDefined();
  });

  it("Date（YAML 顯式時間戳）：set 與 insert 直接值、陣列第一層轉 ISO，物件內的 Date 不遞迴轉（上游 438f9ffc）", async () => {
    const { win } = sandbox();
    const old = withSchema({ 時間: "x", 物品: ["a"], 角色: { 甲: 1 } });
    const result = await win.Mvu.parseMessage(
      "_.set('時間', !!timestamp 2024-01-01);\n_.assign('物品', !!timestamp 2024-02-01);\n_.assign('物品', [!!timestamp 2024-03-01]);\n_.assign('角色', '乙', {d: !!timestamp 2024-04-01});",
      old,
    );
    expect(result.stat_data.時間).toBe("2024-01-01T00:00:00.000Z");
    expect(result.stat_data.物品[1]).toBe("2024-02-01T00:00:00.000Z");
    expect(result.stat_data.物品[2]).toEqual(["2024-03-01T00:00:00.000Z"]);
    expect(Object.prototype.toString.call(result.stat_data.角色.乙.d)).toBe("[object Date]");
  });

  it("指令數上限：JSONPatch 展開、監聽器新增也算（抽取完、監聽器後都重查）", async () => {
    const { win } = sandbox();
    const ops = Array.from({ length: 1001 }, () => ({ op: "replace", path: "/金", value: 1 }));
    await expect(win.Mvu.parseMessage(`<JSONPatch>${JSON.stringify(ops)}</JSONPatch>`, base())).rejects.toMatchObject({ code: "too-many-commands" });
    win.eventOn(win.Mvu.events.COMMAND_PARSED, (_v: unknown, commands: Table[]) => {
      for (let i = 0; i < 1001; i++) commands.push({ type: "set", full_match: "x", args: ["金", 1], reason: "" });
    });
    await expect(win.Mvu.parseMessage("", base())).rejects.toMatchObject({ code: "too-many-commands" });
  });

  it("陣列元素數看 length：稀疏陣列撐大也拒絕", async () => {
    const { win } = sandbox();
    const old = withSchema({ arr: [] });
    await expect(win.Mvu.parseMessage("_.insert('arr', '10000', 1);", old)).rejects.toMatchObject({ code: "too-many-children" });
    const sparse = await win.Mvu.parseMessage("_.insert('arr', 9, 1);", old);
    expect(sparse.stat_data.arr.length).toBe(1);
  });

  it("每次 await 返回與回傳前都重查 5 秒：最後的 ENDED_for_zod 監聽器、最後一筆值解析把時間推過也拒絕", async () => {
    const real = Date.now();
    let offset = 0;
    const spy = vi.spyOn(Date, "now").mockImplementation(() => real + offset);
    const { win } = sandbox();
    win.eventOn(`${win.Mvu.events.VARIABLE_UPDATE_ENDED}_for_zod`, () => {
      offset = 6000;
    });
    await expect(win.Mvu.parseMessage("_.set('金', 1);", base())).rejects.toMatchObject({ code: "deadline" });
    offset = 0;
    const slow = sandbox((op, text) => {
      offset = 6000;
      return runEval(op as "value", text);
    });
    await expect(slow.win.Mvu.parseMessage("_.set('金', 1);", base())).rejects.toMatchObject({ code: "deadline" });
    spy.mockRestore();
  });

  it("結果超出上限整批拒絕：單一字串 64 KB、深度、鍵長", async () => {
    const { win } = sandbox();
    const big = await win.Mvu.parseMessage("", { stat_data: { 字: "字".repeat(30000) }, schema: {} }).catch((e: Error & { code: string }) => e);
    expect((big as Error & { code: string }).code).toBe("string-too-long");
    const deep: Table = {};
    let cursor = deep;
    for (let i = 0; i < 33; i++) cursor = cursor.n = {};
    await expect(win.Mvu.parseMessage("", { stat_data: deep, schema: {} })).rejects.toMatchObject({ code: "too-deep" });
  });

  it("整次超過 5 秒：下一步檢查時拒絕", async () => {
    const { win } = sandbox();
    const real = Date.now();
    let jump = false;
    const spy = vi.spyOn(Date, "now").mockImplementation(() => (jump ? real + 6000 : real));
    win.eventOn(win.Mvu.events.COMMAND_PARSED, () => {
      jump = true;
    });
    await expect(win.Mvu.parseMessage("_.set('金', 1);", base())).rejects.toMatchObject({ code: "deadline" });
    spy.mockRestore();
  });

  it("宿主沒回值解析請求：保險逾時後拒絕", async () => {
    vi.useFakeTimers();
    try {
      const { win } = sandbox(() => null);
      const pending = win.Mvu.parseMessage("_.set('金', 1);", base());
      const caught = pending.catch((e: Error & { code: string }) => e);
      await vi.advanceTimersByTimeAsync(6000);
      expect(((await caught) as Error & { code: string }).code).toBe("eval-timeout");
    } finally {
      vi.useRealTimers();
    }
  });

  it("參數不合法：message 非字串、old_data 非物件", async () => {
    const { win } = sandbox();
    await expect(win.Mvu.parseMessage(1 as unknown as string, base())).rejects.toThrow(/message/);
    await expect(win.Mvu.parseMessage("", null)).rejects.toThrow(/old_data/);
  });
});

// mvu-replace-numeric：同一份案例後端也跑（src-tauri data/message_vars/replace_parity_tests.rs），兩邊結果要一致
describe("parseMessage：沒重構 MVU 卡的數字欄 replace（與後端同一份案例）", () => {
  for (const sample of replaceParity.cases) {
    it(sample.name, async () => {
      const { win } = sandbox();
      const message = `<UpdateVariable>\n<JSONPatch>\n${JSON.stringify(sample.patch)}\n</JSONPatch>\n</UpdateVariable>`;
      const result = await win.Mvu.parseMessage(message, withSchema(structuredClone(sample.stat_data) as Table));
      expect(JSON.parse(JSON.stringify(result.stat_data))).toEqual(sample.expected);
    });
  }
});
