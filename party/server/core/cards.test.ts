// 玩家卡（room-player-cards 包 1）：上傳兩段式與名額回收、取檔、讀卡／存卡／生圖轉送與座位重驗、不落地不變式。
import { describe, expect, it } from "vitest";
import { cardsAlarm } from "./cards";
import { buildSnapshot } from "./snapshot";
import { toWebSave } from "./export";
import { CARDS, playing, STAGE, type Room } from "./room-test-kit";
import { receiveUpload } from "./upload";
import { cleanCrop, validateFrame } from "./validate";

const MiB = 1 << 20;
const SECRET = "祕密標記ZQX";
const png = (extra = "") => new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, ...new TextEncoder().encode(extra)]);
const json = (text: string) => new TextEncoder().encode(`﻿ \n${text}`);

function upload(room: Room, id: string, kind: string, bytes: Uint8Array, o: { length?: string | null; origin?: string; chunks?: Uint8Array[] } = {}) {
  const chunks = o.chunks ?? [bytes];
  return receiveUpload(room.runner, {
    kind, contentLength: o.length === undefined ? String(bytes.length) : o.length, origin: o.origin ?? null, selfOrigin: "https://t.example",
    deviceId: room.deviceId(id), body: (async function* () { yield* chunks; })(),
  });
}
const reserve = (room: Room, id: string, uploadKind: "card" | "image", length: number) =>
  room.run({ kind: "http", route: "upload_reserve", deviceId: room.deviceId(id), uploadKind, length });
const uploads = (room: Room, id: string) => room.frames(id, "vol", "card_upload").map((f) => f.p);
const job = (room: Room, kind: string) => room.frames("x1", `job.${kind}`).at(-1)!.p;
const fetchUpload = (room: Room, uploadId: string, executorToken = room.executorToken) => room.run({ kind: "http", route: "card_fetch", executorToken, uploadId });
const req = (room: Room, t: string) => room.frames("x1", t).at(-1)!.p;
/** 時間往前走、執行端照常心跳（不用 tick：鬧鐘會先判執行端心跳逾時） */
async function elapse(room: Room, ms: number) {
  room.clock += ms;
  await room.send("x1", "hb");
}

/** a、b 入座（c1、c2），c 沒入座；執行端報可以生圖 */
async function table() {
  const room = await playing();
  await room.join("c");
  await room.send("x1", "status", { keepAwake: null, busy: false, imageGen: true });
  return room;
}

