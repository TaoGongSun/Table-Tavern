import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { type CardChat } from "./card-chat-shim";
import { applyScripts, buildShellDocument, extractShell, type InterfaceScript } from "./interface-card";
import {
  buildCardMvu,
  buildMvuData,
  buildMvuShimSource,
  floorSources,
  restoreLeaf,
  withMvuPlaceholder,
  type CardMvu,
  type MvuData,
  type StateTree,
} from "./card-mvu-shim";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

const macros = { user: "阿濤", char: "迷宮" };
// 沙盒最前面內嵌的就是這份 lodash；單元測試的假 window 也先放上去
const lodashName = "lodash";
const lodash: unknown = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;
const BCD = "bcd368cacbc56b2cc29010e0cbad5f09.png";

describe("restoreLeaf：葉子型別還原", () => {
  it("數字：整數、負數、小數、指數", () => {
    expect(restoreLeaf("19")).toBe(19);
    expect(restoreLeaf("-3")).toBe(-3);
    expect(restoreLeaf("0.5")).toBe(0.5);
    expect(restoreLeaf("1e3")).toBe(1000);
    expect(restoreLeaf("-2.5E-2")).toBe(-0.025);
    expect(Object.is(restoreLeaf("-0"), -0)).toBe(true);
  });

  it("數字邊界一律保留字串：前導零、.5、1.、+1、溢位", () => {
    for (const text of ["007", ".5", "1.", "+1", "1e999", "-1e999", "1_000", " 1", "0x10", "Infinity", "NaN"]) {
      expect(restoreLeaf(text)).toBe(text);
    }
  });

  it("布林、null、合法 JSON", () => {
    expect(restoreLeaf("true")).toBe(true);
    expect(restoreLeaf("false")).toBe(false);
    expect(restoreLeaf("null")).toBeNull();
    expect(restoreLeaf("[]")).toEqual([]);
    expect(restoreLeaf("{}")).toEqual({});
    expect(restoreLeaf('[1, "說明"]')).toEqual([1, "說明"]);
  });

  it("壞 JSON、YAML 行內陣列、# 開頭字串保留字串", () => {
    expect(restoreLeaf("[1, 2")).toBe("[1, 2");
    expect(restoreLeaf("[1, 说明]")).toBe("[1, 说明]");
    expect(restoreLeaf("#烦躁 #废了")).toBe("#烦躁 #废了");
    expect(restoreLeaf("18cm/无法测量")).toBe("18cm/无法测量");
  });

  it("JSON 巢狀裡有非有限數字，整個葉值保留原字串（陣列與物件兩種）", () => {
    expect(restoreLeaf("[1, [2, 1e999]]")).toBe("[1, [2, 1e999]]");
    expect(restoreLeaf('{"a": {"b": -1e999}}')).toBe('{"a": {"b": -1e999}}');
  });

  it("value_types 優先：number／bool／list，不符就保留字串", () => {
    expect(restoreLeaf("007", "number")).toBe("007");
    expect(restoreLeaf("12", "number")).toBe(12);
    expect(restoreLeaf("1e999", "number")).toBe("1e999");
    expect(restoreLeaf("abc", "number")).toBe("abc");
    expect(restoreLeaf("true", "bool")).toBe(true);
    expect(restoreLeaf("是", "bool")).toBe("是");
    expect(restoreLeaf("1", "bool")).toBe("1");
    expect(restoreLeaf('["劍", "盾"]', "list")).toEqual(["劍", "盾"]);
    expect(restoreLeaf("劍、盾", "list")).toBe("劍、盾");
    expect(restoreLeaf("{}", "list")).toBe("{}");
  });
});

