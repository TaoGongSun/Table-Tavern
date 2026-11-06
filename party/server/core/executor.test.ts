// 執行端 status 的幕容量提醒：可省欄位，伺服器蓋上房間幕號再投影給成員。
import { describe, expect, it } from "vitest";
import { playing, type Room } from "./room-test-kit";
import { buildSnapshot } from "./snapshot";
import type { Live } from "./types";

const LIVE: Live = { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} };

const hintOf = (room: Room) => {
  const frames = room.frames("a", "vol", "executor");
  return frames[frames.length - 1].p.capacityHint;
};

const status = (capacityHint?: boolean) => ({
  keepAwake: null,
  busy: false,
  ...(capacityHint === undefined ? {} : { capacityHint }),
});

describe("status.capacityHint", () => {
  it("帶 true 之後，成員的 vol.executor 與快照都是 true", async () => {
    const room = await playing();
    await room.send("x1", "status", status(true));
    expect(hintOf(room)).toBe(true);
    expect(buildSnapshot(room.s, room.store.events, "m1", LIVE).executor.capacityHint).toBe(true);
  });

  it("沒帶欄位的 status 不改既有值", async () => {
    const room = await playing();
    await room.send("x1", "status", status(true));
    await room.send("x1", "status", status());
    expect(hintOf(room)).toBe(true);
    expect(room.s.exec.capacity).toEqual({ scene: room.s.scene, hint: true });
  });

  it("scene.started 之後變 false（幕號對不上）", async () => {
    const room = await playing();
    await room.send("x1", "status", status(true));
    await room.send("a", "scene.advance", { round: 2 });
    const job = room.frames("x1", "job.scene_advance")[0].p;
    await room.send("x1", "job.result", {
      jobId: job.jobId,
      ok: true,
      result: { summary: "第一幕摘要" },
    });
    expect(room.events("scene.started").length).toBeGreaterThan(0);
    expect(hintOf(room)).toBe(false);
  });

  it("換幕之後再收一則 capacityHint:true 又為 true", async () => {
    const room = await playing();
    await room.send("x1", "status", status(true));
    await room.send("a", "scene.advance", { round: 2 });
    const job = room.frames("x1", "job.scene_advance")[0].p;
    await room.send("x1", "job.result", {
      jobId: job.jobId,
      ok: true,
      result: { summary: "第一幕摘要" },
    });
    expect(hintOf(room)).toBe(false);
    await room.send("x1", "status", status(true));
    expect(hintOf(room)).toBe(true);
    expect(room.s.exec.capacity).toEqual({ scene: room.s.scene, hint: true });
  });

  it("capacityHint 不是布林 → E_BAD_FRAME", async () => {
    const room = await playing();
    await room.send("x1", "status", { ...status(), capacityHint: "x" });
    const errs = room.frames("x1", "err");
    expect(errs[errs.length - 1].p.code).toBe("E_BAD_FRAME");
    expect(room.closed.x1).toBe(4400);
    expect(room.s.exec.capacity).toBeUndefined();
  });
});
