// 併發重點（協定 §12.2）：回合號＝版本號、同一回合只派一次、重連不重派、結算只一次、換幕中斷線以同 jobId 收尾。
import { describe, expect, it } from "vitest";
import { playing } from "./room-test-kit";

describe("回合版本與單飛派工", () => {
  it("過期回合拒絕（E_ROUND_STALE）、結算後與生成中送出拒絕（E_ROUND_NOT_ACTING），草稿照收", async () => {
    const room = await playing();
    await room.send("a", "action.skip", { round: 2 });
    await room.send("b", "action.skip", { round: 2 });
    expect((await room.ask("b", "action.submit", { round: 2, text: "結算後" }))?.p.code).toBe(
      "E_ROUND_NOT_ACTING",
    );
    expect((await room.ask("b", "action.submit", { round: 3, text: "還沒開" }))?.p.code).toBe(
      "E_ROUND_STALE",
    );
    await room.send("x1", "dispatch.ack", { dispatchId: room.dispatchId() });
    expect((await room.ask("b", "action.submit", { round: 2, text: "生成中" }))?.p.code).toBe(
      "E_ROUND_NOT_ACTING",
    );
    room.clock += 600;
    expect((await room.ask("b", "action.draft", { text: "可以先打字" }))?.t).toBe("ok");
    await room.finishGen();
    expect((await room.ask("b", "action.submit", { round: 2, text: "過期" }))?.p.code).toBe(
      "E_ROUND_STALE",
    );
    expect(room.s.round!.roster[1]).toMatchObject({ memberId: "m2", status: "drafting" });
  });

  it("全員行動與房主結算同時到：只結算一次、只派一次", async () => {
    const room = await playing();
    await room.send("a", "action.submit", { round: 2, text: "a" });
    const [last, host] = await Promise.all([
      room.send("b", "action.submit", { round: 2, text: "b" }, "last"),
      room.send("a", "round.resolve", { round: 2 }, "host"),
    ]);
    expect(last?.events.map((e) => e.type)).toContain("round.resolved");
    expect(host?.sends.find((s) => s.frame.t === "err")?.frame.p).toMatchObject({
      id: "host",
      code: "E_ROUND_NOT_ACTING",
    });
    expect(room.events("round.resolved")).toHaveLength(1);
    expect(room.events("gen.dispatched").filter((e) => e.payload.round === 2)).toHaveLength(1);
  });

  it("同一回合只派一次：重送、retry 都沿用同一個 dispatchId；重複的段落與 done 冪等收下", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "dispatch.ack", { dispatchId });
    await room.send("x1", "gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await room.send("x1", "gen.part.end", { dispatchId, part: 0, text: "一", interrupted: false });
    expect(
      (
        await room.ask("x1", "gen.part.end", {
          dispatchId,
          part: 0,
          text: "一",
          interrupted: false,
        })
      )?.t,
    ).toBe("ok");
    await room.send("x1", "gen.failed", {
      dispatchId,
      atPart: 1,
      error: {
        kind: "quota",
        code: "AI_HTTP_STATUS_429",
        message: "",
        actions: ["retry"],
        resetsAt: null,
      },
    });
    await room.send("a", "ctl", { ctlId: "r1", cmd: { kind: "retry" } });
    expect(room.frames("x1", "ctl").at(-1)?.p.cmd).toEqual({ kind: "retry" });
    await room.send("x1", "gen.part.begin", { dispatchId, part: 1, speaker: { kind: "gm" } });
    await room.send("x1", "gen.part.end", { dispatchId, part: 1, text: "二", interrupted: false });
    await room.send("x1", "gen.done", { dispatchId, aborted: false });
    expect((await room.ask("x1", "gen.done", { dispatchId, aborted: false }))?.t).toBe("ok");
    expect(
      room
        .events("story.part")
        .filter((e) => e.payload.round === 2)
        .map((e) => e.payload.part),
    ).toEqual([0, 1]);
    expect(room.events("gen.dispatched").filter((e) => e.payload.round === 2)).toHaveLength(1);
    expect(room.events("round.opened").at(-1)?.payload.round).toBe(3);
  });
});