describe("buildMvuData", () => {
  it("轉出 stat_data，清掉每一層的 $meta（含 JSON 還原出的巢狀值），結果不含非有限數", () => {
    const data = buildMvuData(
      {
        基础信息: { $meta: { extensible: "false" }, 时间: "未知", 带出财宝计算: "0" },
        迷宫房间: {
          $meta: { extensible: "true", template: { 名字: "未命名房间" } },
          Room_001: { 房间内容: { 魔物: "[]", 其他设施: "深渊裂缝" }, 是否已摧毁: "false" },
        },
        旧式: '{"$meta": {"x": 1}, "值": [5, "说明"]}',
      },
      {},
      macros,
    );
    expect(data.stat_data).toEqual({
      基础信息: { 时间: "未知", 带出财宝计算: 0 },
      迷宫房间: { Room_001: { 房间内容: { 魔物: [], 其他设施: "深渊裂缝" }, 是否已摧毁: false } },
      旧式: { 值: [5, "说明"] },
    });
    expect(data.display_data).toEqual(data.stat_data);
    expect(data.display_data).not.toBe(data.stat_data);
    expect(data.delta_data).toEqual({});
    expect(JSON.parse(JSON.stringify(data))).toEqual(data);
  });

  it("value_types 以點分路徑對應", () => {
    const data = buildMvuData({ 玩家: { 編號: "007", 等級: "3" } }, { "玩家.編號": "number" }, macros);
    expect(data.stat_data).toEqual({ 玩家: { 編號: "007", 等級: 3 } });
  });

  it("{{user}}／{{char}} 遞迴代換：葉值、巢狀 JSON、陣列、鍵", () => {
    const data = buildMvuData(
      {
        "{{user}}": { 稱呼: "{{USER}}大人", 同伴: '["{{char}}", {"名": "{{user}}"}]' },
        設定: { 主角: "{{user}}" },
      },
      {},
      macros,
    );
    expect(data.stat_data).toEqual({
      阿濤: { 稱呼: "阿濤大人", 同伴: ["迷宮", { 名: "阿濤" }] },
      設定: { 主角: "阿濤" },
    });
  });

  it("鍵代換後撞名：保留原字面並警告，不覆蓋既有鍵", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const data = buildMvuData({ "{{user}}": "代換來的", 阿濤: "本來就有" }, {}, macros);
    expect(data.stat_data).toEqual({ "{{user}}": "代換來的", 阿濤: "本來就有" });
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("玩家名含 $&、$'、$` 與巨集字面：照字面放進去、只代換一次", () => {
    const tricky = { user: "A$&B$'C$`D", char: "{{user}}" };
    const data = buildMvuData(
      { 名: "{{user}}", 伴: "{{char}}", 層: { 深: '["{{user}}"]' }, "{{user}}": "鍵" },
      {},
      tricky,
    );
    expect(data.stat_data).toEqual({ 名: "A$&B$'C$`D", 伴: "{{user}}", 層: { 深: ["A$&B$'C$`D"] }, "A$&B$'C$`D": "鍵" });
    const nameWithMacro = buildMvuData({ 名: "{{user}}" }, {}, { user: "{{char}}", char: "迷宮" });
    expect(nameWithMacro.stat_data).toEqual({ 名: "{{char}}" });
  });

  it("__proto__／constructor 鍵存成自有資料，不動原型、不消失", () => {
    const tree = JSON.parse('{"__proto__": {"x": "1"}, "constructor": "c", "玩家": {"__proto__": "2"}}');
    const data = buildMvuData(tree, JSON.parse('{"__proto__": "number"}'), macros);
    expect(Object.keys(data.stat_data)).toEqual(["__proto__", "constructor", "玩家"]);
    expect(Object.getPrototypeOf(data.stat_data)).toBe(Object.prototype);
    expect(Object.getOwnPropertyDescriptor(data.stat_data, "__proto__")?.value).toEqual({ x: 1 });
    expect(Object.getOwnPropertyDescriptor(data.stat_data.玩家 as object, "__proto__")?.value).toBe(2);
    expect(JSON.stringify(data.display_data)).toBe(JSON.stringify(data.stat_data));
  });

  it("metadata 清理：陣列裡的 $__META_EXTENSIBLE__$ 與 $arrayMeta＋$meta 載體元素整個移除", () => {
    const data = buildMvuData(
      {
        背包: '["劍", "$__META_EXTENSIBLE__$", {"$arrayMeta": true, "$meta": {"template": "x"}}, {"$arrayMeta": true, "名": "留下"}, {"名": "盾", "$meta": {}}]',
        巢: '{"清單": [["$__META_EXTENSIBLE__$", 1]], "$meta": {"extensible": true}}',
      },
      {},
      macros,
    );
    expect(data.stat_data).toEqual({
      背包: ["劍", { $arrayMeta: true, 名: "留下" }, { 名: "盾" }],
      巢: { 清單: [[1]] },
    });
  });

  it("沒有角色名時 {{char}} 原樣保留", () => {
    expect(buildMvuData({ a: "{{char}}" }, {}, { user: "阿濤", char: null }).stat_data).toEqual({ a: "{{char}}" });
  });
});

const ev = (kind: TranscriptEvent["kind"], speakerId: string, gold?: string): TranscriptEvent => ({
  ts: "t",
  speaker_id: speakerId,
  speaker_name: kind === "player" ? "玩家" : "GM",
  kind,
  text: "x",
  ...(gold === undefined ? {} : { state: { table: {}, tree: { 金: gold } } }),
});
const gmEv = (gold?: string) => ev("narration", "", gold);
const playerEv = (gold?: string) => ev("player", "", gold);
const charEv = (gold?: string) => ev("dialogue", "c1", gold);
const sysEv = (gold?: string) => ev("system", "", gold);

describe("floorSources：活樓與歷史樓", () => {
  it("最後一個會改狀態的樓與其後的樓（玩家、角色、系統）都是活的", () => {
    expect(floorSources([gmEv("1"), playerEv("1"), gmEv("2"), playerEv("2"), charEv("2"), sysEv("2")])).toEqual([
      0,
      1,
      "live",
      "live",
      "live",
      "live",
    ]);
  });

  it("歷史樓即使中間夾玩家句，也只讀自己之前（含）最近的快照", () => {
    expect(floorSources([gmEv("1"), playerEv(), charEv(), gmEv("3")])).toEqual([0, 0, 0, "live"]);
  });

  it("前面沒快照的歷史樓是 empty；完全沒有 GM 樓時全部是活的", () => {
    expect(floorSources([playerEv(), gmEv("1"), gmEv("2")])).toEqual(["empty", 1, "live"]);
    expect(floorSources([playerEv("1"), charEv("1")])).toEqual(["live", "live"]);
  });

  it("中止／截斷的 GM 回覆也算會改狀態的樓：它之後才是活的", () => {
    const aborted: TranscriptEvent = { ...gmEv("2"), truncated: true };
    expect(floorSources([gmEv("1"), playerEv("1"), aborted, playerEv("2")])).toEqual([0, 1, "live", "live"]);
  });

  it("只有 system 樓：全部是活的", () => {
    expect(floorSources([sysEv("1"), sysEv("1")])).toEqual(["live", "live"]);
  });

  it("角色台詞不算會改狀態的樓", () => {
    expect(floorSources([gmEv("1"), charEv("1"), charEv("1")])).toEqual(["live", "live", "live"]);
  });
});

describe("buildCardMvu", () => {
  it("每樓資料去重；latestId 跳過 system 尾樓", () => {
    const events = [gmEv("1"), playerEv("1"), gmEv("2"), sysEv("2")];
    const mvu = buildCardMvu({
      events,
      roles: ["assistant", "user", "assistant", "system"],
      currentId: 2,
      liveTree: { 金: "9" },
      valueTypes: {},
      macros,
    });
    expect(mvu.states.map((state) => state.stat_data)).toEqual([{ 金: 1 }, { 金: 9 }]);
    expect(mvu.floorState).toEqual([0, 0, 1, 1]);
    expect(mvu.latestId).toBe(2);
    expect(mvu.currentId).toBe(2);
  });

  it("空桌：開場白一樓讀目前狀態", () => {
    const mvu = buildCardMvu({ events: [], roles: ["assistant"], currentId: 0, liveTree: { 金: "3" }, valueTypes: {}, macros });
    expect(mvu.floorState).toEqual([0]);
    expect(mvu.states[0].stat_data).toEqual({ 金: 3 });
    expect(mvu.latestId).toBe(0);
  });
});

describe("withMvuPlaceholder", () => {
  it("AI 樓（不含系統事件）、有 stat_data、夠長、還沒占位才補；開場白不補", () => {
    expect(withMvuPlaceholder("劇情推進了", false, "assistant", true)).toBe("劇情推進了\n\n<StatusPlaceHolderImpl/>");
    expect(withMvuPlaceholder("系統事件文字", false, "system", true)).toBe("系統事件文字");
    expect(withMvuPlaceholder("劇情推進了", true, "assistant", true)).toBe("劇情推進了");
    expect(withMvuPlaceholder("劇情推進了", false, "user", true)).toBe("劇情推進了");
    expect(withMvuPlaceholder("劇情推進了", false, "assistant", false)).toBe("劇情推進了");
    expect(withMvuPlaceholder("短", false, "assistant", true)).toBe("短");
    const once = "正文\n<StatusPlaceHolderImpl/>";
    expect(withMvuPlaceholder(once, false, "assistant", true)).toBe(once);
  });
});

// ---- 沙盒墊片 ----

type Listener = (event: { source: unknown; data: unknown }) => void;
type Handler = (...args: unknown[]) => unknown;
interface MvuSandbox {
  parent: object;
  listeners: Listener[];
  addEventListener: (type: string, fn: Listener) => void;
  getAllVariables: () => MvuData;
  waitGlobalInitialized: (name: string) => Promise<unknown>;
  initializeGlobal: (name: string, value: unknown) => void;
  eventOn: (name: string, fn: Handler) => { stop: () => void };
  eventOnce: (name: string, fn: Handler) => { stop: () => void };
  eventMakeFirst: (name: string, fn: Handler) => { stop: () => void };
  eventRemoveListener: (name: string, fn: Handler) => void;
  eventClearEvent: (name: string) => void;
  eventClearListener: (fn: Handler) => void;
  eventClearAll: () => void;
  eventMakeLast: (name: string, fn: Handler) => { stop: () => void };
  eventEmit: (name: string, ...args: unknown[]) => Promise<void>;
  Mvu: {
    events: Record<string, string>;
    getMvuData: (option?: unknown) => MvuData;
    getMvuVariable: (data: unknown, path: unknown, option?: unknown) => unknown;
  };
  [key: string]: unknown;
}

const data = (stat: Record<string, unknown>): MvuData => ({ stat_data: stat, display_data: stat, delta_data: {} });
const snapshot: CardMvu = {
  currentId: 2,
  latestId: 2,
  states: [data({ 金: 1, 物品: { 劍: 1 } }), data({ 金: 5, 物品: { 劍: 1 }, 舊式: [7, "說明"] })],
  floorState: [0, 0, 1, 1],
};

function sandbox(mvu: CardMvu = snapshot, token = "tok"): MvuSandbox {
  const listeners: Listener[] = [];
  const win = {
    parent: {},
    listeners,
    _: lodash,
    addEventListener(type: string, fn: Listener) {
      if (type === "message") listeners.push(fn);
    },
  } as unknown as MvuSandbox;
  new Function("window", buildMvuShimSource(mvu, token))(win);
  return win;
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

async function push(win: MvuSandbox, mvu: unknown, extra: Record<string, unknown> = {}) {
  for (const listener of win.listeners) {
    listener({
      source: win.parent,
      data: { source: "table-tavern-host", kind: "chat", token: "tok", mvu, ...extra },
    });
  }
  await flush();
}

describe("MVU 墊片：讀取", () => {
  it("getAllVariables 回本樓資料的深拷貝", () => {
    const win = sandbox();
    const vars = win.getAllVariables();
    expect(vars.stat_data).toEqual({ 金: 5, 物品: { 劍: 1 }, 舊式: [7, "說明"] });
    (vars.stat_data as { 金: number }).金 = 999;
    expect(win.getAllVariables().stat_data.金).toBe(5);
  });

  it("快照裡的 __proto__ 鍵照樣是自有資料", () => {
    const tricky = JSON.parse('{"__proto__": {"x": 1}, "金": 2}') as Record<string, unknown>;
    const win = sandbox({ currentId: 0, latestId: 0, states: [data(tricky)], floorState: [0] });
    const stat = win.getAllVariables().stat_data;
    expect(Object.keys(stat)).toEqual(["__proto__", "金"]);
    expect(Object.getPrototypeOf(stat)).toBe(Object.prototype);
  });

  it("waitGlobalInitialized：Mvu 已分享立即 resolve（不回傳值）；原型上的名稱不算存在；initializeGlobal 發初始化事件", async () => {
    const win = sandbox();
    expect(win.Mvu).toBeDefined();
    await expect(win.waitGlobalInitialized("Mvu")).resolves.toBeUndefined();
    for (const name of ["constructor", "toString", "Other", "a.b"]) {
      let done = false;
      void win.waitGlobalInitialized(name).then(() => {
        done = true;
      });
      await flush();
      expect(done).toBe(false);
      const fired = vi.fn();
      win.eventOn(`global_${name}_initialized`, fired);
      win.initializeGlobal(name, 42);
      await flush();
      expect(done).toBe(true);
      expect(fired).toHaveBeenCalledTimes(1);
    }
    expect((win.a as { b: number }).b).toBe(42);
  });

  it("全域路徑照 lodash：括號與索引建出陣列、既存的字面鍵也算已存在", async () => {
    const win = sandbox();
    win.initializeGlobal("Probe[0].ready", true);
    expect(Array.isArray(win.Probe)).toBe(true);
    expect((win.Probe as { ready: boolean }[])[0].ready).toBe(true);
    expect(win["Probe[0]"]).toBeUndefined();
    let done = false;
    void win.waitGlobalInitialized("Probe[0].ready").then(() => {
      done = true;
    });
    await flush();
    expect(done).toBe(true);
    win["x.y"] = 1;
    let literal = false;
    void win.waitGlobalInitialized("x.y").then(() => {
      literal = true;
    });
    await flush();
    expect(literal).toBe(true);
  });

  it("Mvu.events 照上游字串", () => {
    expect(sandbox().Mvu.events.VARIABLE_UPDATE_ENDED).toBe("mag_variable_update_ended");
    expect(sandbox().Mvu.events.SINGLE_VARIABLE_UPDATED).toBe("mag_variable_updated");
  });

  it("getMvuData：正數、負數、'latest'、不給 message_id、超出範圍拋錯", () => {
    const win = sandbox();
    expect(win.Mvu.getMvuData({ type: "message", message_id: 0 }).stat_data.金).toBe(1);
    expect(win.Mvu.getMvuData({ type: "message", message_id: 3 }).stat_data.金).toBe(5);
    expect(win.Mvu.getMvuData({ type: "message", message_id: -4 }).stat_data.金).toBe(1);
    expect(win.Mvu.getMvuData({ type: "message", message_id: "latest" }).stat_data.金).toBe(5);
    expect(win.Mvu.getMvuData({ type: "message" }).stat_data.金).toBe(5);
    expect(() => win.Mvu.getMvuData({ type: "message", message_id: 4 })).toThrow();
    expect(() => win.Mvu.getMvuData({ type: "message", message_id: -5 })).toThrow();
    expect(() => win.Mvu.getMvuData({ type: "message", message_id: 1.5 })).toThrow();
  });

  it("尾樓是 system：'latest'／預設取前一則非 system 樓，-1 取 system 尾樓", () => {
    const win = sandbox({ ...snapshot, latestId: 1 });
    expect(win.Mvu.getMvuData({ type: "message" }).stat_data.金).toBe(1);
    expect(win.Mvu.getMvuData({ type: "message", message_id: "latest" }).stat_data.金).toBe(1);
    expect(win.Mvu.getMvuData({ type: "message", message_id: -1 }).stat_data.金).toBe(5);
  });

  it("只有 system 樓：'latest' 拋錯，明確樓號照讀", () => {
    const win = sandbox({ ...snapshot, latestId: -1 });
    expect(() => win.Mvu.getMvuData({ type: "message" })).toThrow();
    expect(win.Mvu.getMvuData({ type: "message", message_id: 0 }).stat_data.金).toBe(1);
  });

  it("非 message 層與沒給 type 明確拋錯，不暗改成 message 層", () => {
    const win = sandbox();
    for (const option of [undefined, {}, { type: "chat" }, { type: "global" }, { type: "character" }]) {
      expect(() => win.Mvu.getMvuData(option)).toThrow(/尚未支援/);
    }
  });

  it("getMvuVariable：三類別、[值, 說明] 取第一項、預設值", () => {
    const win = sandbox();
    const vars = { stat_data: { a: { b: [3, "說明"] }, c: 2 }, display_data: { c: "1->2" }, delta_data: {} };
    expect(win.Mvu.getMvuVariable(vars, "a.b")).toBe(3);
    expect(win.Mvu.getMvuVariable(vars, ["a", "b"])).toBe(3);
    expect(win.Mvu.getMvuVariable(vars, "c", { category: "display" })).toBe("1->2");
    expect(win.Mvu.getMvuVariable(vars, "c", { category: "delta", default_value: 0 })).toBe(0);
    expect(win.Mvu.getMvuVariable(vars, "x.y", { default_value: "無" })).toBe("無");
    expect(win.Mvu.getMvuVariable({ stat_data: { v: [1, 2] } }, "v")).toEqual([1, 2]);
    // 上游用 _.get：原型鏈上的 constructor 也取得到，不回預設值
    expect(win.Mvu.getMvuVariable(vars, "constructor", { default_value: "無" })).toBe(Object);
  });

  it("寫入類函式本包不定義", () => {
    const win = sandbox() as unknown as Record<string, unknown> & { Mvu: Record<string, unknown> };
    expect(win.Mvu.setMvuVariable).toBeUndefined();
    expect(win.Mvu.replaceMvuData).toBeUndefined();
    expect(win.replaceVariables).toBeUndefined();
  });
});

describe("MVU 墊片：事件", () => {
  const changed: CardMvu = {
    ...snapshot,
    states: [snapshot.states[0], data({ 金: 9, 物品: { 劍: 1, 盾: 1 }, 舊式: [7, "說明"] })],
  };

  it("本樓資料變了：STARTED 只帶舊資料 → SINGLE 逐葉（含分支增刪）→ ENDED（新, 舊）", async () => {
    const win = sandbox();
    const calls: [string, unknown[]][] = [];
    for (const name of ["VARIABLE_UPDATE_STARTED", "SINGLE_VARIABLE_UPDATED", "VARIABLE_UPDATE_ENDED"]) {
      win.eventOn(win.Mvu.events[name], (...args) => {
        calls.push([name, args]);
      });
    }
    const branchy: CardMvu = {
      ...snapshot,
      states: [snapshot.states[0], data({ 金: 9, 新分支: { a: 1 }, 舊式: [7, "說明"] })],
    };
    await push(win, branchy);
    expect(calls.map(([name]) => name)).toEqual([
      "VARIABLE_UPDATE_STARTED",
      "SINGLE_VARIABLE_UPDATED",
      "SINGLE_VARIABLE_UPDATED",
      "SINGLE_VARIABLE_UPDATED",
      "VARIABLE_UPDATE_ENDED",
    ]);
    expect(calls[0][1]).toEqual([snapshot.states[1]]);
    expect(calls.slice(1, 4).map(([, args]) => args.slice(1))).toEqual([
      ["金", 5, 9],
      ["物品", { 劍: 1 }, undefined],
      ["新分支", undefined, { a: 1 }],
    ]);
    expect(calls[4][1]).toEqual([branchy.states[1], snapshot.states[1]]);
    expect(win.getAllVariables().stat_data.金).toBe(9);
  });

  it("本樓資料沒變（例如 load 補推）：換快照但不發事件", async () => {
    const win = sandbox();
    const fn = vi.fn();
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, fn);
    await push(win, { ...snapshot, floorState: [1, 1, 1, 1] });
    expect(fn).not.toHaveBeenCalled();
    expect(win.Mvu.getMvuData({ type: "message", message_id: 0 }).stat_data.金).toBe(5);
  });

  it("推送驗來源、token、kind、形狀", async () => {
    const win = sandbox();
    const fn = vi.fn();
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, fn);
    const bad = [
      { ...changed, floorState: [0, 0, 5, 1] },
      { ...changed, currentId: 9 },
      { ...changed, latestId: 4 },
      { ...changed, states: [{ stat_data: [] }] },
      { ...changed, states: "x" },
      null,
    ];
    for (const mvu of bad) await push(win, mvu);
    await push(win, changed, { token: "別的" });
    await push(win, changed, { kind: "other" });
    for (const listener of win.listeners) {
      listener({ source: {}, data: { source: "table-tavern-host", kind: "chat", token: "tok", mvu: changed } });
    }
    await flush();
    expect(fn).not.toHaveBeenCalled();
    expect(win.getAllVariables().stat_data.金).toBe(5);
  });

  it("async 監聽器依序等完；連續兩次推送排隊處理、不交錯", async () => {
    const win = sandbox();
    const log: string[] = [];
    const slow = (label: string) => async (vars: unknown) => {
      const gold = (vars as MvuData).stat_data.金;
      log.push(`${label}開始${gold}`);
      await new Promise((resolve) => setTimeout(resolve, 5));
      log.push(`${label}結束${gold}`);
    };
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, slow("S"));
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, slow("E"));
    const third: CardMvu = { ...snapshot, states: [snapshot.states[0], data({ 金: 11 })] };
    for (const listener of win.listeners) {
      for (const mvu of [changed, third]) {
        listener({ source: win.parent, data: { source: "table-tavern-host", kind: "chat", token: "tok", mvu } });
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(log).toEqual(["S開始5", "S結束5", "E開始9", "E結束9", "S開始9", "S結束9", "E開始11", "E結束11"]);
  });

  it("STARTED 監聽器裡讀到的仍是舊值，ENDED 裡讀到新值", async () => {
    const win = sandbox();
    const seen: unknown[] = [];
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, () => seen.push(win.getAllVariables().stat_data.金));
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, () => seen.push(win.getAllVariables().stat_data.金));
    await push(win, changed);
    expect(seen).toEqual([5, 9]);
  });

  it("eventOnce 只觸發一次；eventRemoveListener、stop、eventClearEvent、eventClearListener 都能移除；同一函式不重複註冊", async () => {
    const win = sandbox();
    const once = vi.fn();
    const removed = vi.fn();
    const stopped = vi.fn();
    const cleared = vi.fn();
    const twice = vi.fn();
    const ended = win.Mvu.events.VARIABLE_UPDATE_ENDED;
    win.eventOnce(ended, once);
    win.eventOn(ended, removed);
    win.eventRemoveListener(ended, removed);
    win.eventOn(ended, stopped).stop();
    win.eventOn(ended, cleared);
    win.eventClearListener(cleared);
    win.eventOn(ended, twice);
    win.eventOn(ended, twice);
    await push(win, changed);
    await push(win, snapshot);
    expect(once).toHaveBeenCalledTimes(1);
    expect(removed).not.toHaveBeenCalled();
    expect(stopped).not.toHaveBeenCalled();
    expect(cleared).not.toHaveBeenCalled();
    expect(twice).toHaveBeenCalledTimes(2);
    win.eventClearEvent(ended);
    await push(win, changed);
    expect(twice).toHaveBeenCalledTimes(2);
  });

  it("事件名是原型鍵（constructor、__proto__）也只是普通事件；eventMakeFirst 排到最前；eventEmit 可自發", async () => {
    const win = sandbox();
    const order: string[] = [];
    win.eventOn("constructor", () => order.push("a"));
    const first = () => order.push("first");
    win.eventOn("constructor", first);
    win.eventMakeFirst("constructor", first);
    win.eventOn("__proto__", () => order.push("proto"));
    await win.eventEmit("constructor");
    await win.eventEmit("__proto__");
    await win.eventEmit("toString");
    expect(order).toEqual(["first", "a", "proto"]);
  });

  it("監聽器拋錯或拒絕：記錄後照樣通知其他監聽器；改參數不污染快照", async () => {
    const win = sandbox();
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const after = vi.fn();
    const ended = win.Mvu.events.VARIABLE_UPDATE_ENDED;
    win.eventOn(ended, (vars) => {
      (vars as MvuData).stat_data.金 = -1;
      throw new Error("壞監聽器");
    });
    win.eventOn(ended, async () => {
      throw new Error("拒絕的監聽器");
    });
    win.eventOn(ended, after);
    await push(win, changed);
    expect(after).toHaveBeenCalledTimes(1);
    expect((after.mock.calls[0][0] as MvuData).stat_data.金).toBe(9);
    expect(win.getAllVariables().stat_data.金).toBe(9);
    expect(error).toHaveBeenCalledTimes(2);
    error.mockRestore();
  });
});

