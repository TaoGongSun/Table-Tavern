// 座位交給 AI 代管、移除時一起決定處置、房主遠端暫停（room-seat-delegate 包 1）
import { describe, expect, it } from "vitest";
import { CARDS, playing, Room, STAGE } from "./room-test-kit";
import { applyFrame, type RoomView } from "../../src/features/room/room-state";

const lastDispatch = (room: Room) => room.frames("x1", "dispatch").at(-1)?.p;
const ids = (room: Room) => room.s.round!.roster.map((e) => e.memberId);

describe("AI 代管（seat.delegate）", () => {
  it("本人或房主才能開關；狀態沒變回 E_STATE_CONFLICT；沒座位回 E_SEAT_REQUIRED", async () => {
    const room = await playing(["a", "b", "c"]);
    expect((await room.ask("c", "seat.delegate", { memberId: "m2", on: true }))?.p.code).toBe(
      "E_FORBIDDEN",
    );
    expect((await room.ask("b", "seat.delegate", { memberId: "m2", on: true }))?.t).toBe("ok");
    expect((await room.ask("b", "seat.delegate", { memberId: "m2", on: true }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    expect((await room.ask("a", "seat.delegate", { memberId: "m3", on: true }))?.t).toBe("ok");
    await room.join("d");
    expect((await room.ask("a", "seat.delegate", { memberId: "m4", on: true }))?.p.code).toBe(
      "E_SEAT_REQUIRED",
    );
    const snap = room.frames("d", "welcome").at(-1)?.p.snapshot;
    expect(snap.members.find((m: { memberId: string }) => m.memberId === "m2").delegated).toBe(
      true,
    );
  });

  it("已送出後代管：先發 action.retracted 再發 seat.delegated，原文回草稿、批次與在場玩家都沒有他", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "我先走" });
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    const types = room.frames("b", "ev").map((f) => f.p.e.type);
    expect(types.slice(-2)).toEqual(["action.retracted", "seat.delegated"]);
    expect(room.s.drafts.m2).toBe("我先走");
    expect(ids(room)).toEqual(["m1"]);
    await room.send("a", "action.submit", { round: 2, text: "a 行動" });
    expect(room.events("round.resolved")[0].payload.batch).toMatchObject([
      { memberId: "m1", kind: "act" },
    ]);
    expect(lastDispatch(room)).toMatchObject({
      players: [{ cardId: "c1" }],
      departures: [{ cardId: "c2", mode: "delegate" }],
      returns: [],
    });
  });

  it("代管後名單只剩已送出的人：交易尾端才結算，這次派工帶得到暫離項", async () => {
    const room = await playing();
    await room.send("a", "action.submit", { round: 2, text: "a 行動" });
    await room.send("a", "seat.delegate", { memberId: "m2", on: true });
    expect(room.events("round.resolved")).toHaveLength(1);
    expect(lastDispatch(room)).toMatchObject({ departures: [{ cardId: "c2", mode: "delegate" }] });
  });

  it("全員代管：名單空、不結算、倒數暫停；接回時名單空著就直接加入當前回合並算登場與接回", async () => {
    const room = await playing();
    await room.send("a", "room.settings", { patch: { actionTimeoutSec: 60 } });
    await room.send("a", "seat.delegate", { memberId: "m2", on: true });
    await room.send("a", "action.skip", { round: 2 });
    await room.finishGen();
    await room.send("a", "seat.delegate", { memberId: "m1", on: true });
    expect(room.s.round).toMatchObject({ round: 3, roster: [], deadline: null });
    expect(room.frames("x1", "dispatch")).toHaveLength(2);
    await room.send("b", "seat.delegate", { memberId: "m2", on: false });
    expect(room.s.round).toMatchObject({ roster: [{ memberId: "m2" }], arrivals: ["c2"] });
    await room.send("b", "action.skip", { round: 3 });
    expect(lastDispatch(room)).toMatchObject({
      arrivals: ["c2"],
      returns: ["c2"],
      players: [{ cardId: "c2" }],
    });
    expect(room.s.returning).toEqual([]);
  });

  it("接回時名單還有人：下一回合才登場；換幕回合（arrivals 空）不消耗 returning", async () => {
    const room = await playing();
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("a", "action.skip", { round: 2 });
    await room.finishGen();
    await room.send("b", "seat.delegate", { memberId: "m2", on: false });
    expect(ids(room)).toEqual(["m1"]);
    expect(room.s.members.find((m) => m.id === "m2")?.arriving).toBe(true);
    await room.send("a", "scene.advance", { round: 3 });
    const job = room.s.jobs[0].job;
    await room.send("x1", "job.result", {
      jobId: job.jobId,
      ok: true,
      result: { summary: "第一幕摘要" },
    });
    expect(lastDispatch(room)).toMatchObject({ kind: "scene_open", arrivals: [], returns: [] });
    expect(room.s.returning).toEqual(["c2"]);
    await room.finishGen();
    await room.send("a", "action.skip", { round: room.s.round!.round });
    await room.send("b", "action.skip", { round: room.s.round!.round });
    expect(lastDispatch(room)).toMatchObject({ kind: "play", arrivals: ["c2"], returns: ["c2"] });
    expect(room.s.returning).toEqual([]);
  });

  it("開了又關、中間沒派工：抵消，直接回當前名單、不算登場，派工沒有暫離與接回", async () => {
    const room = await playing();
    await room.send("b", "action.draft", { text: "草稿" });
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("b", "seat.delegate", { memberId: "m2", on: false });
    expect(room.s.round).toMatchObject({
      roster: [{ memberId: "m1" }, { memberId: "m2", status: "drafting" }],
      arrivals: [],
    });
    await room.send("a", "action.skip", { round: 2 });
    await room.send("b", "action.skip", { round: 2 });
    expect(lastDispatch(room)).toMatchObject({ departures: [], returns: [], arrivals: [] });
  });

  it("還沒登場就代管再取消：保留待登場，期間派工不帶他的暫離項，下一回合照一般登場", async () => {
    const room = await playing();
    await room.join("c");
    await room.send("c", "seat.claim", { cardId: "c3" });
    await room.send("c", "seat.delegate", { memberId: "m3", on: true });
    await room.send("a", "action.skip", { round: 2 });
    await room.send("b", "action.skip", { round: 2 });
    expect(lastDispatch(room)).toMatchObject({
      departures: [],
      players: [{ cardId: "c1" }, { cardId: "c2" }],
    });
    await room.finishGen();
    expect(ids(room)).toEqual(["m1", "m2"]);
    await room.send("c", "seat.delegate", { memberId: "m3", on: false });
    expect(room.s.returning ?? []).toEqual([]);
    await room.send("a", "action.skip", { round: 3 });
    await room.send("b", "action.skip", { round: 3 });
    await room.finishGen();
    expect(ids(room)).toEqual(["m1", "m2", "m3"]);
    expect(room.s.round!.arrivals).toEqual(["c3"]);
  });

  it("代管項還沒派出就被踢或離座：改寫成離席處置；之後再入座不是代管中", async () => {
    const room = await playing(["a", "b", "c"]);
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("c", "seat.delegate", { memberId: "m3", on: true });
    await room.send("a", "member.kick", { memberId: "m2", ban: false, mode: "npc" });
    await room.send("c", "seat.release");
    expect(room.s.departures).toEqual([
      { cardId: "c2", mode: "npc" },
      { cardId: "c3", mode: "absent" },
    ]);
    expect(room.events("seat.departure").at(-1)?.payload).toEqual({ cardId: "c2", mode: "npc" });
    await room.send("c", "seat.claim", { cardId: "c2" });
    expect(room.s.members.find((m) => m.id === "m3")?.delegated).toBe(false);
  });

  it("換裝置移座位：來源與接手者都不是代管中，暫離沒派出就抵消", async () => {
    const room = await playing();
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.join("b-phone");
    await room.send("a", "member.reassign", { from: "m2", to: "m3" });
    expect(room.s.members.filter((m) => m.delegated)).toEqual([]);
    expect(room.s.departures).toEqual([]);
    expect(ids(room)).toEqual(["m1", "m3"]);
    expect(room.events("seat.delegated").at(-1)?.payload).toMatchObject({
      memberId: "m3",
      on: false,
    });
  });

  it("大廳代管：至少要一位沒代管的玩家才能開始；開演時不算在場，接回後照一般登場（不算接回）", async () => {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
    await room.join("a", "a", { pairCode: await room.pairCode(true) });
    await room.join("b");
    await room.send("a", "seat.claim", { cardId: "c1" });
    await room.send("b", "seat.claim", { cardId: "c2" });
    await room.send("a", "seat.delegate", { memberId: "m1", on: true });
    await room.send("a", "seat.delegate", { memberId: "m2", on: true });
    expect((await room.ask("a", "room.start"))?.p.code).toBe("E_STATE_CONFLICT");
    await room.send("b", "seat.delegate", { memberId: "m2", on: false });
    await room.send("a", "room.start");
    expect(lastDispatch(room)).toMatchObject({
      kind: "open",
      players: [{ cardId: "c2" }],
      departures: [],
    });
    await room.finishGen();
    expect(ids(room)).toEqual(["m2"]);
    await room.send("a", "seat.delegate", { memberId: "m1", on: false });
    await room.send("b", "action.skip", { round: 2 });
    await room.finishGen();
    expect(room.s.round).toMatchObject({ arrivals: ["c1"] });
    await room.send("a", "action.skip", { round: 3 });
    await room.send("b", "action.skip", { round: 3 });
    expect(lastDispatch(room)).toMatchObject({ arrivals: ["c1"], returns: [] });
  });
});