describe("上傳（§3.1）", () => {
  it("標頭與座位：沒長度 411、超過上限 413、他站來源 403、封鎖 403、執行端離線 503、已入座匯入 409、沒入座換圖 403、關掉自帶角色 403", async () => {
    const room = await table();
    expect((await upload(room, "c", "card", png(), { length: null })).status).toBe(411);
    expect((await upload(room, "c", "card", png(), { length: String(8 * MiB + 1) })).status).toBe(413);
    expect((await upload(room, "a", "image", png(), { length: String(2 * MiB + 1) })).status).toBe(413);
    expect((await upload(room, "c", "card", png(), { origin: "https://evil.test" })).status).toBe(403);
    expect((await upload(room, "c", "other", png())).status).toBe(400);
    expect((await upload(room, "zz", "card", png())).status).toBe(401);
    expect((await upload(room, "a", "card", png())).status).toBe(409);
    expect((await upload(room, "c", "image", png())).body).toEqual({ code: "E_SEAT_REQUIRED" });
    await room.send("a", "room.settings", { patch: { allowCustomCards: false } });
    expect((await upload(room, "c", "card", png())).status).toBe(403);
    await room.send("a", "member.kick", { memberId: "m3", ban: true });
    expect((await upload(room, "c", "card", png())).body).toEqual({ code: "E_BANNED" });
    await room.disconnect("x1");
    expect((await upload(room, "a", "image", png())).status).toBe(503);
  });

  it("本體：簽章不對 415、串流超過宣告長度 413、長度不符 400，名額都收回", async () => {
    const room = await table();
    expect((await upload(room, "c", "card", new TextEncoder().encode("hello"))).status).toBe(415);
    expect((await upload(room, "a", "image", json("{}"))).status).toBe(415); // 圖只收 PNG
    expect((await upload(room, "c", "card", png("x"), { length: "9", chunks: [png("xx"), png("yy")] })).status).toBe(413);
    expect((await upload(room, "c", "card", png(), { length: "20" })).status).toBe(400);
    expect(Object.keys(room.runner["live"].uploads)).toEqual([]);
    expect((await upload(room, "c", "card", json('{"name":"x"}'))).status).toBe(202);
  });

  it("限流：每人同時一件、全房同時 3 件與總長 16 MiB 都在讀本體前擋；每人每小時 10 件", async () => {
    const room = await table();
    for (const id of ["d", "e"]) await room.join(id);
    expect((await reserve(room, "c", "card", 8 * MiB))!.http!.status).toBe(200);
    expect((await reserve(room, "c", "card", 10))!.http!.status).toBe(429); // 同一人
    expect((await reserve(room, "d", "card", 7 * MiB))!.http!.status).toBe(200);
    expect((await reserve(room, "e", "card", MiB + 1))!.http).toMatchObject({ status: 429, body: { code: "E_RATE_LIMITED" } }); // 總長
    expect((await reserve(room, "a", "image", 10))!.http!.status).toBe(200);
    expect((await reserve(room, "b", "image", 10))!.http!.status).toBe(429); // 第 4 件
    const fresh = await table();
    for (let i = 0; i < 10; i++) {
      const r = await reserve(fresh, "c", "card", 4);
      await fresh.run({ kind: "http", route: "upload_body", uploadId: (r!.http!.body as { uploadId: string }).uploadId, bytes: null });
    }
    expect((await reserve(fresh, "c", "card", 4))!.http!.status).toBe(429);
  });

  it("匯入成功：派工只帶 id 與長度、取檔一次就 404、結果回來替他入座並只通知上傳者", async () => {
    const room = await table();
    const res = await upload(room, "c", "card", png(SECRET));
    expect(res.status).toBe(202);
    const { uploadId } = res.body as { uploadId: string };
    const j = job(room, "card_import");
    expect(j).toEqual({ jobId: j.jobId, kind: "card_import", memberId: "m3", uploadId, size: png(SECRET).length });
    expect(room.s.jobs.map((x) => x.job)).toContainEqual(j);
    expect((await fetchUpload(room, uploadId, "y".repeat(43)))!.http!.status).toBe(401);
    expect((await fetchUpload(room, uploadId))!.http!.bytes).toEqual(png(SECRET));
    expect((await fetchUpload(room, uploadId))!.http!.status).toBe(404);
    expect((await reserve(room, "c", "card", 4))!.http!.status).toBe(429); // 取走後名額還在，等結果
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE, { ...CARDS[0], cardId: "c9", name: "匯入的" }] });
    await room.send("x1", "job.result", { jobId: j.jobId, ok: true, result: { cardId: "c9" } });
    expect(room.s.members.find((m) => m.id === "m3")!.cardId).toBe("c9");
    expect(uploads(room, "c")).toEqual([{ type: "card_upload", uploadId, ok: true, cardId: "c9" }]);
    expect(uploads(room, "a")).toEqual([]);
    expect(room.s.jobs).toEqual([]);
    expect((await upload(room, "a", "image", png())).status).toBe(202); // 名額已收回
  });

  it("匯入派工後房主封鎖了上傳者：結果回來不替他入座、不留占位", async () => {
    const room = await table();
    await upload(room, "c", "card", png());
    const j = job(room, "card_import");
    await room.send("a", "member.kick", { memberId: "m3", ban: true });
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE, { ...CARDS[0], cardId: "c9", name: "匯入的" }] });
    await room.send("x1", "job.result", { jobId: j.jobId, ok: true, result: { cardId: "c9" } });
    expect(room.s.members.find((m) => m.id === "m3")!.cardId).toBeNull();
    expect(room.s.cards.find((c) => c.cardId === "c9")!.claimedBy).toBeNull();
    expect(room.events("seat.claimed").filter((e) => e.payload.memberId === "m3")).toEqual([]);
    expect(room.s.jobs).toEqual([]);
  });

  it("名額回收：執行端回失敗、10 分鐘到期、執行端斷線、讀本體中斷；換圖取檔前換了座位就作廢", async () => {
    const room = await table();
    const first = (await upload(room, "c", "card", png())).body as { uploadId: string };
    await room.send("x1", "job.result", { jobId: job(room, "card_import").jobId, ok: false, error: { code: "E_CARD_INVALID", message: "細節不外流" } });
    expect(uploads(room, "c").at(-1)).toEqual({ type: "card_upload", uploadId: first.uploadId, ok: false, code: "E_CARD_INVALID" });

    const late = (await upload(room, "c", "card", png())).body as { uploadId: string };
    await elapse(room, 10 * 60_000);
    expect(uploads(room, "c").at(-1)).toMatchObject({ uploadId: late.uploadId, ok: false, code: "E_EXECUTOR_TIMEOUT" });
    expect((await fetchUpload(room, late.uploadId))!.http!.status).toBe(404);
    expect(room.s.jobs).toEqual([]);

    const img = (await upload(room, "a", "image", png())).body as { uploadId: string };
    await room.send("a", "seat.release");
    expect((await fetchUpload(room, img.uploadId))!.http!.status).toBe(404);
    expect(uploads(room, "a").at(-1)).toMatchObject({ uploadId: img.uploadId, ok: false, code: "E_STATE_CONFLICT" });

    const off = (await upload(room, "c", "card", png())).body as { uploadId: string };
    const held = await reserve(room, "b", "image", 8);
    await room.disconnect("x1");
    expect(uploads(room, "c").at(-1)).toMatchObject({ uploadId: off.uploadId, ok: false, code: "E_EXECUTOR_OFFLINE" });
    expect(room.s.jobs).toEqual([]);
    const body = await room.run({ kind: "http", route: "upload_body", uploadId: (held!.http!.body as { uploadId: string }).uploadId, bytes: png() });
    expect(body!.http!.status).toBe(503);
    expect(room.runner["live"].uploads).toEqual({});
  });

  it("DO 被逐出（記憶體清空、執行端連線還在）後執行端斷線：上傳工作從 w.jobs 撤掉、只通知一次", async () => {
    const room = await table();
    await upload(room, "c", "card", png());
    Object.assign(room.runner["live"], { uploads: {}, cardReqs: {} });
    await room.disconnect("x1");
    expect(room.s.jobs).toEqual([]);
    await room.send("c", "profile.rename", { nickname: "丙" });
    expect(uploads(room, "c")).toHaveLength(1);
  });
});