// ---- 整份 srcdoc（內建庫 → 讀訊息 → MVU → 橋接 → 卡片）在 jsdom 裡跑 ----

interface DomWindow {
  document: Document;
  dispatchEvent: (event: Event) => boolean;
  addEventListener: (type: string, fn: (event: MessageEvent) => void) => void;
  MessageEvent: typeof MessageEvent;
  close: () => void;
}

async function runDocument(html: string): Promise<DomWindow> {
  const name = "jsdom";
  const { JSDOM } = (await import(/* @vite-ignore */ name)) as {
    JSDOM: new (html: string, options: object) => { window: DomWindow };
  };
  const win = new JSDOM(html, { runScripts: "dangerously", pretendToBeVisual: true }).window;
  await new Promise((resolve) => setTimeout(resolve, 50));
  return win;
}

async function hostPush(win: DomWindow, mvu: CardMvu, chat: CardChat, token = "tok") {
  // jsdom 頂層文件的 parent 就是自己：墊片收好的真 parent＝win
  win.dispatchEvent(
    new win.MessageEvent("message", {
      data: { source: "table-tavern-host", kind: "chat", token, chat, mvu },
      source: win as unknown as Window,
    }),
  );
  await new Promise((resolve) => setTimeout(resolve, 30));
}

const oneFloorChat = (message: string): CardChat => ({ currentId: 0, floors: [{ name: "GM", role: "assistant", message }] });
const mvuOf = (tree: StateTree): CardMvu =>
  buildCardMvu({ events: [], roles: ["assistant"], currentId: 0, liveTree: tree, valueTypes: {}, macros });

