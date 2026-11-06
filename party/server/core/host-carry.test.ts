// 房主 GM 鈕保留已送出的行動（room-speak-request §3）與催促（§1）：carry 的存入、各出口恢復、失效處理、結算。
import { describe, expect, it } from "vitest";
import { playing, type Room } from "./room-test-kit";
import { buildSnapshot } from "./snapshot";

const fail = (room: Room, kind = "other") =>
  room.send("x1", "gen.failed", {
    dispatchId: room.dispatchId(),
    atPart: 0,
    error: { kind, code: "X", message: "x", actions: ["retry", "continue"], resetsAt: null },
  });
const snap = (room: Room, viewer: string) =>
  buildSnapshot(room.s, room.store.events, viewer, {
    stream: null,
    seed: null,
    buckets: {},
    uploads: {},
    cardReqs: {},
  });
const b = (room: Room) => room.s.round!.roster.find((e) => e.memberId === "m2");

/** b 送出、a（房主）還沒送就按 GM 旁白 → host(3) queued */
async function hosted() {
  const room = await playing();
  await room.send("b", "action.submit", { round: 2, text: "我推門" });
  await room.send("a", "host.gm", { round: 2, mode: "narrate" });
  return room;
}

describe("房主 GM 鈕保留已送出的行動", () => {
  it("host 回合失敗：held 期間 carry 不動、快照帶著；按繼續就放回；重送繼續不會再套一次", async () => {
    const room = await hosted();
    await room.send("x1", "dispatch.ack", { dispatchId: room.dispatchId() });
    await fail(room);
    expect(room.s.round).toMatchObject({ round: 3, phase: "held" });
    expect(snap(room, "m2").round?.carry).toEqual([
      { memberId: "m2", cardId: "c2", status: "submitted", text: "我推門" },
    ]);
    await room.send("a", "round.continue", { round: 3 });
    expect(b(room)).toMatchObject({ status: "submitted", text: "我推門" });
    expect(room.s.carry).toBeNull();
    expect((await room.ask("a", "round.continue", { round: 3 }))?.p.code).toBe("E_ROUND_STALE");
    expect(snap(room, "m2").round?.carry).toBeUndefined();
  });

  it("host 回合期間等待者改代管、送出者換座：放回時全員已送出就立刻結算，行動跟著卡走", async () => {
    const room = await playing(["a", "b", "c"]);
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    await room.send("c", "action.skip", { round: 2 });
    await room.send("a", "host.gm", { round: 2, mode: "advance" });
    await room.send("a", "seat.delegate", { memberId: "m1", on: true });
    await room.send("b", "seat.release");
    await room.join("d");
    await room.send("a", "member.reassign", { from: "m3", to: "m4" }); // c 的座位交給 d
    await room.finishGen();
    // b 離座：原文還給本人；c 的「交給 GM」跟著卡給 d；a 代管不在名單 → 全員已送出，立刻結算
    expect(room.s.returned).toEqual({ m2: "我推門" });
    expect(room.frames("b", "vol", "returned").at(-1)?.p).toEqual({
      type: "returned",
      text: "我推門",
    });
    expect(room.s.round).toMatchObject({ round: 4, phase: "queued" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.actions).toMatchObject([
      { memberId: "m4", cardId: "c3", kind: "watch", watchReason: "skipped" },
    ]);
  });

  it("收回 host 回合：在後面的 acting 收回以結果到達當下的名單為準；edit 當場放回，regenerate 等重跑完", async () => {
    const room = await hosted();
    await room.finishGen("巴頓開口");
    await room.send("b", "action.retract", { round: 4 });
    await room.send("b", "action.submit", { round: 4, text: "我改走後門" });
    await room.send("a", "round.undo", { round: 3, mode: "regenerate" });
    let job = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
    expect(room.s.drafts.m2).toBeUndefined();
    expect(room.s.round).toMatchObject({ round: 5, kind: "host", phase: "queued" });
    expect(room.s.carry).toEqual([
      { memberId: "m2", cardId: "c2", status: "submitted", text: "我改走後門" },
    ]);
    await room.finishGen("巴頓又開口");
    expect(b(room)).toMatchObject({ status: "submitted", text: "我改走後門" });
    await room.send("a", "round.undo", { round: 5, mode: "edit" });
    job = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
    expect(room.s.round).toMatchObject({ round: 7, kind: "play", phase: "acting" });
    expect(b(room)).toMatchObject({ status: "submitted", text: "我改走後門" });
    expect(room.s.returned).toBeUndefined(); // host 派工沒有行動，edit 不另外還原文
  });

  it("held 的 host 回合重生：carry 帶到新的 host 回合，跑完放回", async () => {
    const room = await hosted();
    await room.send("x1", "dispatch.ack", { dispatchId: room.dispatchId() });
    await fail(room);
    await room.send("a", "round.undo", { round: 3, mode: "regenerate" });
    const job = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
    expect(room.s.round).toMatchObject({ round: 4, kind: "host" });
    expect(room.events("round.opened").at(-1)?.payload.carry).toHaveLength(1);
    await room.finishGen();
    expect(b(room)).toMatchObject({ status: "submitted", text: "我推門" });
  });

  it("host 回合卡在這一幕太長而換幕：carry 的原文還給本人，不覆蓋剛打的草稿", async () => {
    const room = await hosted();
    await room.send("b", "action.draft", { text: "新想法" });
    await room.send("x1", "dispatch.ack", { dispatchId: room.dispatchId() });
    await fail(room, "scene_full");
    await room.send("a", "scene.advance", { round: 3 });
    expect(room.s.carry).toBeNull();
    expect(room.s.drafts.m2).toBe("新想法");
    expect(room.s.returned).toEqual({ m2: "我推門" });
  });
});

describe("房主催促（§1）", () => {
  it("只有房主、只催名單上還沒送出且在線的人；只私下通知本人、同一對象 30 秒一次", async () => {
    const room = await playing(["a", "b", "c"]);
    expect((await room.ask("b", "host.nudge", { round: 2, memberId: "m3" }))?.p.code).toBe(
      "E_FORBIDDEN",
    );
    await room.send("a", "host.nudge", { round: 2, memberId: "m2" });
    expect(room.frames("b", "vol", "nudged")).toEqual([
      { t: "vol", p: { type: "nudged", round: 2 } },
    ]);
    expect(room.frames("c", "vol", "nudged")).toEqual([]);
    expect((await room.ask("a", "host.nudge", { round: 2, memberId: "m2" }))?.p.code).toBe(
      "E_RATE_LIMITED",
    );
    await room.send("c", "action.submit", { round: 2, text: "我坐下" });
    expect((await room.ask("a", "host.nudge", { round: 2, memberId: "m3" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    await room.tick(30_000);
    await room.disconnect("b");
    expect((await room.ask("a", "host.nudge", { round: 2, memberId: "m2" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
  });
});