describe("讀卡、存卡、生圖（§3.2、§3.4）", () => {
  it("card.mine 轉執行端、只回本人；回覆前換座位就不帶內容；30 秒沒回逾時", async () => {
    const room = await table();
    expect((await room.ask("c", "card.mine"))!.p.code).toBe("E_SEAT_REQUIRED");
    await room.send("a", "card.mine", {}, "r1");
    const r = req(room, "card.read");
    expect(r).toEqual({ reqId: r.reqId, memberId: "m1", cardId: "c1" });
    const card = { name: "甲", avatar: "🗡", publicMd: "公開", privateMd: SECRET, image: null };
    await room.send("x1", "card.read.result", { reqId: r.reqId, ok: true, card });
    expect(room.frames("a", "ok").find((f) => f.p.id === "r1")!.p.result).toEqual({ card: { cardId: "c1", ...card, gallery: 0, galleryFile: null } });
    expect(JSON.stringify(room.out.b ?? [])).not.toContain(SECRET);

    room.clock += 1000;
    await room.send("a", "card.mine", {}, "r2");
    const r2 = req(room, "card.read");
    await room.send("a", "seat.release");
    expect(room.frames("a", "err").find((f) => f.p.id === "r2")!.p.code).toBe("E_STATE_CONFLICT");
    await room.send("x1", "card.read.result", { reqId: r2.reqId, ok: true, card });
    expect(room.frames("a").filter((f) => f.p?.id === "r2")).toHaveLength(1);

    await room.send("b", "card.mine", {}, "r3");
    await elapse(room, 30_000);
    expect(room.frames("b", "err").find((f) => f.p.id === "r3")!.p.code).toBe("E_EXECUTOR_TIMEOUT");
  });

  it("card.save：只能存自己座位的卡、只轉送改過的欄位、欄位照規則驗、每人 2 秒一次", async () => {
    const room = await table();
    expect((await room.ask("a", "card.save", { cardId: "c2", name: "偷改" }))!.p.code).toBe("E_FORBIDDEN");
    expect((await room.ask("a", "card.save", { cardId: "c1", privateMd: "a\u0007b" }))!.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "card.save", { cardId: "c1", publicMd: "字".repeat(64001) }))!.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "card.save", { cardId: "c1", avatar: "a b" }))!.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "card.save", { cardId: "c1", imageFromGallery: "../x.png" }))!.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "card.save", { cardId: "c1", name: "" }))!.p.code).toBe("E_NAME_INVALID");
    await room.send("a", "card.save", { cardId: "c1", privateMd: `${SECRET}\r\n\t第二行`, imageFromGallery: "1700000000000.png" }, "s1");
    const r = req(room, "card.save");
    expect(r).toEqual({ reqId: r.reqId, memberId: "m1", cardId: "c1", privateMd: `${SECRET}\n\t第二行`, imageFromGallery: "1700000000000.png" });
    expect((await room.ask("a", "card.save", { cardId: "c1", name: "乙" }))!.p.code).toBe("E_RATE_LIMITED");
    await room.send("a", "seat.release"); // 已轉出的存檔不作廢，照執行端結果回
    await room.send("x1", "card.save.result", { reqId: r.reqId, ok: true });
    expect(room.frames("a", "ok").find((f) => f.p.id === "s1")!.p).toEqual({ id: "s1" });
  });

  it("image.gen：GM 主機關掉就拒、只能自己的卡、限流每人一件全房兩件、名額在成功失敗離線逾時換座都收回", async () => {
    const room = await table();
    const gen = (id: string, cardId: string, reqId?: string) => room.send(id, "image.gen", { cardId, name: "甲", description: "描述", extraPrompt: "", framing: "half" }, reqId);
    await room.send("x1", "status", { keepAwake: null, busy: false });
    expect(room.frames("a", "vol", "executor").at(-1)!.p.imageGen).toBe(false);
    await gen("a", "c1", "g0");
    expect(room.frames("a", "err").find((f) => f.p.id === "g0")!.p.code).toBe("E_FORBIDDEN");
    await room.send("x1", "status", { keepAwake: null, busy: false, imageGen: true });
    await gen("a", "c2", "g1");
    expect(room.frames("a", "err").find((f) => f.p.id === "g1")!.p.code).toBe("E_FORBIDDEN");
    await gen("a", "c1", "g2");
    const r = req(room, "image.gen");
    expect(r).toEqual({ reqId: r.reqId, memberId: "m1", cardId: "c1", name: "甲", description: "描述", extraPrompt: "", framing: "half" });
    await gen("a", "c1", "g3");
    expect(room.frames("a", "err").find((f) => f.p.id === "g3")!.p.code).toBe("E_RATE_LIMITED");
    await room.send("c", "seat.claim", { cardId: "c3" });
    await gen("b", "c2", "g4");
    await gen("c", "c3", "g5");
    expect(room.frames("c", "err").find((f) => f.p.id === "g5")!.p.code).toBe("E_RATE_LIMITED"); // 全房兩件
    const preview = "data:image/png;base64,iVBORw0KGgo=";
    await room.send("x1", "image.gen.result", { reqId: r.reqId, ok: true, file: "1700000000000.png", preview });
    expect(room.frames("a", "ok").find((f) => f.p.id === "g2")!.p.result).toEqual({ file: "1700000000000.png", preview });
    await gen("c", "c3", "g6");
    await room.send("x1", "image.gen.result", { reqId: req(room, "image.gen").reqId, ok: false, code: "E_GEN_QUOTA" });
    expect(room.frames("c", "err").find((f) => f.p.id === "g6")!.p.code).toBe("E_GEN_QUOTA");
    await gen("c", "c3", "g7");
    await room.send("c", "seat.release");
    expect(room.frames("c", "err").find((f) => f.p.id === "g7")!.p.code).toBe("E_STATE_CONFLICT");
    room.clock += 2000;
    await gen("a", "c1", "g8");
    await elapse(room, 10 * 60_000);
    expect(room.frames("b", "err").find((f) => f.p.id === "g4")!.p.code).toBe("E_EXECUTOR_TIMEOUT");
    expect(room.frames("a", "err").find((f) => f.p.id === "g8")!.p.code).toBe("E_EXECUTOR_TIMEOUT");
    await gen("a", "c1", "g9");
    await room.disconnect("x1");
    expect(room.frames("a", "err").find((f) => f.p.id === "g9")!.p.code).toBe("E_EXECUTOR_OFFLINE");
    expect(room.runner["live"].cardReqs).toEqual({});
  });

  it("鬧鐘排在最早的上傳到期與請求逾時", () => {
    const u = { memberId: "m1", kind: "card" as const, cardId: null, length: 1, state: "reserved" as const, at: 5, bytes: null };
    expect(cardsAlarm({ uploads: { u }, cardReqs: { r: { kind: "mine", conn: "a", memberId: "m1", cardId: "c1", at: 7 } } })).toBe(30_007);
    expect(cardsAlarm({ uploads: {}, cardReqs: {} })).toBe(Infinity);
  });

  it("訊框上限依種類：只有 card.save 能到 512 KiB", () => {
    const frame = (t: string, p: object) => JSON.stringify({ t, id: "x", p });
    expect(validateFrame(frame("card.save", { cardId: "c1", publicMd: "字".repeat(60000), privateMd: "字".repeat(60000) }), "member").ok).toBe(true);
    expect(validateFrame(frame("card.save", { cardId: "c1", publicMd: "x".repeat(530 * 1024) }), "member").ok).toBe(false);
    expect(validateFrame(frame("action.draft", { text: "x".repeat(17 * 1024) }), "member").ok).toBe(false);
    expect(validateFrame(frame("image.gen", { cardId: "c1", name: "a", description: "x".repeat(17 * 1024), extraPrompt: "", framing: "full" }), "member").ok).toBe(false);
  });
});

