// 協定 §2／§3 每個狀態轉移各一條（房間生命週期、回合階段、中途加入與離開）。
import { describe, expect, it } from "vitest";
import { CARDS, playing, Room } from "./room-test-kit";

describe("房間生命週期（§2）", () => {
  it("建房 → lobby；成員帶加入密鑰加入、掃 grantHost 碼成為房主兼出額度者", async () => {
    const room = await Room.create();
    expect(room.s.status).toBe("lobby");
    expect(room.executorToken).toMatch(/^[A-Za-z0-9_-]{43}$/);
    await room.exec();
    await room.join("a", "阿明", { pairCode: await room.pairCode(true) });
    expect(room.frames("a", "welcome")[0].p.snapshot.me.memberId).toBe("m1");
    expect(room.s.hostId).toBe("m1");
    expect(room.s.payerId).toBe("m1");
    expect(room.frames("x1", "pair.claimed")[0].p).toMatchObject({
      memberId: "m1",
      grantHost: true,
    });
    expect((await room.join("b", "阿華", { joinSecret: "x".repeat(22) }))?.p.code).toBe(
      "E_AUTH_FAILED",
    );
    expect(room.closed.b).toBe(4401);
  });

  it("lobby → playing：開第 1 回合（open）直接派工，執行端 ack 後 generating，gen.done 開 acting(2)", async () => {
    const room = await playing();
    expect(room.s.status).toBe("playing");
    expect(room.events("round.opened").map((e) => e.payload.kind)).toEqual(["open", "play"]);
    expect(room.s.round).toMatchObject({
      round: 2,
      phase: "acting",
      roster: [{ memberId: "m1" }, { memberId: "m2" }],
    });
    expect(room.events("story.part")[0].payload).toMatchObject({
      round: 1,
      part: 0,
      speaker: { kind: "gm" },
      text: "開場",
    });
    // 串流增量只給成員，speaker 只在該段第一則帶
    expect(room.frames("b", "vol", "delta")[0].p).toMatchObject({
      round: 1,
      part: 0,
      speaker: { kind: "gm" },
      text: "開場",
    });
    expect(room.frames("x1", "vol", "delta")).toEqual([]);
  });

  it("playing → closed：所有連線收 room.closed 後關閉、執行端連不回來；7 天後整房刪除", async () => {
    const room = await playing();
    expect((await room.ask("b", "room.close"))?.p.code).toBe("E_FORBIDDEN");
    await room.send("a", "room.close");
    expect(room.s.status).toBe("closed");
    expect(room.frames("b", "ev", "room.closed")).toHaveLength(1);
    expect(room.closed).toMatchObject({ a: 4410, b: 4410, x1: 4410 });
    expect((await room.exec("x2"))?.p.code).toBe("E_ROOM_CLOSED");
    // 既有成員可唯讀回來，寫入回 E_ROOM_CLOSED；新人進不來
    await room.connect("b2");
    await room.ask("b2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("b"),
      lastSeq: 0,
      client: {},
    });
    expect(room.frames("b2", "welcome")).toHaveLength(1);
    expect((await room.ask("b2", "profile.rename", { nickname: "新名" }))?.p.code).toBe(
      "E_ROOM_CLOSED",
    );
    expect((await room.join("c"))?.p.code).toBe("E_ROOM_CLOSED");
    await room.tick(7 * 86_400_000);
    expect(room.store.state).toBeNull();
    expect(room.store.events).toEqual([]);
  });

  it("執行端憑證 HTTP 結束房間（桌卡「結束並刪除房間」）；錯憑證 401（已結束也一樣）、已結束再刪照樣 200；暫停狀態給成員看、重連解除", async () => {
    const room = await playing();
    await room.send("x1", "status", { keepAwake: null, busy: false, paused: true });
    await room.disconnect("x1");
    expect(room.frames("b", "vol", "executor").at(-1)?.p).toMatchObject({
      online: false,
      paused: true,
    });
    await room.exec("x2");
    expect(room.frames("b", "vol", "executor").at(-1)?.p).toMatchObject({
      online: true,
      paused: false,
    });
    const wrong = await room.run({ kind: "http", route: "close", executorToken: "t".repeat(43) });
    expect(wrong?.http?.status).toBe(401);
    const out = await room.run({ kind: "http", route: "close", executorToken: room.executorToken });
    expect(out?.http).toMatchObject({ status: 200 });
    expect(room.s.status).toBe("closed");
    expect(room.closed).toMatchObject({ b: 4410 });
    expect(
      (await room.run({ kind: "http", route: "close", executorToken: room.executorToken }))?.http
        ?.status,
    ).toBe(200);
    expect(
      (await room.run({ kind: "http", route: "close", executorToken: "t".repeat(43) }))?.http
        ?.status,
    ).toBe(401);
  });

  it("閒置 N 天（無持久事件、無新連線）整房刪除", async () => {
    const room = await Room.create();
    await room.tick(29 * 86_400_000);
    expect(room.store.state).not.toBeNull();
    await room.tick(86_400_000);
    expect(room.store.state).toBeNull();
  });
});

