// 重開補送（room-host-actions §3.3）：同步閘門與 story.sync。
import { describe, expect, it } from "vitest";
import { applyFrame, type RoomView } from "../../src/features/room/room-state";
import { toWebSave } from "./export";
import { CARDS, Room, STAGE } from "./room-test-kit";
import { buildSnapshot } from "./snapshot";

const LIVE = { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} };
const story = (room: Room) => buildSnapshot(room.s, room.store.events, "m2", LIVE).story;
const gm = (text: string) => ({ part: { speaker: { kind: "gm" }, text, interrupted: false, at: 0 } });
const npc = (text: string) => ({ part: { speaker: { kind: "npc", cardId: "c3", name: "角色3" }, text, interrupted: false, at: 0 } });
const said = (text: string) => ({ part: { speaker: { kind: "player", memberId: "m1", cardId: "c1", name: "角色1" }, text, interrupted: false, at: 0 } });

/** 送過種子的房間（種子佔回合 1）：a 房主、a b 入座、開場跑完、第 3 回合 a 送出 → 房主關房（status paused）斷線 */
async function paused() {
  const room = await Room.create();
  await room.exec();
  await room.send("x1", "room.seed", { part: 0, total: 1, cards: [...CARDS, STAGE], history: [gm("種子開場").part] });
  for (const [i, id] of ["a", "b"].entries()) {
    await room.join(id, id, i === 0 ? { pairCode: await room.pairCode(true) } : {});
    await room.send(id, "seat.claim", { cardId: CARDS[i].cardId });
  }
  await room.send("a", "room.settings", { patch: { actionTimeoutSec: 60 } });
  await room.send("a", "room.start");
  await room.finishGen("開場");
  await room.send("a", "action.submit", { round: 3, text: "我拔劍" });
  await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
  await room.disconnect("x1");
  return room;
}
/** b 從加入那一刻起看到的畫面（第一份快照＋之後逐筆套事件） */
const clientView = (room: Room, id = "b") =>
  (room.out[id] ?? []).reduce<RoomView | null>((v, f) => (f.t === "welcome" && v ? v : applyFrame(v, f)), null)!;
const sync = (room: Room, p: Record<string, unknown>, id = "x2") => room.ask(id, "story.sync", { part: 0, total: 1, ...p });

