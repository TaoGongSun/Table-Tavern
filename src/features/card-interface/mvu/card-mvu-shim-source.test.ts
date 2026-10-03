// 沙盒寫入函式（包 2a，計畫 8.5、8.6）：酒館助手變數函式與 MVU 寫入 API 的沙盒端行為。讀取與事件的測試在
// card-mvu-shim.test.ts。
import { describe, expect, it, vi } from "vitest";
import { buildMvuShimSource } from "./card-mvu-shim-source";
import { runEval } from "./card-mvu-parse-engine";
import { type CardMvu } from "./card-mvu-shim";

const lodashName = "lodash";
const lodash: unknown = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

type Table = Record<string, unknown>;
type Listener = (event: { source: unknown; data: unknown }) => void;
interface Posted {
  kind: string;
  token: string;
  requestId: string;
  floor: number;
  target: string;
  payload: string;
}
interface Sandbox {
  getVariables: (option?: unknown) => Table;
  getAllVariables: () => Table;
  replaceVariables: (vars: unknown, option?: unknown) => unknown;
  updateVariablesWith: (updater: (vars: Table) => unknown, option?: unknown) => unknown;
  insertOrAssignVariables: (vars: unknown, option?: unknown) => Table;
  insertVariables: (vars: unknown, option?: unknown) => Table;
  deleteVariable: (path: string, option?: unknown) => { variables: Table; delete_occurred: boolean };
  eventOn: (name: string, fn: (...args: unknown[]) => unknown) => void;
  Mvu: {
    events: Record<string, string>;
    getMvuData: (option?: unknown) => Table;
    replaceMvuData: (data: unknown, option?: unknown) => Promise<void>;
    setMvuVariable: (data: unknown, path: string, value: unknown, option?: unknown) => Promise<boolean>;
  };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
const M = { type: "message" } as const;

/** 樓 0（GM，有表）、樓 1（玩家，沒有表）、樓 2（system，有表），本樓＝樓 1 */
const snapshot: CardMvu = {
  currentId: 1,
  latestId: 1,
  states: [{ stat_data: { 金: 1, 物品: ["劍"] }, schema: { k: 1 } }, {}, { stat_data: { 系統: true } }],
  floorState: [0, 1, 2],
  targets: [
    { key: "e0", rev: "r0" },
    { key: "@1", rev: null },
    { key: "e2", rev: "r2" },
  ],
  active: true,
  generation: 3,
  scene: 0,
  layers: {},
  characterId: "c1",
  macros: { user: "玩家", char: null },
};

function sandbox(mvu: CardMvu = snapshot) {
  const posted: Posted[] = [];
  const listeners: Listener[] = [];
  const parent = { postMessage: (message: Posted) => posted.push(message) };
  const win = {
    parent,
    _: lodash,
    addEventListener(type: string, fn: Listener) {
      if (type === "message") listeners.push(fn);
    },
  };
  new Function("window", buildMvuShimSource(mvu, "tok"))(win);
  const deliver = async (data: Record<string, unknown>) => {
    for (const listener of listeners) listener({ source: parent, data: { source: "table-tavern-host", token: "tok", ...data } });
    await flush();
    await flush();
  };
  return {
    win: win as unknown as Sandbox,
    posted,
    settle: (results: unknown[], authority?: unknown, migrate?: unknown) =>
      deliver({ kind: "mvu-settle", results, authority, migrate }),
    push: (next: CardMvu) => deliver({ kind: "chat", mvu: next }),
  };
}

describe("沙盒讀取（8.2）", () => {
  it("逐樓讀自己的表：沒有表回 {}、明確表原樣（缺 display 就是缺）", () => {
    const { win } = sandbox();
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ stat_data: { 金: 1, 物品: ["劍"] }, schema: { k: 1 } });
    expect(win.getVariables({ ...M, message_id: 1 })).toEqual({});
    expect(win.Mvu.getMvuData({ ...M, message_id: -1 })).toEqual({ stat_data: { 系統: true } });
    // 讀取 'latest' 排除 system
    expect(win.getVariables(M)).toEqual({});
  });

  it("getAllVariables：0 樓到本樓依序頂層 assign，沒有表的樓帶出前樓的值", () => {
    const { win } = sandbox();
    expect(win.getAllVariables()).toEqual({ stat_data: { 金: 1, 物品: ["劍"] }, schema: { k: 1 } });
    const vars = win.getAllVariables();
    (vars.stat_data as Table).金 = 9;
    expect((win.getAllVariables().stat_data as Table).金).toBe(1);
  });
});

