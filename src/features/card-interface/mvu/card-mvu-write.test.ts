import { describe, expect, it, vi } from "vitest";
import { createMvuWriteQueue, layerBlocked, parseLayerKey, validateTable, type Authority, type CardWriteResult, type Migration, type SettleResult } from "./card-mvu-write";
import { type TranscriptEvent } from "../../../shared/contracts/backend-contracts";

describe("validateTable：寫入上限（與後端同一組）", () => {
  it("合法表通過；頂層不是物件、壞 JSON 拒絕", () => {
    expect(validateTable('{"stat_data":{"a":[1,"說明"]}}')).toBeNull();
    expect(validateTable("[1]")).toBe("not-object");
    expect(validateTable("{")).toBe("invalid-json");
  });
  it("鍵：非空、≤ 256 字（以字元算）", () => {
    expect(validateTable('{"":1}')).toBe("empty-key");
    expect(validateTable(JSON.stringify({ ["k".repeat(257)]: 1 }))).toBe("key-too-long");
    expect(validateTable(JSON.stringify({ ["鍵".repeat(256)]: 1 }))).toBeNull();
  });
  it("深度 ≤ 32、字串 ≤ 64 KB、子項 ≤ 10000、節點 ≤ 200000、整張 ≤ 2 MB", () => {
    const nest = (depth: number) => `${'{"a":'.repeat(depth)}1${"}".repeat(depth)}`;
    expect(validateTable(nest(31))).toBeNull();
    expect(validateTable(nest(32))).toBe("too-deep");
    expect(validateTable(JSON.stringify({ s: "a".repeat(64 * 1024 + 1) }))).toBe("string-too-long");
    expect(validateTable(JSON.stringify({ l: new Array(10_001).fill(0) }))).toBe("too-many-children");
    const many = Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`k${i}`, new Array(10_000).fill(0)]));
    expect(validateTable(JSON.stringify(many))).toBe("too-many-nodes");
    expect(validateTable(JSON.stringify({ s: "a".repeat(2 * 1024 * 1024) }))).toBe("too-large");
  });
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
const last = <T,>(list: T[]): T | undefined => list[list.length - 1];

/** 可手動結算的假後端：每次 send 排一筆，測試決定它何時、怎麼回 */
function harness() {
  const sends: { key: string; rev: string | null; payload: string; resolve: (result: CardWriteResult) => void; reject: (error: unknown) => void }[] = [];
  const settled: { results: SettleResult[]; authority?: Authority; migrate?: Migration }[] = [];
  const committed: TranscriptEvent[] = [];
  const revs = new Map<string, string | null>([
    ["A", "r0"],
    ["B", null],
  ]);
  const queue = createMvuWriteQueue({
    revOf: (key) => revs.get(key),
    tableOf: (key) => (key === "A" ? { stat_data: { 權威: 1 } } : null),
    send: (key, rev, payload, identity) =>
      new Promise<CardWriteResult>((resolve, reject) => {
        expect(identity).toEqual({ generation: 3, scene: 0 });
        sends.push({ key, rev, payload, resolve, reject });
      }),
    layerCommitted: () => {},
    committed: (_key, event) => {
      committed.push(event);
      if (event.id) revs.set(event.id, event.vars_rev ?? null);
    },
    settle: (results, authority, migrate) => settled.push({ results, authority, migrate }),
  });
  const write = (requestId: string, target: string, value: unknown, base = revs.get(target) ?? null) =>
    queue.enqueue({ requestId, target, payload: JSON.stringify(value), base, generation: 3, scene: 0 });
  const ok = (key: string, rev: string): CardWriteResult => ({
    status: "ok",
    event: { ts: "t", speaker_id: "", speaker_name: "GM", kind: "narration", text: "x", id: key, vars_rev: rev },
  });
  const outcome = (requestId: string) =>
    settled.flatMap((entry) => entry.results).filter((result) => result.requestId === requestId);
  return { queue, sends, settled, committed, write, ok, outcome };
}