describe("房主遠端暫停（ctl pause）", () => {
  it("房主才能下（出額度者不行）；同時只能一個；執行端回 ok 就回房主", async () => {
    const room = await playing();
    await room.send("a", "host.transfer", { memberId: "m2" });
    expect((await room.ask("a", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }))?.p.code).toBe(
      "E_FORBIDDEN",
    );
    await room.send("b", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }, "r1");
    expect(room.frames("x1", "ctl").at(-1)?.p).toMatchObject({
      ctlId: "p1",
      cmd: { kind: "pause" },
    });
    expect((await room.ask("b", "ctl", { ctlId: "p2", cmd: { kind: "pause" } }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    await room.send("x1", "ctl.result", { ctlId: "p1", ok: true });
    expect(room.frames("b", "ok").find((f) => f.p.id === "r1")?.p.result).toEqual({ ok: true });
    expect(room.s.ctls).toEqual({});
  });

  it("等回覆時執行端下線：已報暫停就算成功，否則回 E_EXECUTOR_OFFLINE；30 秒沒回 E_EXECUTOR_TIMEOUT", async () => {
    const room = await playing();
    await room.send("a", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }, "r1");
    await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x1");
    expect(room.frames("a", "ok").find((f) => f.p.id === "r1")?.p.result).toEqual({ ok: true });

    const r2 = await playing();
    await r2.send("a", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }, "r1");
    await r2.disconnect("x1");
    expect(r2.frames("a", "err").find((f) => f.p.id === "r1")?.p.code).toBe("E_EXECUTOR_OFFLINE");

    const r3 = await playing();
    await r3.send("a", "ctl", { ctlId: "p1", cmd: { kind: "pause" } }, "r1");
    r3.clock += 15_000;
    await r3.send("x1", "hb");
    await r3.tick(15_000);
    expect(r3.frames("a", "err").find((f) => f.p.id === "r1")?.p.code).toBe("E_EXECUTOR_TIMEOUT");
  });
});

