// 房主操作（room-host-actions）：GM 鈕 host.gm。
import { describe, expect, it } from "vitest";
import { playing } from "./room-test-kit";

describe("房主 GM 鈕（§3.1）", () => {
  it("送出一半時按旁白：已送出的退回草稿、開 host 回合直接派工（不帶行動與登場），GM 點到的 NPC 只告訴房主", async () => {
    const room = await playing();
    await room.send("b", "action.submit", { round: 2, text: "我推門" });
    await room.send("a", "host.gm", { round: 2, mode: "narrate" });
    expect(room.s.drafts.m2).toBe("我推門");
    expect(room.events("round.cancelled").at(-1)?.payload).toEqual({ round: 2 });
    expect(room.s.round).toMatchObject({ round: 3, kind: "host", phase: "queued", roster: [] });
    const d = room.frames("x1", "dispatch").at(-1)!.p;
    expect(d).toMatchObject({ kind: "host", actions: [], arrivals: [], host: { mode: "narrate" } });
    expect(d.players).toHaveLength(2);
    const dispatchId = room.dispatchId();
    await room.send("x1", "gen.done", { dispatchId, aborted: false, called: "c3" });
    expect(room.s.round).toMatchObject({ round: 4, kind: "play", phase: "acting", called: "c3" });
    expect(room.frames("a", "vol", "called").at(-1)?.p).toEqual({ type: "called", round: 4, cardId: "c3" });
    expect(room.frames("b", "vol", "called")).toEqual([]);
    // 「讓 X 接話」＝請 GM 剛點到的那張發言：寫點名列（call: true）；請別張不寫
    await room.send("a", "host.gm", { round: 4, mode: "call", cardId: "c3" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.host).toEqual({ mode: "call", cardId: "c3", call: true });
  });

  it("沒人入座也能按；請發言只限沒人認領、可上場、未隱藏的卡；執行端離線不給按", async () => {
    const room = await playing(["a"]);
    await room.send("a", "seat.release");
    expect(room.s.round).toMatchObject({ round: 2, phase: "acting", roster: [] });
    expect((await room.ask("a", "host.gm", { round: 2, mode: "call", cardId: "npc" }))?.p.code).toBe("E_SEAT_TAKEN");
    await room.join("b");
    await room.send("b", "seat.claim", { cardId: "c2" });
    expect((await room.ask("b", "host.gm", { round: 2, mode: "advance" }))?.p.code).toBe("E_FORBIDDEN");
    expect((await room.ask("a", "host.gm", { round: 2, mode: "call", cardId: "c2" }))?.p.code).toBe("E_SEAT_TAKEN");
    await room.send("a", "host.gm", { round: 2, mode: "call", cardId: "c3" });
    expect(room.frames("x1", "dispatch").at(-1)!.p.host).toEqual({ mode: "call", cardId: "c3", call: false });
    await room.finishGen("店主開口");
    await room.disconnect("x1");
    expect((await room.ask("a", "host.gm", { round: room.s.round!.round, mode: "advance" }))?.p.code).toBe("E_EXECUTOR_OFFLINE");
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