describe("執行端重連不重派（InflightReport 三種情況，§6.5）", () => {
  async function droppedMidRound() {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.disconnect("x1");
    return { room, dispatchId };
  }
  const reconnect = (room: Awaited<ReturnType<typeof playing>>, inflight: unknown) =>
    room.exec("x2", { inflight, lastSeq: room.s.seq });

  it("同一 dispatchId 且 running：welcome 帶原派工、直接當 generating，不產生新派工", async () => {
    const { room, dispatchId } = await droppedMidRound();
    await reconnect(room, {
      dispatch: { dispatchId, state: "running", lastEndedPart: null },
      job: null,
    });
    expect(room.frames("x2", "welcome")[0].p.dispatch.dispatchId).toBe(dispatchId);
    expect(room.s.round?.phase).toBe("generating");
    expect(room.events("gen.dispatched")).toHaveLength(2);
  });

  it("同一 dispatchId 且 done：執行端只重送佇列，伺服器照收、推進到下一回合", async () => {
    const { room, dispatchId } = await droppedMidRound();
    await reconnect(room, { dispatch: { dispatchId, state: "done", lastEndedPart: 0 }, job: null });
    await room.send("x2", "gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await room.send("x2", "gen.part.end", {
      dispatchId,
      part: 0,
      text: "斷線前寫好的",
      interrupted: false,
    });
    await room.send("x2", "gen.done", { dispatchId, aborted: false });
    expect(room.s.round).toMatchObject({ round: 3, phase: "acting" });
    expect(room.events("gen.dispatched")).toHaveLength(2);
  });

  it("不同或沒有：welcome 照樣給同一個 dispatchId（執行端用 action_id 去重），伺服器不重產", async () => {
    const { room, dispatchId } = await droppedMidRound();
    await reconnect(room, {
      dispatch: { dispatchId: "OLD", state: "done", lastEndedPart: 3 },
      job: null,
    });
    expect(room.frames("x2", "welcome")[0].p.dispatch.dispatchId).toBe(dispatchId);
    expect(room.s.round?.phase).toBe("queued");
    await room.send("x2", "dispatch.ack", { dispatchId });
    expect(room.s.round?.phase).toBe("generating");
    expect(room.events("gen.dispatched")).toHaveLength(2);
    // 落後 ≤ 500 筆：逐筆補 ev，不給快照
    expect(room.frames("x2", "welcome")[0].p.snapshot).toBeUndefined();
  });
});

describe("重連與同憑證取代（§4.3、§1.2）", () => {
  it("落後多於 500 筆或第一次給快照；同裝置第二條連線取代舊的；快照不帶別人的草稿", async () => {
    const room = await playing();
    room.clock += 600;
    await room.send("b", "action.draft", { text: "秘密草稿" });
    await room.connect("a2");
    await room.ask("a2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("a"),
      lastSeq: 0,
      client: {},
    });
    expect(room.closed.a).toBe(4409);
    expect(room.frames("a", "err").at(-1)?.p.code).toBe("E_SESSION_REPLACED");
    const snap = room.frames("a2", "welcome")[0].p.snapshot;
    expect(snap.me).toMatchObject({ memberId: "m1", isHost: true, isPayer: true, draft: null });
    expect(JSON.stringify(snap)).not.toContain("秘密草稿");
    expect(JSON.stringify(snap)).not.toContain(room.deviceId("b"));
    expect(snap.story.map((p: { text: string }) => p.text)).toEqual(["開場"]);
    expect(snap.round.roster[1].status).toBe("drafting");
    await room.connect("b2");
    await room.ask("b2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("b"),
      lastSeq: 5,
      client: {},
    });
    expect(room.frames("b2", "ev").map((f) => f.p.seq)).toEqual(
      room.store.events.filter((e) => e.seq > 5).map((e) => e.seq),
    );
    expect(room.frames("b2", "welcome")[0].p.snapshot).toBeUndefined();
  });

  it("補事件的重連也帶當下即時值：執行端在線、成員在線、草稿狀態、粗額度、串流中那一段", async () => {
    const room = await playing();
    await room.send("x1", "quota", {
      route: "or",
      routeLabel: "OpenRouter",
      window: "credits",
      state: "near",
      usedRatio: null,
      resetsAt: null,
      creditsLeft: null,
      observedAt: room.clock,
    });
    await room.send("a", "action.draft", { text: "打字中" });
    await room.send("b", "action.skip", { round: 2 });
    await room.send("a", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await room.send("x1", "gen.delta", { dispatchId, part: 0, text: "說到一半" });
    const lastSeq = room.s.seq;
    await room.disconnect("x1");
    await room.connect("b2");
    await room.ask("b2", "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId("b"),
      lastSeq,
      client: {},
    });
    const welcome = room.frames("b2", "welcome")[0].p;
    expect(welcome.snapshot).toBeUndefined();
    expect(welcome.live).toMatchObject({
      executor: { online: false },
      presence: [
        { memberId: "m1", connected: true },
        { memberId: "m2", connected: true },
      ],
      rosterStatus: [
        { memberId: "m1", status: "drafting" },
        { memberId: "m2", status: "skipped" },
      ],
      quota: { state: "near", resetsAt: null },
      streaming: { round: 2, part: 0, text: "說到一半" },
    });
  });
});