describe("回合階段（§3.1–§3.2）", () => {
  it("acting → queued：全員行動或跳過就結算（by all），批次照入座順序、行動送出即公開", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    expect(room.frames("a", "ev", "action.submitted")[0].p.e).toMatchObject({
      memberId: "m2",
      text: "我推門",
    });
    await room.send("a", "action.skip", { round: 2 });
    const resolved = room.events("round.resolved")[0].payload;
    expect(resolved).toMatchObject({
      by: "all",
      batch: [
        { memberId: "m1", kind: "watch", watchReason: "skipped" },
        { memberId: "m2", kind: "act", text: "我推門", name: "角色2" },
      ],
    });
    expect(room.s.round?.phase).toBe("queued");
    expect(room.frames("x1", "dispatch").at(-1)?.p).toMatchObject({
      round: 2,
      kind: "play",
      actions: resolved.batch,
      settings: { model: "m-a" },
    });
  });

  it("acting → queued：房主結算，沒送的人算觀望、草稿留著；收回可再改", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "第一版" });
    await room.send("b", "action.retract", { round: 2 });
    expect(room.s.drafts.m2).toBe("第一版");
    expect(room.s.round!.roster[1]).toMatchObject({ status: "drafting", text: null });
    expect((await room.ask("b", "round.resolve", { round: 2 }))?.p.code).toBe("E_FORBIDDEN");
    await room.send("a", "round.resolve", { round: 2 });
    expect(room.events("round.resolved")[0].payload.batch).toMatchObject([
      { watchReason: "host_resolved" },
      { watchReason: "host_resolved" },
    ]);
    expect(room.s.drafts.m2).toBe("第一版");
  });

  it("acting → queued：倒數到期比照房主結算，沒送出的一律算觀望（timeout）、草稿留著", async () => {
    const room = await playing();
    await room.send("a", "room.settings", { patch: { actionTimeoutSec: 60 } });
    await room.send("b", "action.submit", { round: 2, text: "x" });
    await room.send("b", "action.retract", { round: 2 });
    await room.send("a", "action.skip", { round: 2 });
    await room.send("a", "action.retract", { round: 2 });
    await room.send("a", "round.resolve", { round: 2 });
    await room.finishGen();
    expect(room.s.round).toMatchObject({ round: 3, deadline: room.clock + 60_000 });
    await room.send("b", "action.draft", { text: "草稿要送" });
    room.clock += 45_000;
    await room.send("x1", "hb");
    await room.tick(15_000);
    const batch = room.events("round.resolved")[1].payload.batch;
    expect(batch).toMatchObject([
      { kind: "watch", watchReason: "timeout" },
      { kind: "watch", watchReason: "timeout" },
    ]);
    expect(room.s.drafts.m2).toBe("草稿要送");
  });

  it("generating → held → retry 回 generating；held → round.continue 開 acting(r+1)；錯誤細節只給出額度者", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "dispatch.ack", { dispatchId });
    const error = {
      kind: "refusal",
      code: "AI_CONTENT_FILTERED",
      message: "被擋",
      actions: ["switch_model", "retry", "continue"],
      resetsAt: null,
    };
    await room.send("x1", "gen.failed", { dispatchId, atPart: 0, error });
    expect(room.s.round).toMatchObject({ phase: "held", hold: { kind: "refusal" } });
    expect(room.frames("a", "vol", "error_detail")[0].p.error.code).toBe("AI_CONTENT_FILTERED");
    expect(room.frames("b", "vol", "error_detail")).toEqual([]);
    expect(room.frames("b", "ev", "gen.failed")[0].p.e.error).toEqual({
      kind: "refusal",
      actions: ["switch_model", "retry", "continue"],
      resetsAt: null,
    });
    // 幕中切模型（唯一例外）：轉送時回 generating，執行端回 ok 後才記 settings_switched
    await room.send("a", "ctl", { ctlId: "c1", cmd: { kind: "switch_model", model: "m-b" } });
    expect(room.frames("x1", "ctl").at(-1)?.p).toMatchObject({
      ctlId: "c1",
      by: "m1",
      cmd: { kind: "switch_model", model: "m-b" },
    });
    expect(room.s.round?.phase).toBe("generating");
    await room.send("x1", "ctl.result", { ctlId: "c1", ok: true });
    expect(room.events("scene.settings_switched")[0].payload).toMatchObject({
      settings: { model: "m-b", modelLabel: "模型乙" },
      reason: "refusal",
    });
    await room.send("x1", "gen.failed", { dispatchId, atPart: 0, error });
    await room
      .send("b", "round.continue", { round: 2 })
      .then(() => expect(room.frames("b", "err").at(-1)?.p.code).toBe("E_FORBIDDEN"));
    await room.send("a", "round.continue", { round: 2 });
    expect(room.s.round).toMatchObject({ round: 3, phase: "acting" });
    // held 之後晚到的段落丟掉
    expect(
      (
        await room.ask("x1", "gen.part.end", {
          dispatchId,
          part: 0,
          text: "晚到",
          interrupted: false,
        })
      )?.p.code,
    ).toBe("E_DISPATCH_UNKNOWN");
  });

  it("generating → acting(r+1)：中止，執行端回半截段落＋gen.done{aborted}", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "dispatch.ack", { dispatchId });
    await room.send("a", "gen.abort", { round: 2 });
    expect(room.frames("x1", "abort").at(-1)?.p).toEqual({ dispatchId });
    await room.send("x1", "gen.part.begin", {
      dispatchId,
      part: 0,
      speaker: { kind: "npc", cardId: "npc", name: "店主" },
    });
    await room.send("x1", "gen.part.end", { dispatchId, part: 0, text: "半截", interrupted: true });
    await room.send("x1", "gen.done", { dispatchId, aborted: true });
    expect(room.events("story.part").at(-1)?.payload).toMatchObject({
      interrupted: true,
      speaker: { kind: "npc", cardId: "npc" },
    });
    expect(room.s.round).toMatchObject({ round: 3, phase: "acting" });
  });

  it("generating 15 分鐘沒有 gen.* 訊框 → held（E_GEN_TIMEOUT）", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    await room.send("x1", "dispatch.ack", { dispatchId: room.dispatchId() });
    for (let i = 0; i < 60; i++) {
      await room.send("x1", "hb");
      await room.tick(15_000);
    }
    expect(room.s.round?.phase).toBe("held");
    expect(room.events("gen.failed")[0].payload).toMatchObject({
      code: "E_GEN_TIMEOUT",
      error: { kind: "provider_unreachable" },
    });
  });

  it("執行端離線：不結算、倒數暫停；結算回 E_EXECUTOR_OFFLINE；回來後倒數照剩餘續跑、條件 1 立刻重判", async () => {
    const room = await playing();
    await room.send("a", "room.settings", { patch: { actionTimeoutSec: 60 } });
    await room.send("a", "round.resolve", { round: 2 });
    await room.finishGen();
    room.clock += 20_000;
    await room.disconnect("x1");
    expect(room.s.round).toMatchObject({ deadline: null, pausedRemainingMs: 40_000 });
    expect(room.frames("b", "vol", "executor").at(-1)?.p.online).toBe(false);
    expect((await room.ask("a", "round.resolve", { round: 3 }))?.p.code).toBe("E_EXECUTOR_OFFLINE");
    await room.send("a", "action.skip", { round: 3 });
    await room.send("b", "action.skip", { round: 3 });
    expect(room.s.round?.phase).toBe("acting");
    room.clock += 600_000;
    await room.exec("x2");
    expect(room.events("round.resolved")).toHaveLength(2);
    expect(room.events("round.deadline").map((e) => e.payload.pausedRemainingMs)).toEqual([
      40_000,
      null,
    ]);
    // hb 停了 30 秒也算離線
    await room.tick(30_000);
    expect(room.s.exec.online).toBe(false);
    expect(room.closed.x2).toBe(1001);
  });
});