describe("沙盒寫入（8.6）", () => {
  it("replaceVariables：同步、回 void、本地立即換；'latest' 寫最後一樓（含 system）；整張表送宿主", () => {
    const { win, posted } = sandbox();
    expect(win.replaceVariables({ stat_data: { x: 1 } }, M)).toBeUndefined();
    expect(win.getVariables({ ...M, message_id: 2 })).toEqual({ stat_data: { x: 1 } });
    expect(posted).toHaveLength(1);
    expect(posted[0]).toMatchObject({ kind: "mvu-write", token: "tok", floor: 2, target: "e2", payload: '{"stat_data":{"x":1}}' });
    // 寫入沒有表的樓 1：目標就是樓 1，不因讀值來自樓 0 寫到樓 0
    win.replaceVariables({ b: 1 }, { ...M, message_id: 1 });
    expect(posted[1]).toMatchObject({ floor: 1, target: "@1" });
    expect(win.getAllVariables()).toEqual({ stat_data: { 金: 1, 物品: ["劍"] }, schema: { k: 1 }, b: 1 });
    expect(() => win.replaceVariables({}, { ...M, message_id: 3 })).toThrow(/超出範圍/);
  });

  it("insertOrAssign 新值蓋舊值且陣列整個取代；insert 既有值優先；兩者回新表", () => {
    const { win, posted } = sandbox();
    const assigned = win.insertOrAssignVariables({ stat_data: { 金: 2, 物品: ["盾"] } }, { ...M, message_id: 0 });
    expect(assigned).toEqual({ stat_data: { 金: 2, 物品: ["盾"] }, schema: { k: 1 } });
    const inserted = win.insertVariables({ stat_data: { 金: 99, 新: 1 }, other: 1 }, { ...M, message_id: 0 });
    expect(inserted).toEqual({ stat_data: { 金: 2, 物品: ["盾"], 新: 1 }, schema: { k: 1 }, other: 1 });
    expect(JSON.parse(posted[1].payload)).toEqual(inserted);
  });

  it("deleteVariable 回 { variables, delete_occurred }，缺路徑也可能是 true（照 lodash unset）", () => {
    const { win } = sandbox();
    const removed = win.deleteVariable("stat_data.金", { ...M, message_id: 0 });
    expect(removed).toEqual({ variables: { stat_data: { 物品: ["劍"] }, schema: { k: 1 } }, delete_occurred: true });
    expect(win.deleteVariable("沒有.這條", { ...M, message_id: 0 }).delete_occurred).toBe(true);
  });

  it("updateVariablesWith：Promise 版 resolve 後才算本地新值；讀用讀取的樓號規則、寫用寫入的", async () => {
    const { win, posted } = sandbox();
    let release: (value: Table) => void = () => {};
    const result = win.updateVariablesWith(
      () =>
        new Promise<Table>((resolve) => {
          release = resolve;
        }),
      { ...M, message_id: 0 },
    ) as Promise<Table>;
    expect(posted).toHaveLength(0);
    release({ v: 1 });
    expect(await result).toEqual({ v: 1 });
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ v: 1 });
    expect(posted).toHaveLength(1);
  });

  it("卡片換掉 window 上的變數函式，insert／delete 照樣用墊片自己的", () => {
    const { win } = sandbox();
    (win as unknown as Record<string, unknown>).getVariables = () => ({ 假的: 1 });
    expect(win.insertVariables({ a: 1 }, { ...M, message_id: 0 })).toMatchObject({ a: 1, schema: { k: 1 } });
  });

  it("非有限數、循環參照整批拒絕、本地不動、不送；不能寫的樓（空桌開場白）拒絕", async () => {
    const { win, posted } = sandbox();
    await expect(win.Mvu.replaceMvuData({ stat_data: { x: Infinity } }, { ...M, message_id: 0 })).rejects.toThrow(
      /invalid-value/,
    );
    const cyclic: Table = {};
    cyclic.self = cyclic;
    await expect(win.Mvu.replaceMvuData(cyclic, { ...M, message_id: 0 })).rejects.toThrow(/invalid-value/);
    expect(posted).toHaveLength(0);
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual(snapshot.states[0]);
    const empty = sandbox({ currentId: 0, latestId: 0, states: [{ stat_data: {} }], floorState: [0], targets: [null], active: false, generation: 3, scene: 0, layers: {}, characterId: "c1", macros: { user: "玩家", char: null } });
    await expect(empty.win.Mvu.replaceMvuData({}, M)).rejects.toThrow(/no-target/);
    const report = vi.spyOn(console, "error").mockImplementation(() => {});
    empty.win.replaceVariables({}, M);
    await flush();
    expect(report).toHaveBeenCalled();
    report.mockRestore();
  });
});