describe("只給出額度者的值（包 4 對接）", () => {
  const rejoin = async (
    room: Awaited<ReturnType<typeof playing>>,
    conn: string,
    who: string,
    lastSeq: number,
  ) => {
    await room.connect(conn);
    await room.ask(conn, "hello", {
      protocol: 1,
      role: "member",
      deviceId: room.deviceId(who),
      lastSeq,
      client: {},
    });
    return room.frames(conn, "welcome")[0].p;
  };

  it("路線與模型清單：快照、vol.executor、補事件 welcome.live.executor 都只對出額度者帶 routes", async () => {
    const room = await playing();
    await room.send("x1", "status", { keepAwake: true, busy: false });
    expect(
      room
        .frames("a", "vol", "executor")
        .at(-1)
        ?.p.routes[0].models.map((m: { id: string }) => m.id),
    ).toEqual(["m-a", "m-b"]);
    expect(room.frames("b", "vol", "executor").at(-1)?.p.routes).toBeUndefined();
    expect((await rejoin(room, "a2", "a", 0)).snapshot.routes[0].route).toBe("or");
    expect((await rejoin(room, "b2", "b", 0)).snapshot.routes).toBeUndefined();
    expect((await rejoin(room, "a3", "a", room.s.seq - 1)).live.executor.routes[0].route).toBe(
      "or",
    );
    expect((await rejoin(room, "b3", "b", room.s.seq - 1)).live.executor.routes).toBeUndefined();
  });

  it("錯誤細節重連後還在：快照與補事件的 live 對出額度者帶最近一筆，別人看不到；開下一回合後清掉", async () => {
    const room = await playing();
    await room.send("a", "round.resolve", { round: 2 });
    const error = {
      kind: "refusal",
      code: "AI_CONTENT_FILTERED",
      message: "被擋",
      actions: ["retry", "continue"],
      resetsAt: null,
    };
    await room.send("x1", "gen.failed", { dispatchId: room.dispatchId(), atPart: 0, error });
    expect((await rejoin(room, "a2", "a", 0)).snapshot.errorDetail).toEqual({ round: 2, error });
    expect((await rejoin(room, "a3", "a", room.s.seq - 1)).live.errorDetail).toEqual({
      round: 2,
      error,
    });
    const other = await rejoin(room, "b2", "b", 0);
    expect(other.snapshot.errorDetail).toBeUndefined();
    expect(JSON.stringify(other)).not.toContain("AI_CONTENT_FILTERED");
    await room.send("a3", "round.continue", { round: 2 });
    expect((await rejoin(room, "a4", "a", 0)).snapshot.errorDetail).toBeNull();
  });
});