describe("整份殼文件：仿 MVU 前端卡的寫法", () => {
  const shell = `<!DOCTYPE html><html><head></head><body><span id="gold"></span><span id="count">0</span>
<button id="act" onclick="triggerSlash('/send 前進')">前進</button>
<script>
var renders = 0;
function render() {
  var V = _.get(getAllVariables(), 'stat_data', {});
  renders += 1;
  $('#gold').text(_.get(V, '玩家.金幣'));
  $('#count').text(String(renders));
}
async function init() {
  await waitGlobalInitialized('Mvu');
  render();
  eventOn(Mvu.events.VARIABLE_UPDATE_ENDED, render);
}
$(errorCatched(init));
</script></body></html>`;

  it("首次渲染讀到值；推送變動後重畫一次；同值補推不重畫；按鈕送出仍走誘餌", async () => {
    const chat = oneFloorChat("x");
    const first = mvuOf({ 玩家: { 金幣: "5" } });
    const win = await runDocument(buildShellDocument(shell, {}, { chat, token: "tok", mvu: first }));
    const text = (id: string) => win.document.getElementById(id)?.textContent;
    expect(text("gold")).toBe("5");
    expect(text("count")).toBe("1");

    await hostPush(win, first, chat);
    expect(text("count")).toBe("1");

    await hostPush(win, mvuOf({ 玩家: { 金幣: "8" } }), chat);
    expect(text("gold")).toBe("8");
    expect(text("count")).toBe("2");

    const sent: unknown[] = [];
    win.addEventListener("message", (event) => {
      if ((event.data as { source?: string }).source === "table-tavern-card") sent.push(event.data);
    });
    (win.document.getElementById("act") as HTMLButtonElement).click();
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(sent).toContainEqual({ source: "table-tavern-card", kind: "input", text: "前進" });
    win.close();
  });
});