describe("倒數與遠端控制的邊界（驗收第 1 輪）", () => {
  it("關房後倒數不到期、私人草稿不進紀錄與匯出；執行端心跳逾時與倒數同一次到期時先算離線、不結算", async () => {
    const room = await playing();
    await room.send("a", "room.settings", { patch: { actionTimeoutSec: 30 } });
    await room.send("a", "round.resolve", { round: 2 });
    await room.finishGen();
    await room.send("b", "action.draft", { text: "私人草稿" });
    // 心跳 30 秒逾時與倒數 30 秒同時到期
    await room.tick(30_000);
    expect(room.s.exec.online).toBe(false);
    expect(room.events("round.resolved")).toHaveLength(1);
    await room.exec("x2");
    await room.send("a", "room.close");
    expect(room.s.round).toMatchObject({ deadline: null, pausedRemainingMs: null });
    await room.tick(3_600_000);
    expect(room.events("round.resolved")).toHaveLength(1);
    const save = await room.run({ kind: "http", route: "export", deviceId: room.deviceId("a") });
    expect(JSON.stringify(save?.http?.body) + JSON.stringify(room.store.events)).not.toContain(
      "私人草稿",
    );
  });

  it("遠端控制 retry 逾時或執行端離線、而它之後沒送過 gen.*：回合退回原本的 held，不卡在 generating", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    const error = {
      kind: "quota",
      code: "AI_HTTP_STATUS_429",
      message: "",
      actions: ["retry"],
      resetsAt: null,
    };
    await room.send("x1", "gen.failed", { dispatchId, atPart: 0, error });
    await room.send("a", "ctl", { ctlId: "r1", cmd: { kind: "retry" } });
    expect(room.s.round?.phase).toBe("generating");
    room.clock += 15_000;
    await room.send("x1", "hb");
    await room.tick(15_000);
    expect(room.frames("a", "err").at(-1)?.p.code).toBe("E_EXECUTOR_TIMEOUT");
    expect(room.s.round).toMatchObject({ phase: "held", hold: { kind: "quota" } });
    await room.send("a", "ctl", { ctlId: "r2", cmd: { kind: "retry" } });
    await room.disconnect("x1");
    expect(room.s.round).toMatchObject({ phase: "held", hold: { kind: "quota" } });
  });

  it("名冊超過 1 MiB（分片建房累加）回 E_STATE_CONFLICT（roster_too_large），不關執行端連線", async () => {
    const room = await Room.create();
    await room.exec();
    const avatar = `data:image/png;base64,${"A".repeat(60_000)}`;
    const cards = (from: number) =>
      Array.from({ length: 10 }, (_, i) => ({ ...CARDS[0], cardId: `big${from + i}`, avatar }));
    await room.send("x1", "room.seed", { part: 0, total: 2, cards: cards(0) });
    expect(
      (await room.ask("x1", "room.seed", { part: 1, total: 2, cards: cards(10) }))?.p,
    ).toMatchObject({ code: "E_STATE_CONFLICT", detail: "roster_too_large" });
    expect(room.closed.x1).toBeUndefined();
  });
});