it("不落地：上傳本體、秘密段、生成圖不進狀態、事件、快照與匯出", async () => {
  const room = await table();
  await upload(room, "c", "card", png(SECRET));
  await fetchUpload(room, job(room, "card_import").uploadId);
  await upload(room, "a", "image", png(SECRET));
  await room.send("b", "card.save", { cardId: "c2", privateMd: SECRET });
  await room.send("x1", "card.save.result", { reqId: req(room, "card.save").reqId, ok: true });
  await room.send("b", "card.mine");
  await room.send("x1", "card.read.result", { reqId: req(room, "card.read").reqId, ok: true, card: { name: "乙", avatar: "", publicMd: "", privateMd: SECRET, image: null } });
  await room.send("a", "image.gen", { cardId: "c1", name: "甲", description: SECRET, extraPrompt: SECRET, framing: "full" });
  const preview = `data:image/png;base64,${btoa(SECRET.replace(/[^A-Z]/g, ""))}`;
  await room.send("x1", "image.gen.result", { reqId: req(room, "image.gen").reqId, ok: true, file: "1.png", preview });
  const haystack = JSON.stringify([room.store.state, room.store.events, buildSnapshot(room.s, room.store.events, "m1", room.runner["live"]), toWebSave(room.s, room.store.events, 0)]);
  expect(haystack).not.toContain(SECRET);
  expect(haystack).not.toContain(preview.slice(22));
});