// TestCards/ 是 gitignore 的本機測試卡：有才跑，CI 上略過
const cardsDir = new URL("../../../TestCards/", import.meta.url);
const hasCards = existsSync(new URL("DongeonMaster.png", cardsDir)) && existsSync(new URL(BCD, cardsDir));

function cardScript(file: string, scriptName: string): InterfaceScript {
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
      if (key === "ccv3" || (key === "chara" && json === "")) {
        json = Buffer.from(chunk.toString("latin1", split + 1), "base64").toString("utf8");
      }
    }
    offset += 12 + length;
  }
  const card = JSON.parse(json) as { data: { extensions: { regex_scripts: Record<string, unknown>[] } } };
  const raw = card.data.extensions.regex_scripts.find((script) => script.scriptName === scriptName)!;
  return {
    name: scriptName,
    find_regex: String(raw.findRegex),
    replace_string: String(raw.replaceString),
    trim_strings: [],
    min_depth: null,
    max_depth: null,
  };
}

function realShell(file: string, scriptName: string): string {
  const shell = extractShell(applyScripts("劇情推進的正文\n\n<StatusPlaceHolderImpl/>", [cardScript(file, scriptName)]));
  // jsdom 不跑 module script：改成一般 script（卡片腳本放在 body 尾端，執行時機相同）
  return shell!.replace(/<script type="module">/g, "<script>");
}