describe("Mvu.replaceMvuData：宿主確認才 resolve（app 加強語意）", () => {
  it("確認 resolve；被拒 reject 並換成權威值、發外部變動事件", async () => {
    const box = sandbox({ ...snapshot, currentId: 0 });
    const { win, posted } = box;
    const ok = win.Mvu.replaceMvuData({ stat_data: { 金: 5 } }, { ...M, message_id: 0 });
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "r1" }]);
    await expect(ok).resolves.toBeUndefined();
    const events: unknown[][] = [];
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_STARTED, (...args) => void events.push(["started", ...args]));
    win.eventOn(win.Mvu.events.SINGLE_VARIABLE_UPDATED, (...args) => void events.push(["single", ...args.slice(1)]));
    win.eventOn(win.Mvu.events.VARIABLE_UPDATE_ENDED, (...args) => void events.push(["ended", ...args]));
    const bad = win.Mvu.replaceMvuData({ stat_data: { 金: 6 } }, { ...M, message_id: 0 });
    void bad.catch(() => {});
    await box.settle([{ requestId: posted[1].requestId, ok: false, error: "stale" }], {
      key: "e0",
      table: { stat_data: { 金: 7 } },
      rev: "r9",
    });
    await expect(bad).rejects.toThrow(/stale/);
    expect(win.getAllVariables()).toEqual({ stat_data: { 金: 7 } });
    expect(events).toEqual([
      ["started", { stat_data: { 金: 6 } }],
      ["single", "金", 6, 7],
      ["ended", { stat_data: { 金: 7 } }, { stat_data: { 金: 6 } }],
    ]);
  });

  it("推送：有在途寫入的目標照留本地值；確認的版本到了才換回快照；快照還舊就照留", async () => {
    const box = sandbox();
    const { win, posted } = box;
    const pending = win.Mvu.replaceMvuData({ stat_data: { 金: 5 } }, { ...M, message_id: 0 });
    await box.push(snapshot);
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ stat_data: { 金: 5 } });
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "r1" }]);
    await pending;
    // 確認了，但推來的快照還是舊版本（r0）：照留本地值
    await box.push(snapshot);
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ stat_data: { 金: 5 } });
    // 快照追上 r1：改讀快照
    const updated: CardMvu = {
      ...snapshot,
      states: [{ stat_data: { 金: 5, 後端: true } }, {}, { stat_data: { 系統: true } }],
      targets: [{ key: "e0", rev: "r1" }, snapshot.targets[1], snapshot.targets[2]],
    };
    await box.push(updated);
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ stat_data: { 金: 5, 後端: true } });
  });

  it("寫入帶底版、世代與幕；確認 r1 之後直接收到跳版的 r2 也改讀快照", async () => {
    const box = sandbox();
    const { win, posted } = box;
    const first = win.Mvu.replaceMvuData({ v: 1 }, { ...M, message_id: 0 });
    expect(posted[0]).toMatchObject({ base: "r0", generation: 3, scene: 0 });
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "r1" }]);
    await first;
    // 確認後的下一筆以確認的版本為底
    const second = win.Mvu.replaceMvuData({ v: 2 }, { ...M, message_id: 0 });
    expect(posted[1]).toMatchObject({ base: "r1" });
    await box.settle([{ requestId: posted[1].requestId, ok: true, rev: "r2" }]);
    await second;
    // 別處寫入的更新版 r3（跳過 r2）：改讀快照
    await box.push({
      ...snapshot,
      states: [{ 別處: 1 }, {}, { stat_data: { 系統: true } }],
      targets: [{ key: "e0", rev: "r3" }, snapshot.targets[1], snapshot.targets[2]],
    });
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ 別處: 1 });
  });

  it("舊事件（@位置）第一次寫入配到 id：本地值與在途寫入搬到新 key，之後的寫入直接寫到 id", async () => {
    const box = sandbox();
    const { win, posted } = box;
    const first = win.Mvu.replaceMvuData({ v: 1 }, { ...M, message_id: 1 });
    const second = win.Mvu.replaceMvuData({ v: 2 }, { ...M, message_id: 1 });
    expect(posted.map((message) => message.target)).toEqual(["@1", "@1"]);
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "r1" }], undefined, { from: "@1", to: "E1" });
    await first;
    // 宿主推來的快照已改用 id：第二筆還在途，照留本地值
    const migrated: CardMvu = {
      ...snapshot,
      states: [snapshot.states[0], { v: 1 }, snapshot.states[2]],
      targets: [snapshot.targets[0], { key: "E1", rev: "r1" }, snapshot.targets[2]],
    };
    await box.push(migrated);
    expect(win.getVariables({ ...M, message_id: 1 })).toEqual({ v: 2 });
    await box.settle([{ requestId: posted[1].requestId, ok: true, rev: "r2" }]);
    await second;
    const third = win.Mvu.replaceMvuData({ v: 3 }, { ...M, message_id: 1 });
    expect(posted[2]).toMatchObject({ target: "E1", base: "r2" });
    await box.settle([{ requestId: posted[2].requestId, ok: true, rev: "r3" }]);
    await third;
  });

  it("結算與推送驗 token 與形狀：別的殼送來的不理", async () => {
    const box = sandbox();
    const { win, posted } = box;
    const pending = win.Mvu.replaceMvuData({ v: 1 }, { ...M, message_id: 0 });
    let settled = false;
    void pending.then(() => (settled = true));
    await box.settle([{ requestId: "tok:999", ok: true }]);
    expect(settled).toBe(false);
    await box.push({ ...snapshot, targets: [] } as CardMvu);
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual({ v: 1 });
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "r1" }]);
    expect(settled).toBe(true);
  });
});