describe("手機圖庫（room-phone-gallery §3.1–§3.5）", () => {
  const thumb = "data:image/png;base64,AAAA";
  it("card.gallery 轉執行端、只給自己座位；檔名照規則；列頁與取單張分開限流；回覆前換座不帶內容；結果形狀要對", async () => {
    const room = await table();
    expect((await room.ask("c", "card.gallery"))!.p.code).toBe("E_SEAT_REQUIRED");
    expect((await room.ask("a", "card.gallery", { before: "../x.png" }))!.p.code).toBe("E_TEXT_INVALID");
    expect((await room.ask("a", "card.gallery", { file: "a/b.png" }))!.p.code).toBe("E_TEXT_INVALID");
    await room.send("a", "card.gallery", {}, "g1");
    const r = req(room, "card.gallery");
    expect(r).toEqual({ reqId: r.reqId, memberId: "m1", cardId: "c1" });
    const items = [{ file: "1700000000001.png", thumb }];
    await room.send("x1", "card.gallery.result", { reqId: r.reqId, ok: true, items, more: 3 });
    expect(room.frames("a", "ok").find((f) => f.p.id === "g1")!.p.result).toEqual({ items, more: 3 });
    // 剛展開就點縮圖、連續翻頁：兩種各自的額度
    await room.send("a", "card.gallery", { file: "1700000000001.png" }, "f1");
    expect(req(room, "card.gallery")).toMatchObject({ file: "1700000000001.png" });
    await room.send("a", "card.gallery", { before: "1700000000001.png" }, "g2");
    await room.send("a", "card.gallery", { before: "1700000000001.png" }, "g3");
    expect(room.frames("a", "err").filter((f) => ["f1", "g2", "g3"].includes(f.p.id))).toHaveLength(0);
    expect((await room.ask("a", "card.gallery"))!.p.code).toBe("E_RATE_LIMITED");
    // 結果形狀錯：檔名、縮圖、筆數、空頁卻說還有
    const bad = [
      { items: [{ file: "../x.png", thumb }], more: 0 },
      { items: [{ file: "1.png", thumb: "data:image/jpeg;base64,AAAA" }], more: 0 },
      { items: Array.from({ length: 25 }, (_, i) => ({ file: `${i}.png`, thumb })), more: 0 },
      { items: [], more: 2 },
    ];
    const pending = room.frames("x1", "card.gallery").slice(-3).map((f) => f.p.reqId);
    for (const [i, reqId] of pending.entries()) await room.send("x1", "card.gallery.result", { reqId, ok: true, ...bad[i] });
    for (const id of ["f1", "g2", "g3"]) expect(room.frames("a", "err").find((f) => f.p.id === id)!.p.code).toBe("E_INTERNAL");
    room.clock += 2000;
    await room.send("a", "card.gallery", {}, "g4");
    await room.send("x1", "card.gallery.result", { reqId: req(room, "card.gallery").reqId, ok: true, ...bad[3] });
    expect(room.frames("a", "err").find((f) => f.p.id === "g4")!.p.code).toBe("E_INTERNAL");
    // 回覆前換座：不帶內容
    await room.send("a", "card.gallery", {}, "g5");
    const r5 = req(room, "card.gallery");
    await room.send("a", "seat.release");
    expect(room.frames("a", "err").find((f) => f.p.id === "g5")!.p.code).toBe("E_STATE_CONFLICT");
    await room.send("x1", "card.gallery.result", { reqId: r5.reqId, ok: true, items, more: 0 });
    expect(room.frames("a").filter((f) => f.p?.id === "g5")).toHaveLength(1);
    // 30 秒沒回逾時
    await room.send("b", "card.gallery", {}, "g6");
    await elapse(room, 30_000);
    expect(room.frames("b", "err").find((f) => f.p.id === "g6")!.p.code).toBe("E_EXECUTOR_TIMEOUT");
    // 兩個同時送是壞訊框（會斷線，所以放最後）
    expect((await room.ask("b", "card.gallery", { before: "1.png", file: "2.png" }))!.p.code).toBe("E_BAD_FRAME");
  });

  it("card.read.result 的使用中檔名照圖庫檔名規則", async () => {
    const room = await table();
    await room.send("a", "card.mine", {}, "r1");
    const card = { name: "甲", avatar: "🗡", publicMd: "", privateMd: "", image: null, gallery: 2, galleryFile: "../x.png" };
    await room.send("x1", "card.read.result", { reqId: req(room, "card.read").reqId, ok: true, card });
    expect(room.frames("a", "err").find((f) => f.p.id === "r1")!.p.code).toBe("E_INTERNAL");
    room.clock += 1000;
    await room.send("a", "card.mine", {}, "r2");
    await room.send("x1", "card.read.result", { reqId: req(room, "card.read").reqId, ok: true, card: { ...card, galleryFile: "1.png" } });
    expect(room.frames("a", "ok").find((f) => f.p.id === "r2")!.p.result.card).toMatchObject({ gallery: 2, galleryFile: "1.png" });
  });

  it("card.save 的裁切框：要跟圖庫圖一起、四欄有限數字、在原圖範圍內", async () => {
    const room = await table();
    const save = async (crop: unknown, extra: object = { imageFromGallery: "1.png" }) => {
      room.clock += 2000;
      return (await room.ask("a", "card.save", { cardId: "c1", ...extra, crop }))!.p;
    };
    for (const crop of [{ x: 0, y: 0, w: 1 }, { x: "0", y: 0, w: 1, h: 1 }, { x: -0.1, y: 0, w: 0.5, h: 1 }, { x: 0, y: 0, w: 0, h: 1 }, { x: 0.5, y: 0, w: 0.5002, h: 1 }])
      expect((await save(crop)).code, JSON.stringify(crop)).toBe("E_TEXT_INVALID");
    room.clock += 2000;
    await room.send("a", "card.save", { cardId: "c1", imageFromGallery: "1.png", crop: { x: 0.5, y: 0, w: 0.5001, h: 1 } }, "s1");
    expect(req(room, "card.save")).toMatchObject({ imageFromGallery: "1.png", crop: { x: 0.5, y: 0, w: 0.5001, h: 1 } });
    // 壞訊框會斷線，各用一支手機
    expect((await room.ask("b", "card.save", { cardId: "c2", crop: { x: 0, y: 0, w: 1, h: 1 } }))!.p.code).toBe("E_BAD_FRAME");
    room.clock += 2000;
    await room.send("a", "card.save", { cardId: "c1", imageFromGallery: "1.png", crop: 5 }); // 外形不對：連 id 都不回
    expect(room.frames("a", "err").at(-1)!.p.code).toBe("E_BAD_FRAME");
  });

  it("cleanCrop 擋 NaN、Infinity、缺欄位；原始訊框裡的 NaN 整則拒收", () => {
    for (const v of [{ x: NaN, y: 0, w: 1, h: 1 }, { x: 0, y: 0, w: Infinity, h: 1 }, { x: 0, y: 0, w: 1 }, null, "x"]) expect(cleanCrop(v)).toBeNull();
    expect(cleanCrop({ x: 0, y: 0, w: 1.0001, h: 1 })).toEqual({ x: 0, y: 0, w: 1.0001, h: 1 });
    const raw = '{"t":"card.save","id":"s1","p":{"cardId":"c1","imageFromGallery":"1.png","crop":{"x":NaN,"y":0,"w":1,"h":1}}}';
    expect(validateFrame(raw, "member").ok).toBe(false);
  });
});