describe.skipIf(!hasCards)("真卡的 MVU 前端（TestCards 本機限定）", () => {
  it("DongeonMaster「迷宫之书」：首次渲染時間欄，推送後重畫", async () => {
    const chat = oneFloorChat("x");
    const tree = { 基础信息: { $meta: { extensible: "false" }, 时间: "第3日", 具体时间: "黄昏" }, 迷宫房间: {} };
    const win = await runDocument(
      buildShellDocument(realShell("DongeonMaster.png", "迷宫之书"), {}, { chat, token: "tok", mvu: mvuOf(tree) }),
    );
    const time = () => win.document.getElementById("w-time")?.textContent;
    expect(time()).toBe("第3日 黄昏");
    await hostPush(win, mvuOf({ ...tree, 基础信息: { 时间: "第4日", 具体时间: "清晨" } }), chat);
    expect(time()).toBe("第4日 清晨");
    win.close();
  });

  it.each(["mvu前端", "mvu前端（武侠）"])("bcd368「%s」：首次渲染帳號名稱，推送後重畫", async (scriptName) => {
    const chat = oneFloorChat("x");
    const tree = { 用户数据: { 名称: "測試玩家", 等级: "3", 资金: "12345", 积分: "7" } };
    const win = await runDocument(buildShellDocument(realShell(BCD, scriptName), {}, { chat, token: "tok", mvu: mvuOf(tree) }));
    expect(win.document.body.textContent).toContain("測試玩家");
    await hostPush(win, mvuOf({ 用户数据: { ...tree.用户数据, 名称: "改名玩家" } }), chat);
    expect(win.document.body.textContent).toContain("改名玩家");
    win.close();
  });
});