describe("Mvu.setMvuVariable：純沙盒內運算", () => {
  it("不送宿主；路徑不存在回 false；長度二陣列改第 0 項；不轉 Number；display／delta 只更新 $internal 那份", async () => {
    const { win, posted } = sandbox();
    const internal = { display_data: {}, delta_data: {} };
    const data = { stat_data: { hp: [5, "血量"], pair: [1, 2], 名: "甲", $internal: internal }, display_data: {} };
    expect(await win.Mvu.setMvuVariable(data, "沒有", 1)).toBe(false);
    expect(await win.Mvu.setMvuVariable(data, "hp", "7", { reason: "受傷" })).toBe(true);
    expect(data.stat_data.hp).toEqual(["7", "血量"]);
    expect(await win.Mvu.setMvuVariable(data, "pair", 9)).toBe(true);
    expect(data.stat_data.pair).toEqual([9, 2]);
    expect(await win.Mvu.setMvuVariable(data, "名", "乙")).toBe(true);
    expect(internal.display_data).toEqual({ hp: "5->7 (受傷)", pair: "1->9 ", 名: "甲->乙 " });
    expect(internal.delta_data).toEqual(internal.display_data);
    expect(data.display_data).toEqual({});
    expect(posted).toHaveLength(0);
  });

  it("is_recursive 才發 SINGLE_VARIABLE_UPDATED，參數不拷貝", async () => {
    const { win } = sandbox();
    const seen: unknown[][] = [];
    win.eventOn(win.Mvu.events.SINGLE_VARIABLE_UPDATED, (...args) => void seen.push(args));
    const data = { stat_data: { a: 1 } };
    await win.Mvu.setMvuVariable(data, "a", 2);
    expect(seen).toHaveLength(0);
    await win.Mvu.setMvuVariable(data, "a", 3, { is_recursive: true });
    expect(seen).toHaveLength(1);
    expect(seen[0][0]).toBe(data.stat_data);
    expect(seen[0].slice(1)).toEqual(["a", 2, 3]);
  });
});