describe("換幕（§3.5）", () => {
  it("acting → transition → 執行端回摘要 → scene.summary＋scene.started → scene_open 回合；已送出的行動退回草稿", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "送出了" });
    await room.send("a", "ctl", {
      ctlId: "n1",
      cmd: { kind: "set_next", route: "or", model: "m-b" },
    });
    await room.send("a", "scene.advance", { round: 2 });
    expect(room.s.status).toBe("transition");
    expect(room.s.drafts.m2).toBe("送出了");
    const job = room.frames("x1", "job.scene_advance")[0].p;
    expect(job).toMatchObject({ scene: 1, nextSettings: { model: "m-b" } });
    expect((await room.ask("b", "action.submit", { round: 2, text: "x" }))?.p.code).toBe(
      "E_ROUND_STALE",
    );
    await room.send("x1", "job.ack", { jobId: job.jobId });
    await room.send("x1", "job.result", {
      jobId: job.jobId,
      ok: true,
      result: { summary: "第一幕摘要" },
    });
    expect(room.events("scene.started")[0].payload).toMatchObject({
      scene: 2,
      settings: { model: "m-b" },
    });
    expect(room.s).toMatchObject({
      status: "playing",
      scene: 2,
      round: { round: 3, kind: "scene_open", phase: "queued" },
    });
    expect(
      (
        await room.ask("x1", "job.result", {
          jobId: job.jobId,
          ok: true,
          result: { summary: "重送" },
        })
      )?.t,
    ).toBe("ok");
    expect(room.events("scene.summary")).toHaveLength(1);
  });

  it("換幕中執行端離線：房間停在 transition，重連後 welcome 帶同一個 jobId，以它收尾", async () => {
    const room = await playing();
    await room.send("a", "scene.advance", { round: 2 });
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    await room.disconnect("x1");
    room.clock += 3_600_000;
    await room.tick(0);
    expect(room.s.status).toBe("transition");
    await room.exec("x2", { inflight: { dispatch: null, job: { jobId, state: "running" } } });
    expect(room.frames("x2", "welcome")[0].p.job).toMatchObject({ jobId, kind: "scene_advance" });
    await room.send("x2", "job.result", { jobId, ok: true, result: { summary: "摘要" } });
    expect(room.s.scene).toBe(2);
  });

  it("15 分鐘沒有 job.* → stalled；房主取消 → job.cancel；執行端回失敗 → scene.failed、同一幕重開 acting", async () => {
    const room = await playing();
    await room.send("a", "scene.advance", { round: 2 });
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    expect((await room.ask("a", "scene.advance_cancel"))?.p.code).toBe("E_STATE_CONFLICT");
    for (let i = 0; i < 60; i++) {
      await room.send("x1", "hb");
      await room.tick(15_000);
    }
    expect(room.s.transition?.stalled).toBe(true);
    expect(room.events("scene.stalled")).toHaveLength(1);
    await room.send("a", "scene.advance_cancel");
    expect(room.frames("x1", "job.cancel")[0].p).toEqual({ jobId });
    await room.send("x1", "job.result", {
      jobId,
      ok: false,
      error: { kind: "other", code: "ABORTED", message: "", actions: [], resetsAt: null },
    });
    expect(room.events("scene.failed")).toHaveLength(1);
    expect(room.s).toMatchObject({
      status: "playing",
      scene: 1,
      round: { round: 3, kind: "play", phase: "acting" },
    });
  });

  it("換幕容量鎖：held(scene_full) 只能換幕；換幕帶 next 只有房主兼出額度者可以", async () => {
    const room = await playing();
    await room.send("a", "host.transfer", { memberId: "m2" });
    await room.send("b", "round.resolve", { round: 2 });
    const dispatchId = room.dispatchId();
    await room.send("x1", "gen.failed", {
      dispatchId,
      atPart: 0,
      error: {
        kind: "scene_full",
        code: "scene_capacity_full",
        message: "",
        actions: ["scene_advance"],
        resetsAt: null,
      },
    });
    expect(
      (await room.ask("a", "ctl", { ctlId: "r", cmd: { kind: "switch_model", model: "m-b" } }))?.p
        .code,
    ).toBe("E_STATE_CONFLICT");
    expect(
      (await room.ask("b", "scene.advance", { round: 2, next: { route: "or", model: "m-b" } }))?.p
        .code,
    ).toBe("E_FORBIDDEN");
    await room.send("b", "scene.advance", { round: 2 });
    expect(room.s.status).toBe("transition");
  });
});