describe("代管接回碰上收回／重生", () => {
  /** b 代管後接回，第 4 回合登場那次派工帶 returns: [c2]，GM 跑完停在第 5 回合 */
  async function returned() {
    const room = await playing();
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("a", "action.skip", { round: 2 });
    await room.finishGen();
    await room.send("b", "seat.delegate", { memberId: "m2", on: false });
    await room.send("a", "action.skip", { round: 3 });
    await room.finishGen();
    await room.send("a", "action.skip", { round: 4 });
    await room.send("b", "action.skip", { round: 4 });
    expect(lastDispatch(room)).toMatchObject({ round: 4, arrivals: ["c2"], returns: ["c2"] });
    await room.finishGen();
    return room;
  }
  const undoJob = (room: Room) => room.frames("x1", "job.undo").at(-1)!.p;

  it("收回改字：接回的卡放回 returning，下次派工照樣寫接回", async () => {
    const room = await returned();
    await room.send("a", "round.undo", { round: 4, mode: "edit" });
    await room.send("x1", "job.result", { jobId: undoJob(room).jobId, ok: true });
    expect(room.s.returning).toEqual(["c2"]);
    for (const id of ["a", "b"]) await room.send(id, "action.skip", { round: room.s.round!.round });
    expect(lastDispatch(room)).toMatchObject({ arrivals: ["c2"], returns: ["c2"] });
    expect(room.s.returning).toEqual([]);
  });

  it("收回改字後又被代管：不放回 returning", async () => {
    const room = await returned();
    await room.send("a", "round.undo", { round: 4, mode: "edit" });
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("x1", "job.result", { jobId: undoJob(room).jobId, ok: true });
    expect(room.s.returning ?? []).toEqual([]);
  });

  it("重生：照原派工快照帶同一份 returns，不重算、不動 returning", async () => {
    const room = await returned();
    const first = lastDispatch(room);
    await room.send("a", "round.undo", { round: 4, mode: "regenerate" });
    await room.send("x1", "job.result", { jobId: undoJob(room).jobId, ok: true });
    const again = lastDispatch(room);
    expect(again.dispatchId).not.toBe(first.dispatchId);
    expect(again).toMatchObject({
      players: first.players,
      arrivals: ["c2"],
      departures: first.departures,
      returns: ["c2"],
    });
    expect(room.s.returning).toEqual([]);
  });
});