describe("沙盒非 message 層（8.7）", () => {
  const layered: CardMvu = {
    ...snapshot,
    layers: {
      chat: { rev: "c1", vars: { c: 1 } },
      "character:c1": { rev: "h1", vars: { h: 1, c: 0 } },
      global: { rev: "g1", vars: { g: 1, c: -1 } },
      preset: { rev: null, vars: {} },
      "script:s1": { rev: "s1", vars: { s: [1, 2] } },
      "extension:e/1": { rev: "x1", vars: { x: { y: 1 } } },
    },
  };
  const options = {
    chat: {},
    character: { type: "character" },
    global: { type: "global" },
    script: { type: "script", script_id: "s1" },
    extension: { type: "extension", extension_id: "e/1" },
  };

  it("預設層是 chat；各層讀自己的表、沒列的層是空表；拿到的是拷貝", () => {
    const { win } = sandbox(layered);
    expect(win.getVariables()).toEqual({ c: 1 });
    expect(win.getVariables({ type: "chat" })).toEqual({ c: 1 });
    expect(win.getVariables(options.character)).toEqual({ h: 1, c: 0 });
    expect(win.getVariables(options.global)).toEqual({ g: 1, c: -1 });
    expect(win.getVariables({ type: "preset" })).toEqual({});
    expect(win.getVariables({ type: "preset", preset_name: "in_use" })).toEqual({});
    expect(win.getVariables(options.script)).toEqual({ s: [1, 2] });
    expect(win.getVariables(options.extension)).toEqual({ x: { y: 1 } });
    expect(win.getVariables({ type: "script", script_id: "沒有" })).toEqual({});
    expect(win.Mvu.getMvuData({ type: "global" })).toEqual({ g: 1, c: -1 });
    (win.getVariables() as Table).c = 99;
    expect(win.getVariables()).toEqual({ c: 1 });
  });

  it("沒給 ID 的 script／extension、別張卡的 character、不認得的層與預設集拋錯", () => {
    const { win } = sandbox(layered);
    expect(() => win.getVariables({ type: "script" })).toThrow(/script_id/);
    expect(() => win.getVariables({ type: "extension", extension_id: "" })).toThrow(/extension_id/);
    expect(() => win.getVariables({ type: "script", script_id: "字".repeat(257) })).toThrow(/script_id/);
    expect(() => win.getVariables({ type: "character", character_name: "別人" })).toThrow(/目前這張卡/);
    expect(() => win.getVariables({ type: "preset", preset_name: "別的" })).toThrow(/預設集不存在/);
    expect(() => win.getVariables({ type: "nope" })).toThrow(/不支援的變數層/);
    expect(() => win.getVariables(5 as never)).toThrow(/物件/);
  });

  it("getAllVariables：全域 → 角色 → 聊天 → 各樓，後者蓋前者（頂層 assign）", () => {
    const { win } = sandbox(layered);
    expect(win.getAllVariables()).toEqual({
      g: 1,
      h: 1,
      c: 1,
      stat_data: { 金: 1, 物品: ["劍"] },
      schema: { k: 1 },
    });
  });

  it("寫入：本地立即換、整張表送宿主（key 是層名＋ID、base 是該層版本）；各層互不影響", () => {
    const { win, posted } = sandbox(layered);
    win.replaceVariables({ c: 2 });
    expect(win.getVariables()).toEqual({ c: 2 });
    expect(posted[0]).toMatchObject({ kind: "mvu-write", target: "chat", base: "c1", payload: '{"c":2}', generation: 3 });
    win.replaceVariables({ n: 1 }, { type: "preset" });
    expect(posted[1]).toMatchObject({ target: "preset", base: null });
    win.replaceVariables({ v: 1 }, options.extension);
    expect(posted[2]).toMatchObject({ target: "extension:e/1", base: "x1" });
    win.replaceVariables({ v: 1 }, { type: "script", script_id: "新的" });
    expect(posted[3]).toMatchObject({ target: "script:新的", base: null });
    win.replaceVariables({ v: 1 }, options.character);
    expect(posted[4]).toMatchObject({ target: "character:c1", base: "h1" });
    expect(win.getVariables(options.global)).toEqual({ g: 1, c: -1 });
    expect(win.getVariables({ ...M, message_id: 0 })).toEqual(snapshot.states[0]);
  });

  it("insertOrAssign／insert／delete／updateVariablesWith 對非 message 層同語意", () => {
    const { win, posted } = sandbox(layered);
    expect(win.insertOrAssignVariables({ s: [9], t: 1 }, options.script)).toEqual({ s: [9], t: 1 });
    expect(win.insertVariables({ s: [0], u: 2 }, options.script)).toEqual({ s: [9], t: 1, u: 2 });
    expect(win.deleteVariable("t", options.script)).toEqual({ variables: { s: [9], u: 2 }, delete_occurred: true });
    expect(JSON.parse(posted[posted.length - 1].payload)).toEqual({ s: [9], u: 2 });
    expect(win.getVariables(options.script)).toEqual({ s: [9], u: 2 });
  });

  it("確認落檔才 resolve；之後推來新版快照改讀快照；被拒換成權威值", async () => {
    const box = sandbox(layered);
    const { win, posted } = box;
    const first = win.Mvu.replaceMvuData({ c: 2 }, {});
    await box.settle([{ requestId: posted[0].requestId, ok: true, rev: "c2" }]);
    await first;
    await box.push({ ...layered, layers: { ...layered.layers, chat: { rev: "c2", vars: { c: 2 } } } });
    expect(win.getVariables()).toEqual({ c: 2 });
    // 下一筆寫入以確認的版本為底
    void win.Mvu.replaceMvuData({ c: 3 }, {}).catch(() => {});
    expect(posted[1]).toMatchObject({ target: "chat", base: "c2" });
    await box.settle([{ requestId: posted[1].requestId, ok: false, error: "stale" }], {
      key: "chat",
      table: { c: 7 },
      rev: "c9",
    });
    expect(win.getVariables()).toEqual({ c: 7 });
    expect(posted).toHaveLength(2);
  });

  it("讀取失敗的層與類別：讀寫與 getAllVariables 都拋錯（不當空表）；其他層照常；找不到所屬卡時 character 層拋錯", () => {
    const { win, posted } = sandbox({
      ...layered,
      layers: {
        ...layered.layers,
        global: { rev: null, vars: {}, error: "corrupt-file" },
        "extension:": { rev: null, vars: {}, error: "load-failed" },
      },
    });
    expect(() => win.getVariables({ type: "global" })).toThrow(/讀取失敗（corrupt-file）/);
    expect(() => win.replaceVariables({}, { type: "global" })).toThrow(/讀取失敗/);
    expect(() => win.getAllVariables()).toThrow(/讀取失敗/);
    expect(() => win.getVariables({ type: "extension", extension_id: "任何" })).toThrow(/load-failed/);
    expect(win.getVariables()).toEqual({ c: 1 });
    expect(win.getVariables(options.script)).toEqual({ s: [1, 2] });
    expect(posted).toHaveLength(0);
    const unknown = sandbox({ ...layered, characterId: "" });
    expect(() => unknown.win.getVariables({ type: "character" })).toThrow(/唯一所屬的卡/);
    expect(unknown.win.getVariables()).toEqual({ c: 1 });
  });

  it("寫入在飛時推來的舊版本快照不蓋掉本地值", async () => {
    const box = sandbox(layered);
    void box.win.Mvu.replaceMvuData({ c: 5 }, {}).catch(() => {});
    await box.push(layered);
    expect(box.win.getVariables()).toEqual({ c: 5 });
  });

  it("非有限數整批拒絕、不送；快照形狀不對整份不收", async () => {
    const box = sandbox(layered);
    await expect(box.win.Mvu.replaceMvuData({ a: Infinity }, {})).rejects.toThrow(/invalid-value/);
    expect(box.posted).toHaveLength(0);
    await box.push({ ...layered, layers: { chat: { rev: 5, vars: {} } } } as unknown as CardMvu);
    expect(box.win.getVariables()).toEqual({ c: 1 });
    await box.push({ ...layered, characterId: 3 } as unknown as CardMvu);
    expect(box.win.getVariables()).toEqual({ c: 1 });
  });
});

