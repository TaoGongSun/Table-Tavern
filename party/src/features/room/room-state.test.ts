// 客戶端投影（room-speak-request）：換座時保留行動跟著卡走、即時畫面與重連快照一致；催促只收當前回合、同回合再催也算一次。
import { describe, expect, it } from "vitest";
import { playing, type Room } from "../../../server/core/room-test-kit";
import { buildSnapshot } from "../../../server/core/snapshot";
import { applyFrame, shownRound, type RoomView } from "./room-state";
import type { Frame } from "../../../server/core/types";

const live = (room: Room, conn: string) =>
  room.frames(conn).reduce<RoomView | null>((v, f) => applyFrame(v, f as never), null)!;
const snap = (room: Room, viewer: string) =>
  buildSnapshot(room.s, room.store.events, viewer, {
    stream: null,
    seed: null,
    buckets: {},
    uploads: {},
    cardReqs: {},
  });

describe("客戶端投影", () => {
  it("房主 GM 回合期間換座：carry 跟著卡給接手的人，即時畫面與重連快照一致", async () => {
    const room = await playing();
    await room.join("c");
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    await room.send("a", "host.gm", { round: 2, mode: "narrate" });
    await room.send("a", "member.reassign", { from: "m2", to: "m3" });
    const expected = [{ memberId: "m3", cardId: "c2", status: "submitted", text: "我推門" }];
    expect(room.s.carry).toEqual(expected);
    for (const [conn, viewer] of [
      ["a", "m1"],
      ["c", "m3"],
    ]) {
      expect(live(room, conn).round?.carry).toEqual(expected);
      expect(snap(room, viewer).round?.carry).toEqual(expected);
    }
  });

  it("催促：只收當前回合；同回合再催 n 會加一；舊回合的晚到催促不算", async () => {
    const room = await playing();
    await room.send("a", "host.nudge", { round: 2, memberId: "m2" });
    expect(live(room, "b").nudged).toEqual({ round: 2, n: 1 });
    await room.tick(30_000);
    await room.send("a", "host.nudge", { round: 2, memberId: "m2" });
    const v = live(room, "b");
    expect(v.nudged).toEqual({ round: 2, n: 2 });
    expect(applyFrame(v, { t: "vol", p: { type: "nudged", round: 1 } } as never)!.nudged).toEqual({
      round: 2,
      n: 2,
    });
  });

  it("聊天 vol 依 id 去重，客戶端只留最後 200 則", async () => {
    const room = await playing();
    await room.send("a", "chat.send", { text: "第一則" });
    let view = live(room, "a");
    const again = view.chat[0];
    const duped = applyFrame(view, { t: "vol", p: { type: "chat", msg: again } } as Frame)!;
    expect(duped.chat).toHaveLength(1);
    for (let i = 0; i < 205; i++) {
      view = applyFrame(view, {
        t: "vol",
        p: { type: "chat", msg: { id: 1000 + i, at: 1, from: "m2", to: null, text: "x" } },
      } as Frame)!;
    }
    expect(view.chat).toHaveLength(200);
    expect(view.chat[0].id).toBe(1005);
    expect(view.chat.at(-1)!.id).toBe(1204);
  });

  it("回合數給人看的是本幕第幾輪玩家行動：GM 鈕不跳號、收回改字回到同一輪，擲果列不算，即時與快照一致", async () => {
    const room = await playing();
    await room.send("x1", "status", { keepAwake: null, busy: false, diceEnabled: true });
    const shown = () => [shownRound(live(room, "a")), shownRound(snap(room, "m1") as never)];
    expect(shown()).toEqual([1, 1]); // 伺服器 acting(2)：開場那回合不算
    const round = room.s.round!.round;
    await room.ask("a", "action.roll", { round, formula: "1d20" });
    await room.send("a", "action.submit", { round, text: "我拔劍" });
    await room.send("b", "action.skip", { round });
    await room.finishGen();
    expect(shown()).toEqual([2, 2]);
    await room.send("a", "round.undo", { round, mode: "edit" });
    const job = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: job.jobId, ok: true });
    expect(room.s.round!.round).toBe(4);
    expect(shown()).toEqual([1, 1]); // 擲果列留在原位、帶被收回的回合號
    await room.send("a", "host.gm", { round: 4, mode: "narrate" });
    expect(shown()).toEqual([1, 1]);
    await room.finishGen();
    expect(room.s.round).toMatchObject({ round: 6, kind: "play" });
    expect(shown()).toEqual([1, 1]);
    // 只擲骰沒寫字也算一輪；重生上一回合數字不動
    await room.ask("a", "action.roll", { round: 6, formula: "1d20", submit: true });
    await room.send("b", "action.skip", { round: 6 });
    await room.finishGen();
    expect(shown()).toEqual([2, 2]);
    await room.send("a", "round.undo", { round: 6, mode: "regenerate" });
    const again = room.frames("x1", "job.undo").at(-1)!.p;
    await room.send("x1", "job.result", { jobId: again.jobId, ok: true });
    expect(shown()).toEqual([1, 1]); // 重生中：被收回的那輪重跑
    await room.finishGen();
    expect(shown()).toEqual([2, 2]);
  });
});