describe("一鍵代管 seat.delegate_many", () => {
  it("同一筆交易做完才結算：已送出的人照樣退回草稿、不被算進這回合（A 送出、B 沒送、C 送出，代管 B、C）", async () => {
    const room = await playing(["a", "b", "c"]);
    await room.send("a", "action.submit", { round: 2, text: "a 行動" });
    await room.send("c", "action.submit", { round: 2, text: "c 行動" });
    expect((await room.ask("b", "seat.delegate_many", { memberIds: ["m2", "m3"] }))?.p.code).toBe(
      "E_FORBIDDEN",
    );
    await room.send("a", "seat.delegate_many", { memberIds: ["m2", "m3"] });
    expect(room.events("round.resolved")).toHaveLength(1);
    expect(room.events("round.resolved")[0].payload.batch).toMatchObject([
      { memberId: "m1", kind: "act" },
    ]);
    expect(room.s.drafts.m3).toBe("c 行動");
    expect(lastDispatch(room)).toMatchObject({
      players: [{ cardId: "c1" }],
      departures: [
        { cardId: "c2", mode: "delegate" },
        { cardId: "c3", mode: "delegate" },
      ],
    });
  });

  it("其中一位不能代管（已在代管、沒座位）：整筆不動、回錯", async () => {
    const room = await playing(["a", "b", "c"]);
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    expect((await room.ask("a", "seat.delegate_many", { memberIds: ["m3", "m2"] }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    expect(room.s.members.find((m) => m.id === "m3")?.delegated).toBeFalsy();
    expect((await room.ask("a", "seat.delegate_many", { memberIds: [] }))?.p.code).toBe(
      "E_BAD_FRAME",
    );
  });
});

describe("手機畫面跟著座位異動清代管旗標", () => {
  /** 某位成員從加入那一刻起看到的畫面（第一份快照＋之後逐筆套事件） */
  const view = (room: Room, id: string) =>
    (room.out[id] ?? []).reduce<RoomView | null>(
      (v, f) => (f.t === "welcome" && v ? v : applyFrame(v, f)),
      null,
    )!;

  it("代管、離座、再認領：本人與名單都不是代管中", async () => {
    const room = await playing();
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    expect(view(room, "b").me.delegated).toBe(true);
    await room.send("b", "seat.release");
    await room.send("b", "seat.claim", { cardId: "c3" });
    for (const id of ["a", "b"])
      expect(view(room, id).members.find((m) => m.memberId === "m2")?.delegated).toBe(false);
    expect(view(room, "b").me).toMatchObject({ cardId: "c3", delegated: false });
  });

  it("代管中的座位轉給別人：來源與接手者在每支手機上都不是代管中", async () => {
    const room = await playing();
    await room.join("c");
    await room.send("b", "seat.delegate", { memberId: "m2", on: true });
    await room.send("a", "member.reassign", { from: "m2", to: "m3" });
    for (const id of ["a", "b", "c"]) {
      const v = view(room, id);
      expect(v.members.filter((m) => m.delegated)).toEqual([]);
    }
    expect(view(room, "b").me.delegated).toBe(false);
    expect(view(room, "c").me).toMatchObject({ cardId: "c2", delegated: false });
  });
});