describe("自製測試卡 mvu-write-probe（GUI 驗收 6a 用的 fixture）", () => {
  it("殼在整份文件裡跑得起來：讀到值與型別，按鈕送出卡片寫入，六層各讀各寫，parseMessage 預覽走值解析", async () => {
    const { readFileSync } = await import("node:fs");
    const { applyScripts, buildShellDocument, extractShell } = await import("../interface-card");
    const card = JSON.parse(
      readFileSync(new URL("../../../../scripts/harness-fixtures/mvu-write-probe.json", import.meta.url), "utf8"),
    ) as { data: { extensions: { regex_scripts: Record<string, unknown>[] } } };
    const raw = card.data.extensions.regex_scripts[0];
    const shell = extractShell(
      applyScripts("探針回合正文\n\n<StatusPlaceHolderImpl/>", [
        {
          name: "probe",
          find_regex: String(raw.findRegex),
          replace_string: String(raw.replaceString),
          trim_strings: [],
          min_depth: null,
          max_depth: null,
        },
      ]),
    )!;
    const mvu: CardMvu = {
      currentId: 0,
      latestId: 0,
      states: [{ stat_data: { 欄位: { 錢: 100, 血量: [5, "生命值"], 名字: "阿濤", 編號: "123", 暫存: "刪我" } } }],
      floorState: [0],
      targets: [{ key: "e0", rev: "r0" }],
      active: true,
      generation: 3,
      scene: 0,
      layers: { chat: { rev: "c0", vars: { probe: 1 } }, global: { rev: null, vars: {} } },
      characterId: "c1",
      macros: { user: "玩家", char: null },
    };
    const chat = { currentId: 0, floors: [{ name: "GM", role: "assistant" as const, message: "探針回合正文" }] };
    const name = "jsdom";
    const { JSDOM } = (await import(/* @vite-ignore */ name)) as {
      JSDOM: new (html: string, options: object) => { window: Window & typeof globalThis };
    };
    const win = new JSDOM(buildShellDocument(shell, {}, { chat, token: "tok", mvu }), {
      runScripts: "dangerously",
      pretendToBeVisual: true,
    }).window;
    const sent: { kind?: string; payload?: string }[] = [];
    win.addEventListener("message", (event) => {
      const data = event.data as { source?: string; kind?: string; requestId?: string; op?: "value" | "patch"; text?: string };
      if (data.source !== "table-tavern-card") return;
      sent.push(data as never);
      // 宿主的值解析 Worker 換成同進程的引擎
      if (data.kind === "mvu-eval") {
        // 在頁面自己的 realm 裡造事件，source 才會是頁面認得的 parent
        const reply = { source: "table-tavern-host", token: "tok", kind: "mvu-eval-result", requestId: data.requestId, ...runEval(data.op!, data.text!) };
        // jsdom 的 MessageEvent 不收 source 初始值，事件造好後再補上；頂層頁面的真 parent 就是 window（window.parent 已被誘餌換掉）
        win.eval(
          `var e = new MessageEvent("message", { data: JSON.parse(${JSON.stringify(JSON.stringify(reply))}) }); Object.defineProperty(e, "source", { value: window }); window.dispatchEvent(e);`,
        );
      }
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    const text = (id: string) => win.document.getElementById(id)?.textContent ?? "";
    expect(text("money")).toBe("100　typeof number");
    expect(text("hp")).toBe('[5,"生命值"]　typeof array');
    expect(text("code")).toBe('"123"　typeof string');
    expect(text("chat")).toBe("1　typeof number");
    expect(text("global")).toBe("（無）");
    expect(text("script")).toBe("（無）");
    (win.document.getElementById("b-mvu") as HTMLButtonElement).click();
    (win.document.getElementById("b-parse") as HTMLButtonElement).click();
    await new Promise((resolve) => setTimeout(resolve, 150));
    const write = sent.find((message) => message.kind === "mvu-write")!;
    expect(JSON.parse(write.payload!).stat_data.欄位.血量).toEqual([4, "生命值"]);
    expect(text("hp")).toBe('[4,"生命值"]　typeof array');
    // parseMessage 預覽：數學式、{{user}}、JSONPatch 都走 Worker 值解析；只算不寫（沒有多出 mvu-write）
    const log = text("log");
    expect(log).toContain('預覽 stat：{"錢":42,"血量":[2,"生命值"],"名字":"玩家的探針"');
    expect(log).toContain("parseMessage：已落檔");
    expect(sent.filter((message) => message.kind === "mvu-write")).toHaveLength(1);
    (win.document.getElementById("b-layers") as HTMLButtonElement).click();
    await new Promise((resolve) => setTimeout(resolve, 30));
    const layerWrites = sent.filter((message) => message.kind === "mvu-write").slice(1) as unknown as {
      target: string;
      base: string | null;
      payload: string;
    }[];
    expect(layerWrites.map((message) => [message.target, message.base, JSON.parse(message.payload)])).toEqual([
      ["chat", "c0", { probe: 2 }],
      ["character:c1", null, { probe: 1 }],
      ["global", null, { probe: 1 }],
      ["preset", null, { probe: 1 }],
      ["script:probe-script", null, { probe: 1 }],
      ["extension:probe-ext", null, { probe: 1 }],
    ]);
    expect(text("chat")).toBe("2　typeof number");
    expect(text("extension")).toBe("1　typeof number");
    win.close();
  });
});