describe("手機房主拿現行邀請連結 invite.current（room-seat-delegate §3.6）", () => {
  const reply = (room: Room, id: string, reqId: string) => room.out[id]?.find((f) => (f.t === "ok" || f.t === "err") && f.p.id === reqId);

  it("候選密語對得上就直接回、不經電腦（電腦離線也行）；不是房主回 E_FORBIDDEN", async () => {
    const room = await table();
    expect((await room.ask("a", "invite.current", { candidate: room.joinSecret }))?.p.result).toEqual({ joinSecret: room.joinSecret });
    expect(room.frames("x1", "invite.get")).toEqual([]);
    expect((await room.ask("b", "invite.current", { candidate: room.joinSecret }))?.p.code).toBe("E_FORBIDDEN");
    await room.disconnect("x1");
    expect((await room.ask("a", "invite.current", { candidate: room.joinSecret }))?.p.result).toEqual({ joinSecret: room.joinSecret });
    expect((await room.ask("a", "invite.current", { candidate: "old" }))?.p.code).toBe("E_EXECUTOR_OFFLINE");
  });

  it("候選過期或沒帶：轉電腦問、只回發問的連線；電腦回的不是現行密語就不帶內容", async () => {
    const room = await table();
    await room.send("a", "invite.current", { candidate: "old" }, "r1");
    const { reqId } = req(room, "invite.get");
    expect(reply(room, "a", "r1")).toBeUndefined();
    await room.send("x1", "invite.get.result", { reqId, joinSecret: room.joinSecret });
    expect(reply(room, "a", "r1")?.p.result).toEqual({ joinSecret: room.joinSecret });
    for (const id of ["b", "c"]) expect(JSON.stringify(room.out[id])).not.toContain(room.joinSecret);

    await room.send("a", "invite.current", {}, "r2");
    await room.send("x1", "invite.get.result", { reqId: req(room, "invite.get").reqId, joinSecret: "stale" });
    expect(reply(room, "a", "r2")?.p.code).toBe("E_STATE_CONFLICT");
  });

  it("回覆前房主轉讓了就不帶內容；電腦斷線即失敗；30 秒沒回逾時", async () => {
    const room = await table();
    await room.send("a", "invite.current", {}, "r1");
    const { reqId } = req(room, "invite.get");
    await room.send("a", "host.transfer", { memberId: "m2" });
    expect(reply(room, "a", "r1")?.p.code).toBe("E_STATE_CONFLICT");
    await room.send("x1", "invite.get.result", { reqId, joinSecret: room.joinSecret });
    expect(JSON.stringify(room.out.a)).not.toContain(room.joinSecret);

    await room.send("b", "invite.current", {}, "r2");
    await elapse(room, 30_000);
    expect(reply(room, "b", "r2")?.p.code).toBe("E_EXECUTOR_TIMEOUT");

    await room.send("b", "invite.current", {}, "r3");
    await room.disconnect("x1");
    expect(reply(room, "b", "r3")?.p.code).toBe("E_EXECUTOR_OFFLINE");
  });

  it("不落地：密語不進狀態、事件、快照與匯出", async () => {
    const room = await table();
    await room.send("a", "invite.current", { candidate: room.joinSecret });
    await room.send("a", "invite.current", {}, "r1");
    await room.send("x1", "invite.get.result", { reqId: req(room, "invite.get").reqId, joinSecret: room.joinSecret });
    const snap = buildSnapshot(room.s, room.store.events, "m1", { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} });
    for (const blob of [room.s, room.store.events, snap, toWebSave(room.s, room.store.events, room.clock)]) expect(JSON.stringify(blob)).not.toContain(room.joinSecret);
  });
});