describe("中途加入與離開（§3.4）", () => {
  it("playing 中入座的人下一次 acting 才進 roster，該回合派工帶 arrivals；離開立刻移出並重判", async () => {
    const room = await playing();
    await room.join("c");
    await room.send("c", "seat.claim", { cardId: "c3" });
    expect(room.s.round!.roster.map((e) => e.memberId)).toEqual(["m1", "m2"]);
    await room.send("b", "action.submit", { round: 2, text: "走" });
    await room.send("a", "seat.release");
    // a 離座 → 名單只剩已送出的 b → 條件 1 成立直接結算
    expect(room.events("round.resolved")[0].payload.batch).toHaveLength(1);
    await room.finishGen();
    expect(room.s.round!.roster.map((e) => e.memberId)).toEqual(["m2", "m3"]);
    await room.send("a", "member.departure", { cardId: "c1", mode: "npc" });
    await room.send("b", "action.skip", { round: 3 });
    await room.send("c", "action.skip", { round: 3 });
    expect(room.frames("x1", "dispatch").at(-1)?.p).toMatchObject({
      arrivals: ["c3"],
      departures: [{ cardId: "c1", mode: "npc" }],
    });
  });

  it("名單為空不結算不派工：房主結算回 E_STATE_CONFLICT；有人入座直接加進當前名單", async () => {
    const room = await playing(["a"]);
    await room.send("a", "room.settings", { patch: { actionTimeoutSec: 30 } });
    await room.send("a", "action.skip", { round: 2 });
    await room.finishGen();
    await room.send("a", "seat.release");
    expect(room.s.round).toMatchObject({
      round: 3,
      roster: [],
      deadline: null,
      pausedRemainingMs: 30_000,
    });
    expect((await room.ask("a", "round.resolve", { round: 3 }))?.p.code).toBe("E_STATE_CONFLICT");
    room.clock += 3_600_000;
    await room.send("x1", "hb");
    await room.tick(0);
    expect(room.events("round.resolved")).toHaveLength(1);
    expect(room.frames("x1", "dispatch")).toHaveLength(2);
    await room.send("a", "seat.claim", { cardId: "c2" });
    expect(room.s.round).toMatchObject({
      roster: [{ memberId: "m1", cardId: "c2" }],
      arrivals: ["c2"],
      deadline: room.clock + 30_000,
    });
  });

  it("踢人與封鎖、換裝置移座位、名冊移除被認領的卡照離開處理", async () => {
    const room = await playing(["a", "b", "c"]);
    await room.send("b", "action.draft", { text: "b 的草稿" });
    await room.join("b-phone", "阿華新手機");
    await room.send("a", "member.reassign", { from: "m2", to: "m4" });
    expect(room.s.members.find((m) => m.id === "m4")).toMatchObject({ cardId: "c2" });
    expect(room.s.drafts.m4).toBe("b 的草稿");
    expect(room.s.round!.roster.map((e) => e.memberId)).toEqual(["m1", "m4", "m3"]);
    await room.send("a", "member.kick", { memberId: "m3", ban: true });
    expect(room.closed.c).toBe(4403);
    expect(room.frames("c", "err").at(-1)?.p.code).toBe("E_BANNED");
    expect((await room.join("c2-conn", "c", { deviceId: room.deviceId("c") }))?.p.code).toBe(
      "E_BANNED",
    );
    await room.send("x1", "roster.update", { cards: [CARDS[0], { ...CARDS[1], playable: false }] });
    expect(room.s.members.find((m) => m.id === "m4")?.cardId).toBeNull();
    expect(room.s.round!.roster.map((e) => e.memberId)).toEqual(["m1"]);
  });
});