describe("非 message 層寫入目標（8.7）", () => {
  it("parseLayerKey：層名、帶 ID 的層（ID 可含冒號與任意字元）；事件 key 不是層", () => {
    expect(parseLayerKey("chat")).toEqual({ layer: "chat", id: null });
    expect(parseLayerKey("global")).toEqual({ layer: "global", id: null });
    expect(parseLayerKey("preset")).toEqual({ layer: "preset", id: null });
    expect(parseLayerKey("character:c1")).toEqual({ layer: "character", id: "c1" });
    expect(parseLayerKey("script:a:b\n字")).toEqual({ layer: "script", id: "a:b\n字" });
    expect(parseLayerKey("extension:x")).toEqual({ layer: "extension", id: "x" });
    for (const key of ["script:", "@3", "01JABCDEFGHJKMNPQRSTVWXYZ0", "chat2", "global:x"]) {
      expect(parseLayerKey(key)).toBeNull();
    }
  });

  it("layerBlocked：沒讀回、該層或所屬類別標了 error 都擋；正常層不擋", () => {
    const layers: Record<string, { error?: string }> = { chat: {}, "script:": { error: "x" }, global: { error: "y" } };
    expect(layerBlocked(null, "chat")).toBe(true);
    expect(layerBlocked(layers, "chat")).toBe(false);
    expect(layerBlocked(layers, "global")).toBe(true);
    expect(layerBlocked(layers, "script:任何")).toBe(true);
    expect(layerBlocked(layers, "extension:任何")).toBe(false);
  });

  it("layer_ok：通知宿主新版本與落檔的表、結算 resolve、鏈用新版本續送；不是事件所以不搬家", async () => {
    const layerCommitted = vi.fn();
    const sends: { key: string; rev: string | null; payload: string; resolve: (r: CardWriteResult) => void }[] = [];
    const settled: { results: SettleResult[]; migrate?: Migration }[] = [];
    const queue = createMvuWriteQueue({
      revOf: () => null,
      tableOf: () => null,
      send: (key, rev, payload) =>
        new Promise<CardWriteResult>((resolve) => sends.push({ key, rev, payload, resolve })),
      committed: () => {},
      layerCommitted,
      settle: (results, _authority, migrate) => settled.push({ results, migrate }),
    });
    const write = (requestId: string, value: unknown) =>
      queue.enqueue({ requestId, target: "chat", payload: JSON.stringify(value), base: "c0", generation: 3, scene: 0 });
    write("l1", { n: 1 });
    write("l2", { n: 2 });
    expect(sends).toHaveLength(1);
    expect(sends[0]).toMatchObject({ key: "chat", rev: "c0", payload: '{"n":1}' });
    sends[0].resolve({ status: "layer_ok", rev: "c1" });
    await flush();
    expect(layerCommitted).toHaveBeenCalledWith("chat", "c1", '{"n":1}', { generation: 3, scene: 0 });
    expect(settled[0]).toEqual({ results: [{ requestId: "l1", ok: true, rev: "c1" }], migrate: undefined });
    expect(sends[1]).toMatchObject({ rev: "c1", payload: '{"n":2}' });
    sends[1].resolve({ status: "stale", found: true, rev: "c9", table: '{"n":9}' });
    await flush();
    expect(settled[1].results).toEqual([{ requestId: "l2", ok: false, error: "stale" }]);
    expect(settled[1]).toMatchObject({ results: [{ requestId: "l2", ok: false, error: "stale" }] });
  });
});