describe("墊片順序", () => {
  it("內建庫 → 讀訊息 → MVU → 橋接 → 卡片；沒有 MVU 快照時不定義 MVU 函式", () => {
    const shell = "<html><head></head><body><script>/*卡片*/</script></body></html>";
    const doc = buildShellDocument(shell, {}, { chat: oneFloorChat("x"), token: "tok", mvu: mvuOf({ a: "1" }) });
    const order = ["jQuery v3.7.1", "lodash", "window.errorCatched", "getChatMessages", "getAllVariables", "__ttHost", "/*卡片*/"];
    const positions = order.map((marker) => doc.indexOf(marker));
    expect(positions.every((position) => position >= 0)).toBe(true);
    expect([...positions].sort((a, b) => a - b)).toEqual(positions);
    const plain = buildShellDocument(shell, {}, { chat: oneFloorChat("x"), token: "tok", mvu: null });
    expect(plain).not.toContain("getAllVariables");
    expect(plain).toContain("jQuery v3.7.1");
  });
});

describe("MVU 墊片：通用事件 API", () => {
  const removers: [string, (win: MvuSandbox, name: string, victim: Handler) => void][] = [
    ["eventRemoveListener", (win, name, victim) => win.eventRemoveListener(name, victim)],
    ["eventClearEvent", (win, name) => win.eventClearEvent(name)],
    ["eventClearListener", (win, _name, victim) => win.eventClearListener(victim)],
    ["eventClearAll", (win) => win.eventClearAll()],
  ];

  it.each(removers)("派送途中被 %s 移除的監聽器，輪到它時不再呼叫", async (_label, remove) => {
    const win = sandbox();
    const calls: string[] = [];
    const b = () => calls.push("B");
    win.eventOn("probe", () => {
      calls.push("A");
      remove(win, "probe", b);
    });
    win.eventOn("probe", b);
    await win.eventEmit("probe");
    expect(calls).toEqual(["A"]);
  });

  it("stop() 在派送途中移除也一樣", async () => {
    const win = sandbox();
    const calls: string[] = [];
    let handle: { stop: () => void } | null = null;
    win.eventOn("probe", () => {
      calls.push("A");
      handle?.stop();
    });
    handle = win.eventOn("probe", () => calls.push("B"));
    await win.eventEmit("probe");
    expect(calls).toEqual(["A"]);
  });

  it("eventOnce 之後 eventMakeFirst／eventMakeLast：保留 once，發兩次只執行一次", async () => {
    for (const move of ["eventMakeFirst", "eventMakeLast"] as const) {
      const win = sandbox();
      const once = vi.fn();
      const other = vi.fn();
      win.eventOn("probe", other);
      win.eventOnce("probe", once);
      win[move]("probe", once);
      await win.eventEmit("probe");
      await win.eventEmit("probe");
      expect(once).toHaveBeenCalledTimes(1);
      expect(other).toHaveBeenCalledTimes(2);
    }
  });

  it("還沒註冊的函式 eventMakeFirst 排最前、eventMakeLast 排最後", async () => {
    const win = sandbox();
    const calls: string[] = [];
    win.eventOn("probe", () => calls.push("A"));
    win.eventMakeFirst("probe", () => calls.push("B"));
    win.eventMakeLast("probe", () => calls.push("C"));
    await win.eventEmit("probe");
    expect(calls).toEqual(["B", "A", "C"]);
  });

  it("卡片換掉 window._ 之後，getMvuVariable 與 initializeGlobal／waitGlobalInitialized 仍用內嵌那份 lodash", async () => {
    const win = sandbox();
    win._ = { get: () => "假的", has: () => false, set: () => {} };
    expect(win.Mvu.getMvuVariable({ stat_data: { a: { b: 3 } } }, "a.b")).toBe(3);
    win.initializeGlobal("Later.ready", true);
    expect((win.Later as { ready: boolean }).ready).toBe(true);
    await expect(win.waitGlobalInitialized("Later.ready")).resolves.toBeUndefined();
  });

  it("eventEmit 參數原樣傳：函式與循環參照都不拷貝", async () => {
    const win = sandbox();
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    const callback = () => "ok";
    const seen: unknown[] = [];
    win.eventOn("probe", (...args) => {
      seen.push(...args);
    });
    await win.eventEmit("probe", callback, cyclic);
    expect(seen[0]).toBe(callback);
    expect(seen[1]).toBe(cyclic);
  });
});
