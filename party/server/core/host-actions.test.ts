// 房主操作（room-host-actions）：GM 鈕 host.gm、收回／重生 round.undo。
import { describe, expect, it } from "vitest";
import { toWebSave } from "./export";
import { playing, type Room } from "./room-test-kit";
import { buildSnapshot } from "./snapshot";

describe("房主 GM 鈕（§3.1）", () => {
  it("送出一半時按旁白：已送出的行動留著、開 host 回合直接派工（不帶行動與登場），GM 跑完放回；GM 點到的 NPC 只告訴房主", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    await room.send("a", "host.gm", { round: 2, mode: "narrate" });
    expect(room.s.drafts.m2).toBeUndefined();
    expect(room.s.carry).toEqual([
      { memberId: "m2", cardId: "c2", status: "submitted", text: "我推門" },
    ]);
    expect(room.events("round.cancelled").at(-1)?.payload).toEqual({ round: 2, kept: true });
    expect(room.events("round.opened").at(-1)?.payload.carry).toEqual(room.s.carry);
    expect(room.s.round).toMatchObject({ round: 3, kind: "host", phase: "queued", roster: [] });
    const d = room.frames("x1", "dispatch").at(-1)!.p;
    expect(d).toMatchObject({ kind: "host", actions: [], arrivals: [], host: { mode: "narrate" } });
    expect(d.players).toHaveLength(2);
    const dispatchId = room.dispatchId();
    await room.send("x1", "gen.done", { dispatchId, aborted: false, called: "c3" });
    expect(room.s.round).toMatchObject({ round: 4, kind: "play", phase: "acting", called: "c3" });
    expect(room.s.round!.roster).toEqual([
      { memberId: "m1", cardId: "c1", status: "pending", text: null },
      { memberId: "m2", cardId: "c2", status: "submitted", text: "我推門" },
    ]);
    expect(room.s.carry).toBeNull();
    expect(room.frames("a", "vol", "called").at(-1)?.p).toEqual({
      type: "called",
      round: 4,
      cardId: "c3",
    });
    expect(room.frames("b", "vol", "called")).toEqual([]);
    // 「讓 X 接話」＝請 GM 剛點到的那張發言：寫點名列（call: true）；請別張不寫
    await room.send("a", "host.gm", { round: 4, mode: "call", cardId: "c3" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.host).toEqual({
      mode: "call",
      cardId: "c3",
      call: true,
    });
  });

  it("沒人入座也能按；請發言只限沒人認領、可上場、未隱藏的卡；執行端離線不給按", async () => {
    const room = await playing(["a"]);
    await room.send("a", "seat.release");
    expect(room.s.round).toMatchObject({ round: 2, phase: "acting", roster: [] });
    expect(
      (await room.ask("a", "host.gm", { round: 2, mode: "call", cardId: "npc" }))?.p.code,
    ).toBe("E_SEAT_TAKEN");
    await room.join("b");
    await room.send("b", "seat.claim", { cardId: "c2" });
    expect((await room.ask("b", "host.gm", { round: 2, mode: "advance" }))?.p.code).toBe(
      "E_FORBIDDEN",
    );
    expect((await room.ask("a", "host.gm", { round: 2, mode: "call", cardId: "c2" }))?.p.code).toBe(
      "E_SEAT_TAKEN",
    );
    await room.send("a", "host.gm", { round: 2, mode: "call", cardId: "c3" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.host).toEqual({
      mode: "call",
      cardId: "c3",
      call: false,
    });
    await room.finishGen("店主開口");
    await room.disconnect("x1");
    expect(
      (await room.ask("a", "host.gm", { round: room.s.round!.round, mode: "advance" }))?.p.code,
    ).toBe("E_EXECUTOR_OFFLINE");
  });

  it("acting 回合裡剛登場的人被 GM 鈕作廢後，下一個派工照樣帶登場", async () => {
    const room = await playing(["a"]);
    await room.send("a", "seat.release");
    await room.join("b");
    await room.send("b", "seat.claim", { cardId: "c2" }); // 名單空著停住：直接加進當前名單、算登場
    expect(room.s.round!.arrivals).toEqual(["c2"]);
    await room.send("a", "host.gm", { round: 2, mode: "narrate" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.arrivals).toEqual([]);
    await room.finishGen();
    expect(room.s.round!.arrivals).toEqual(["c2"]);
  });
});

/** a、b 都送出、結算、GM 跑完第 2 回合 → acting(3) */
async function played() {
  const room = await playing();
  await room.send("a", "action.submit", { round: 2, text: "我拔劍" });
  await room.send("b", "action.submit", { round: 2, text: "我推門" });
  await room.finishGen("門開了");
  return room;
}
const undoJob = (room: Room, exec = "x1") => room.frames(exec, "job.undo").at(-1)!.p;
const story = (room: Room, viewer = "m2") =>
  buildSnapshot(room.s, room.store.events, viewer, {
    stream: null,
    seed: null,
    buckets: {},
    uploads: {},
    cardReqs: {},
  }).story;

describe("收回與重生（§3.2）", () => {
  it("重生：等執行端收乾淨前回合指令都擋、全員送出也不結算；收完照原派工快照重跑同一批行動，故事串與匯出都不留舊段", async () => {
    const room = await played();
    const original = room.events("gen.dispatched").length;
    const first = room.frames("x1", "dispatch").at(-1)!.p;
    await room.send("a", "action.submit", { round: 3, text: "再一次" });
    expect(
      await room.ask("a", "round.undo", {
        round: 2,
        mode: "regenerate",
        next: { route: "or", model: "m-b" },
      }),
    ).toBeNull(); // 等執行端才回
    await room.send("b", "seat.release"); // 收回到重生之間的座位變動留給下一個正常派工
    expect(room.s.departures).toEqual([{ cardId: "c2", mode: "absent" }]);
    const job = undoJob(room);
    expect(job).toMatchObject({ kind: "undo", dispatchId: first.dispatchId });
    expect((await room.ask("a", "round.undo", { round: 2, mode: "regenerate" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    );
    expect((await room.ask("a", "action.retract", { round: 3 }))?.p.code).toBe(
      "E_ROUND_NOT_ACTING",
    );
    expect(room.s.round!.phase).toBe("acting"); // b 離座後名單全送出了，收回期間不結算
    await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
    expect(room.s.undo).toBeNull();
    expect(room.s.drafts.m1).toBe("再一次"); // 收回後開出的回合，已送出的退回草稿
    expect(room.events("round.undone").at(-1)?.payload).toEqual({ round: 2 });
    expect(room.events("scene.settings_switched").at(-1)?.payload).toMatchObject({
      reason: "regenerate",
      settings: { model: "m-b" },
    });
    expect(room.s.round).toMatchObject({ round: 4, kind: "play", phase: "queued" });
    expect(room.events("gen.dispatched")).toHaveLength(original + 1);
    const again = room.frames("x1", "dispatch").at(-1)!.p;
    expect(again).toMatchObject({
      round: 4,
      actions: first.actions,
      players: first.players,
      arrivals: first.arrivals,
      departures: first.departures,
      settings: { model: "m-b" },
    });
    expect(again.dispatchId).not.toBe(first.dispatchId);
    expect(room.s.departures).toEqual([{ cardId: "c2", mode: "absent" }]);
    await room.finishGen("門又開了");
    const texts = story(room).map((p) => p.text);
    expect(texts).not.toContain("門開了");
    expect(texts.filter((x) => x === "我推門")).toHaveLength(1);
    expect(
      story(room).some((p) => p.speaker.kind === "system" && p.speaker.marker === "round_undone"),
    ).toBe(true);
    const save = toWebSave(room.s, room.store.events, room.clock);
    expect(save.messages.map((m) => m.text).join("|")).not.toContain("門開了");
    expect(save.messages.filter((m) => m.text.endsWith("我推門"))).toHaveLength(1);
    expect((await room.ask("a", "round.undo", { round: 2, mode: "edit" }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    ); // 只收一層
  });

  it("收回改字：原文不論有沒有草稿一律只給本人（草稿不動），丟掉後清空；重連看得到", async () => {
    const room = await played();
    await room.send("b", "action.draft", { text: "新想法" });
    await room.send("a", "round.undo", { round: 2, mode: "edit" });
    await room.send("x1", "job.result", { jobId: undoJob(room).jobId, ok: true });
    expect(room.s.drafts).toEqual({ m2: "新想法" });
    expect(room.s.returned).toEqual({ m1: "我拔劍", m2: "我推門" });
    expect(room.frames("b", "vol", "returned").at(-1)?.p).toEqual({
      type: "returned",
      text: "我推門",
    });
    expect(room.frames("a", "vol", "returned").map((f) => f.p.text)).toEqual(["我拔劍"]);
    expect(room.s.round).toMatchObject({
      round: 4,
      phase: "acting",
      roster: [
        { memberId: "m1", status: "pending" },
        { memberId: "m2", status: "drafting" },
      ],
    });
    expect(
      buildSnapshot(room.s, room.store.events, "m2", {
        stream: null,
        seed: null,
        buckets: {},
        uploads: {},
        cardReqs: {},
      }).me,
    ).toMatchObject({ draft: "新想法", returned: "我推門" });
    // 斷線時被收回、帶 lastSeq 補事件回來（不給快照）也拿得到
    const seq = room.s.seq - 3;
    await room.disconnect("b");
    await room.connect("b2");
    await room.ask("b2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("b"),
      lastSeq: seq,
      client: {},
    });
    expect(room.frames("b2", "welcome")[0].p.snapshot).toBeUndefined();
    expect(room.frames("b2", "vol", "returned").at(-1)?.p.text).toBe("我推門");
    // 帶回：合併後太長存不進去＝原文照留（重開頁面的快照也還在）；存得進去才同一筆清掉原文
    expect(
      (await room.ask("b2", "action.returned_dismiss", { draft: "長".repeat(2001) }))?.p.code,
    ).toBe("E_TEXT_INVALID");
    expect(
      buildSnapshot(room.s, room.store.events, "m2", {
        stream: null,
        seed: null,
        buckets: {},
        uploads: {},
        cardReqs: {},
      }).me,
    ).toMatchObject({ draft: "新想法", returned: "我推門" });
    await room.send("b2", "action.returned_dismiss", { draft: "新想法\n我推門" });
    expect(room.s.drafts.m2).toBe("新想法\n我推門");
    expect(room.s.returned).toEqual({ m1: "我拔劍" });
    expect(room.frames("b2", "vol", "returned").at(-1)?.p.text).toBeNull();
    await room.send("a", "action.returned_dismiss"); // 丟掉
  });

  it("held 也能收；非出額度者不能換模型；收回中執行端斷線，重連的 welcome 帶這筆工作；執行端回失敗就不動、回 E_STATE_CONFLICT", async () => {
    const room = await played();
    await room.send("a", "action.skip", { round: 3 });
    await room.send("b", "action.skip", { round: 3 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "dispatch.ack", { dispatchId });
    await room.send("x1", "gen.failed", {
      dispatchId,
      atPart: 0,
      error: { kind: "refusal", code: "X", message: "x", actions: ["retry"], resetsAt: null },
    });
    expect(room.s.round).toMatchObject({ round: 3, phase: "held" });
    await room.send("a", "host.transfer", { memberId: "m2" });
    expect(
      (
        await room.ask("b", "round.undo", {
          round: 3,
          mode: "regenerate",
          next: { route: "or", model: "m-b" },
        })
      )?.p.code,
    ).toBe("E_FORBIDDEN");
    await room.send("b", "round.undo", { round: 3, mode: "edit" });
    const job = undoJob(room);
    expect((await room.ask("a", "ctl", { ctlId: "c1", cmd: { kind: "retry" } }))?.p.code).toBe(
      "E_STATE_CONFLICT",
    ); // 收回中不重試
    expect(room.s.round!.phase).toBe("held");
    await room.disconnect("x1");
    await room.exec("x2");
    expect(room.frames("x2", "welcome")[0].p.job).toMatchObject({ jobId: job.jobId, kind: "undo" });
    const reply = room.frames("b", "err").length;
    await room.send("x2", "job.result", {
      jobId: job.jobId,
      ok: false,
      error: { kind: "other", code: "UNDO_NOT_TAIL", message: "x", actions: [], resetsAt: null },
    });
    expect(room.frames("b", "err").slice(reply).at(-1)?.p.code).toBe("E_STATE_CONFLICT");
    expect(room.s.round).toMatchObject({ round: 3, phase: "held" });
    expect(room.s.undo).toBeNull();
    await room.send("b", "round.undo", { round: 3, mode: "edit" }); // 狀態沒動，可以再試
    await room.send("x2", "job.result", { jobId: undoJob(room, "x2").jobId, ok: true });
    expect(room.events("round.undone").at(-1)?.payload).toEqual({ round: 3 });
    expect(room.s.round).toMatchObject({ round: 4, phase: "acting" });
  });
});