describe("宿主寫入佇列", () => {
  it("同目標 W1 在飛時 W2、W3 合併成一筆（送最新整張表）；W1 先結算，W2、W3 一起結算", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 });
    h.write("w2", "A", { v: 2 });
    h.write("w3", "A", { v: 3 });
    expect(h.sends).toHaveLength(1);
    expect(h.sends[0]).toMatchObject({ key: "A", rev: "r0", payload: '{"v":1}' });
    h.sends[0].resolve(h.ok("A", "r1"));
    await flush();
    expect(h.outcome("w1")).toEqual([{ requestId: "w1", ok: true, rev: "r1" }]);
    expect(h.outcome("w2")).toEqual([]);
    // 第二筆帶著剛確認的新版本、內容是 W3
    expect(h.sends).toHaveLength(2);
    expect(h.sends[1]).toMatchObject({ rev: "r1", payload: '{"v":3}' });
    h.sends[1].resolve(h.ok("A", "r2"));
    await flush();
    expect(h.outcome("w2")).toEqual([{ requestId: "w2", ok: true, rev: "r2" }]);
    expect(h.outcome("w3")).toEqual([{ requestId: "w3", ok: true, rev: "r2" }]);
    expect(h.committed.map((event) => event.vars_rev)).toEqual(["r1", "r2"]);
  });

  it("舊事件（@位置）連寫：W1 確認配到 id 後，在飛期間排著的 W2、W3 改送到新 id，之後的 @位置 寫入也改寫到 id", async () => {
    const h = harness();
    h.write("w1", "@4", { v: 1 }, null);
    h.write("w2", "@4", { v: 2 }, null);
    h.write("w3", "@4", { v: 3 }, null);
    expect(h.sends[0]).toMatchObject({ key: "@4", rev: null });
    h.sends[0].resolve(h.ok("E4", "r1"));
    await flush();
    expect(h.settled[0]).toMatchObject({ results: [{ requestId: "w1", ok: true, rev: "r1" }], migrate: { from: "@4", to: "E4" } });
    expect(h.sends[1]).toMatchObject({ key: "E4", rev: "r1", payload: '{"v":3}' });
    h.sends[1].resolve(h.ok("E4", "r2"));
    await flush();
    expect(h.outcome("w3")).toEqual([{ requestId: "w3", ok: true, rev: "r2" }]);
    // 沙盒還沒收到搬家通知就送出的 @4 寫入：改寫到 E4
    h.write("w4", "@4", { v: 4 }, "r2");
    expect(h.sends[2]).toMatchObject({ key: "E4", rev: "r2" });
  });

  it("結果不屬於現在這桌（gone）：在飛與未送的一起 stale，不推權威值、不換逐字稿", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 });
    h.write("w2", "A", { v: 2 });
    h.sends[0].resolve({ status: "gone" });
    await flush();
    expect(h.outcome("w1")).toEqual([{ requestId: "w1", ok: false, error: "stale" }]);
    expect(h.outcome("w2")).toEqual([{ requestId: "w2", ok: false, error: "stale" }]);
    expect(h.settled.every((entry) => entry.authority === undefined)).toBe(true);
    expect(h.committed).toEqual([]);
    expect(h.sends).toHaveLength(1);
  });

  it("鏈的第一筆用沙盒給的底版當預期版本，不冒用宿主手上較新的版本；鏈中途換了世代或幕的寫入直接 stale", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 }, "r-old");
    expect(h.sends[0].rev).toBe("r-old");
    h.queue.enqueue({ requestId: "w2", target: "A", payload: "{}", base: "r-old", generation: 4, scene: 0 });
    expect(h.outcome("w2")).toEqual([{ requestId: "w2", ok: false, error: "stale" }]);
    h.sends[0].resolve({ status: "stale", found: true, rev: "r0", table: "{}" });
    await flush();
    // 被拒後鏈清掉：下一筆再用自己帶的底版
    h.write("w3", "A", { v: 3 }, "r0");
    expect(h.sends[1].rev).toBe("r0");
  });

  it("不同目標各自送、各自結算，一邊被拒不影響另一邊", async () => {
    const h = harness();
    h.write("a1", "A", { v: 1 });
    h.write("b1", "B", { v: 1 });
    expect(h.sends.map((send) => [send.key, send.rev])).toEqual([
      ["A", "r0"],
      ["B", null],
    ]);
    h.sends[1].resolve({ status: "stale", found: true, rev: "rb", table: '{"b":9}' });
    await flush();
    expect(h.outcome("b1")).toEqual([{ requestId: "b1", ok: false, error: "stale" }]);
    expect(last(h.settled)?.authority).toEqual({ key: "B", table: { b: 9 }, rev: "rb" });
    expect(h.outcome("a1")).toEqual([]);
    h.sends[0].resolve(h.ok("A", "r1"));
    await flush();
    expect(h.outcome("a1")).toEqual([{ requestId: "a1", ok: true, rev: "r1" }]);
  });

  it("W1 成功、W2 被拒：W2 與其後未送的 W3 一起 reject，推回權威值", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 });
    h.write("w2", "A", { v: 2 });
    h.sends[0].resolve(h.ok("A", "r1"));
    await flush();
    h.write("w3", "A", { v: 3 });
    h.sends[1].resolve({ status: "busy", found: true, rev: "r1", table: '{"v":1}' });
    await flush();
    expect(h.outcome("w1")).toEqual([{ requestId: "w1", ok: true, rev: "r1" }]);
    expect(h.outcome("w2")).toEqual([{ requestId: "w2", ok: false, error: "busy" }]);
    expect(h.outcome("w3")).toEqual([{ requestId: "w3", ok: false, error: "busy" }]);
    expect(last(h.settled)?.authority).toEqual({ key: "A", table: { v: 1 }, rev: "r1" });
    expect(h.sends).toHaveLength(2);
  });

  it("後端拒絕附代碼；目標已不在時權威表為 null；呼叫拋錯也算被拒", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 });
    h.sends[0].resolve({ status: "rejected", code: "too-deep", found: false, rev: null, table: null });
    await flush();
    expect(h.outcome("w1")).toEqual([{ requestId: "w1", ok: false, error: "too-deep" }]);
    expect(last(h.settled)?.authority).toEqual({ key: "A", table: null, rev: null });
    h.write("w2", "A", { v: 2 });
    h.sends[1].reject(new Error("ipc"));
    await flush();
    expect(h.outcome("w2")[0].ok).toBe(false);
    expect(h.outcome("w2")[0].error).toMatch(/^error: /);
  });

  it("本地驗證失敗：這筆與前面還沒送的一起拒絕；沒有在飛就立刻推回權威值，有在飛就等它回來再推", async () => {
    const h = harness();
    h.write("big", "A", { s: "a".repeat(64 * 1024 + 1) });
    expect(h.sends).toHaveLength(0);
    expect(h.outcome("big")).toEqual([{ requestId: "big", ok: false, error: "string-too-long" }]);
    expect(last(h.settled)?.authority).toEqual({ key: "A", table: { stat_data: { 權威: 1 } }, rev: "r0" });
    h.write("w1", "A", { v: 1 });
    h.write("w2", "A", { v: 2 });
    h.write("bad", "A", { "": 1 });
    expect(h.outcome("w2")).toEqual([{ requestId: "w2", ok: false, error: "empty-key" }]);
    expect(h.outcome("bad")).toEqual([{ requestId: "bad", ok: false, error: "empty-key" }]);
    expect(last(h.settled)?.authority).toBeUndefined();
    h.sends[0].resolve(h.ok("A", "r1"));
    await flush();
    expect(h.outcome("w1")).toEqual([{ requestId: "w1", ok: true, rev: "r1" }]);
    expect(last(h.settled)?.authority).toEqual({ key: "A", table: { stat_data: { 權威: 1 } }, rev: "r1" });
    expect(h.sends).toHaveLength(1);
  });

  it("關掉：在飛與未送的全部 reject（closed）；在飛那筆後端結果照實換進逐字稿、不再回覆舊殼；之後的寫入直接 closed", async () => {
    const h = harness();
    h.write("w1", "A", { v: 1 });
    h.write("w2", "A", { v: 2 });
    h.write("b1", "B", { v: 1 });
    h.queue.close();
    for (const id of ["w1", "w2", "b1"]) expect(h.outcome(id)).toEqual([{ requestId: id, ok: false, error: "closed" }]);
    const before = h.settled.length;
    h.sends[0].resolve(h.ok("A", "r1"));
    await flush();
    expect(h.committed).toHaveLength(1);
    expect(h.settled).toHaveLength(before);
    expect(h.sends).toHaveLength(2);
    h.write("late", "A", { v: 3 });
    expect(h.outcome("late")).toEqual([{ requestId: "late", ok: false, error: "closed" }]);
  });

  it("每個 requestId 都恰好結算一次", async () => {
    const h = harness();
    const ids = ["w0", "w1", "w2", "w3", "w4", "w5"];
    let answered = 0;
    const answerAll = async () => {
      while (answered < h.sends.length) {
        const send = h.sends[answered];
        answered += 1;
        send.resolve(h.ok(send.key, `${send.key}-${answered}`));
        await flush();
      }
    };
    for (const [i, id] of ids.entries()) {
      h.write(id, i % 2 === 0 ? "A" : "B", { i });
      if (i === 2) await answerAll();
    }
    await answerAll();
    for (const id of ids) expect(h.outcome(id)).toHaveLength(1);
  });
});