describe("同步閘門（§3.3）", () => {
  it("閘門期間：welcome 不帶派工、status 不解除、全員送出與倒數到期都不結算、回合指令回 E_EXECUTOR_OFFLINE；空的 story.sync 才放行", async () => {
    const room = await paused();
    await room.exec("x2");
    expect(room.frames("x2", "welcome")[0].p.dispatch).toBeNull();
    expect(room.s.exec).toMatchObject({ online: true, sync: "needed", paused: true });
    await room.send("x2", "status", { keepAwake: null, busy: false });
    expect(room.s.exec.paused).toBe(true);
    expect(room.frames("b", "vol", "executor").at(-1)?.p).toMatchObject({ online: true, syncing: true, paused: false });
    await room.send("b", "action.submit", { round: 3, text: "我推門" });
    expect(room.s.round).toMatchObject({ round: 3, phase: "acting", deadline: null }); // 倒數停著，不會到期
    expect(room.s.round!.pausedRemainingMs).toBeGreaterThan(0);
    for (const [t, p] of [["round.resolve", { round: 3 }], ["host.gm", { round: 3, mode: "advance" }], ["round.undo", { round: 2, mode: "edit" }], ["scene.advance", { round: 3 }]] as const)
      expect((await room.ask("a", t, p))?.p.code).toBe("E_EXECUTOR_OFFLINE");
    expect(room.frames("x2", "dispatch")).toEqual([]);
    const last = story(room).at(-1)!;
    expect((await sync(room, { keepThrough: { round: last.round, part: last.part } }))?.t).toBe("ok");
    expect(room.s.exec).toMatchObject({ sync: null, paused: false });
    expect(room.events("story.truncated")).toEqual([]);
    expect(room.s.round).toMatchObject({ round: 3, phase: "queued" }); // 補完才結算、派工 // 沒有補送內容：已送出的不動，照常結算
    expect(room.frames("x2", "dispatch")).toHaveLength(1);
    expect((await sync(room, {}))?.p.detail).toBe("sync_not_needed");
  });

  it("閘門期間停在 generating 的回合（執行端沒留下結果）：補完重送同一份派工", async () => {
    const room = await paused();
    await room.send("b", "action.submit", { round: 3, text: "我推門" }); // 全員送出，但閘門沒開不結算
    await room.exec("x2");
    expect(room.frames("x2", "dispatch")).toEqual([]);
    const last = story(room).at(-1)!;
    await sync(room, { keepThrough: { round: last.round, part: last.part } });
    expect(room.s.round).toMatchObject({ round: 3, phase: "queued" }); // 補完才結算、派工
    const first = room.frames("x2", "dispatch").at(-1)!.p;
    await room.send("x2", "dispatch.ack", { dispatchId: first.dispatchId });
    expect(room.s.round!.phase).toBe("generating");
    // 執行端收下後又被關房、結果沒留下：重開補完時拿到同一份
    await room.send("x2", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x2");
    await room.exec("x3");
    expect(room.frames("x3", "welcome")[0].p.dispatch).toBeNull();
    await sync(room, { keepThrough: { round: last.round, part: last.part } }, "x3");
    expect(room.frames("x3", "dispatch").at(-1)?.p.dispatchId).toBe(first.dispatchId);
  });

  it("關房那則 status 沒送到：hello 自報 paused 一樣上閘門；沒送過種子的房間不上閘門", async () => {
    const room = await paused();
    room.s.exec.paused = false; // 模擬伺服器沒收到
    await room.exec("x2", { paused: true });
    expect(room.s.exec.sync).toBe("needed");
    const fresh = await Room.create();
    await fresh.exec("x1", { paused: true });
    expect(fresh.s.exec.sync).toBeNull();
  });

  it("撤段＋補段：快照、客戶端逐筆套用、匯出三處一致；已送出的行動退回草稿；補送玩家句與 GM 分回合", async () => {
    const room = await paused();
    await room.exec("x2");
    const before = story(room).map((p) => p.text);
    expect(before).toEqual(["種子開場", "開場"]);
    const seed = story(room)[0];
    const reply = await sync(room, { keepThrough: { round: seed.round, part: seed.part }, items: [{ part: { speaker: { kind: "system", marker: "solo_paused" }, text: "", interrupted: false, at: 0 } }, gm("開場（改過）"), said("我搜身"), gm("找到鑰匙"), npc("還我"), said("不還")] });
    expect(reply?.t).toBe("ok");
    expect(room.events("story.truncated").at(-1)?.payload).toEqual({ after: { round: seed.round, part: seed.part } });
    const texts = ["種子開場", "", "開場（改過）", "我搜身", "找到鑰匙", "還我", "不還"];
    expect(story(room).map((p) => p.text)).toEqual(texts);
    expect(clientView(room).story.map((p) => p.text)).toEqual(texts);
    expect(toWebSave(room.s, room.store.events, room.clock).messages.map((m) => m.text)).toEqual(["種子開場", "開場（改過）", "角色1：我搜身", "找到鑰匙", "角色3：還我", "角色1：不還"]);
    const parts = story(room).slice(1).map((p) => [p.round - story(room)[1].round, p.part]);
    expect(parts).toEqual([[0, 0], [0, 1], [1, -1], [1, 0], [1, 1], [2, -1]]);
    expect(room.s.drafts.m1).toBe("我拔劍");
    expect(room.s.round!.roster.find((e) => e.memberId === "m1")?.status).toBe("drafting");
    expect(room.s.lastDispatch).toBeNull();
  });

  it("種子超過 32 段：段號不撞摘要／收回提示的保留號；重開補送照常比對，快照與客戶端一致", async () => {
    const room = await Room.create();
    await room.exec();
    const history = Array.from({ length: 40 }, (_, i) => gm(`種子第${i}段`).part);
    await room.send("x1", "room.seed", { part: 0, total: 1, cards: [...CARDS, STAGE], history });
    await room.join("a", "a", { pairCode: await room.pairCode(true) });
    await room.join("b");
    await room.send("a", "seat.claim", { cardId: "c1" });
    await room.send("a", "room.start");
    await room.finishGen("開場");
    const seeded = story(room).filter((p) => p.text.startsWith("種子"));
    expect(seeded).toHaveLength(40);
    expect(seeded.every((p) => p.part >= 0 && p.part <= 31)).toBe(true);
    expect(new Set(seeded.map((p) => `${p.round}:${p.part}`)).size).toBe(40);
    await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x1");
    await room.exec("x2");
    await sync(room, { keepThrough: null, items: [gm("單人補的")] }); // 全量重送：本幕從頭撤（舊版客戶端會把 part 32 當幕界、留下前 33 段）
    const texts = story(room).map((p) => p.text);
    expect(texts).toEqual(["單人補的"]);
    expect(clientView(room).story.map((p) => p.text)).toEqual(texts);
  });

  it("補送後接著玩的第一回合照常能收回：待玩回合的回合號排在補送段之後", async () => {
    const room = await paused();
    await room.exec("x2");
    const last = story(room).at(-1)!;
    await sync(room, { keepThrough: { round: last.round, part: last.part }, items: [gm("單人旁白")] });
    const r = room.s.round!.round;
    expect(r).toBeGreaterThan(Math.max(...story(room).map((p) => p.round)));
    await room.send("a", "action.submit", { round: r, text: "我拔劍" });
    await room.send("b", "action.submit", { round: r, text: "我推門" });
    await room.finishGen("門開了", "x2");
    expect(room.s.round!.round).toBe(r + 1);
    await room.send("a", "round.undo", { round: r, mode: "edit" });
    expect(room.frames("x2", "job.undo").at(-1)?.p.dispatchId).toBe(room.s.lastDispatch!.dispatchId);
  });

  it("保留到收回提示那段：三處一致（匯出也認得那一段）", async () => {
    const room = await paused();
    await room.exec("x2", { paused: true });
    const top = story(room).at(-1)!;
    await sync(room, { keepThrough: { round: top.round, part: top.part } }); // 先開閘門好收回
    await room.send("b", "action.submit", { round: 3, text: "我推門" });
    await room.finishGen("門開了", "x2");
    await room.send("a", "round.undo", { round: 3, mode: "edit" });
    await room.send("x2", "job.result", { jobId: room.frames("x2", "job.undo").at(-1)!.p.jobId, ok: true });
    const note = story(room).at(-1)!;
    expect(note.part).toBe(33);
    await room.send("a", "host.gm", { round: room.s.round!.round, mode: "narrate" });
    await room.finishGen("單人時被收掉的旁白", "x2");
    await room.send("x2", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x2");
    await room.exec("x3");
    expect((await sync(room, { keepThrough: { round: note.round, part: note.part }, items: [gm("單人補的")] }, "x3"))?.t).toBe("ok");
    expect(room.events("story.truncated")).toHaveLength(1);
    const texts = story(room).map((p) => p.text);
    expect(texts.slice(-2)).toEqual(["", "單人補的"]); // 收回提示留著、旁白撤掉
    expect(clientView(room).story.map((p) => p.text)).toEqual(texts);
    expect(toWebSave(room.s, room.store.events, room.clock).messages.map((m) => m.text)).toEqual(texts.filter((x) => x !== "")); // 系統段不輸出
  });

  it("單人時換過幕：比照換幕補摘要（不重複）、開新的 play 回合、sceneStartSeq 與快照一致", async () => {
    const room = await paused();
    await room.exec("x2");
    const last = story(room).at(-1)!;
    await sync(room, { keepThrough: { round: last.round, part: last.part }, items: [gm("第一幕尾聲"), { summary: "第一幕摘要" }, gm("第二幕")] });
    expect(room.s).toMatchObject({ scene: 2, round: { kind: "play", phase: "acting" } });
    expect(room.events("scene.started").at(-1)!.seq).toBe(room.s.sceneStartSeq);
    expect(room.s.summarySeqs).toEqual([room.events("scene.summary")[0].seq]);
    expect(story(room).map((p) => p.text)).toEqual(["種子開場", "開場", "第一幕尾聲", "第一幕摘要", "第二幕"]);
    await room.disconnect("b");
    await room.join("b"); // 重開頁面：快照只帶本幕與上幾幕摘要
    expect(room.frames("b", "welcome").at(-1)!.p.snapshot.story.map((p: { text: string }) => p.text)).toEqual(["第一幕摘要", "第二幕"]);
    expect(clientView(room).story.map((p) => p.text).filter((x) => x === "第一幕摘要")).toHaveLength(1);
    expect(room.s.drafts.m1).toBe("我拔劍"); // 換幕作廢回合：已送出的退回草稿
  });

  it("keepThrough 指不到本幕段落回 E_STATE_CONFLICT、狀態不動；中途斷片不落地，下一片號對不上要從頭重送", async () => {
    const room = await paused();
    await room.exec("x2");
    const seq = room.s.seq;
    expect((await sync(room, { keepThrough: { round: 99, part: 0 } }))?.p.detail).toBe("sync_keep_missing");
    expect(room.s.seq).toBe(seq);
    await room.ask("x2", "story.sync", { part: 0, total: 2, keepThrough: null, items: [gm("一")] });
    expect(room.s.seq).toBe(seq);
    room.runner["live"].sync = null; // 殼被逐出：記憶體緩衝遺失
    expect((await room.ask("x2", "story.sync", { part: 1, total: 2, items: [gm("二")] }))?.p.detail).toBe("sync_restart");
    expect(room.s.seq).toBe(seq);
    expect(room.s.exec.sync).toBe("needed");
    await sync(room, { keepThrough: null, items: [gm("全量")] });
    expect(story(room).map((p) => p.text)).toEqual(["全量"]);
  });
});

describe("手機房主遠端暫停後重開（room-seat-delegate §3.3）", () => {
  it("ctl pause 走同一條關房流程：重開照樣上閘門、補完放行；代管狀態保留，派工不帶代管的人", async () => {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "room.seed", { part: 0, total: 1, cards: [...CARDS, STAGE], history: [gm("種子開場").part] });
    for (const [i, id] of ["a", "b"].entries()) {
      await room.join(id, id, i === 0 ? { pairCode: await room.pairCode(true) } : {});
      await room.send(id, "seat.claim", { cardId: CARDS[i].cardId });
    }
    await room.send("a", "room.start");
    await room.finishGen("開場");
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("a", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }, "r1");
    await room.send("x1", "ctl.result", { ctlId: "p1", ok: true });
    await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x1");
    expect(room.frames("a", "ok").find((f) => f.p.id === "r1")?.p.result).toEqual({ ok: true });
    expect(room.s.exec).toMatchObject({ online: false, paused: true });

    await room.exec("x2", { paused: true });
    expect(room.s.exec).toMatchObject({ online: true, sync: "needed" });
    await room.send("x2", "roster.update", { cards: [...CARDS, STAGE] }); // 重開先重送名冊：代管的卡照樣在
    const last = story(room).at(-1)!;
    expect((await sync(room, { keepThrough: { round: last.round, part: last.part } }))?.t).toBe("ok");
    expect(room.s.exec).toMatchObject({ sync: null, paused: false });
    expect(room.s.members.find((m) => m.id === "m2")).toMatchObject({ cardId: CARDS[1].cardId, delegated: true });
    expect(room.s.round!.roster.map((e) => e.memberId)).toEqual(["m1"]);
    await room.send("a", "action.submit", { round: room.s.round!.round, text: "繼續" });
    const d = room.frames("x2", "dispatch").at(-1)!.p;
    expect(d.players.map((p: { cardId: string }) => p.cardId)).toEqual([CARDS[0].cardId]);
    expect(d.departures).toEqual([{ cardId: CARDS[1].cardId, mode: "delegate" }]);
  });
});
