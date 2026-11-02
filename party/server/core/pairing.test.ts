// 執行端配對（協定 §1.2、§7.3）：配對碼暴力鎖、已有出額度者時的規則、遠端控制只認出額度者。
import { describe, expect, it } from "vitest";
import { playing, Room } from "./room-test-kit";

describe("配對碼", () => {
  it("全房累計錯 5 次鎖 10 分鐘（帶 retryAfterMs），鎖住期間正確的碼也不收；解鎖後可用", async () => {
    const room = await Room.create();
    await room.exec();
    await room.join("a");
    await room.join("evil");
    const code = await room.pairCode(false);
    for (let i = 0; i < 5; i++) expect((await room.ask("evil", "pair.claim", { code: "0000-0000" }))?.p.code).toBe("E_PAIR_CODE_INVALID");
    const locked = await room.ask("a", "pair.claim", { code });
    expect(locked?.p).toMatchObject({ code: "E_PAIR_LOCKED", retryAfterMs: 600_000 });
    room.clock += 600_000;
    await room.send("x1", "hb");
    const again = await room.pairCode(false); // 原碼與鎖同樣 10 分鐘，這時已過期
    expect((await room.ask("a", "pair.claim", { code: again.slice(0, 4) + "-" + again.slice(4).toLowerCase() }))?.t).toBe("ok");
    expect(room.s.payerId).toBe("m1");
    expect(room.s.hostId).toBeNull();
    // 一次性：用過即失效
    expect((await room.ask("evil", "pair.claim", { code: again }))?.p.code).toBe("E_PAIR_CODE_INVALID");
  });

  it("配對碼 10 分鐘過期、再發新碼舊碼作廢；已有出額度者時一般碼回 E_STATE_CONFLICT、grantHost 碼只給房主", async () => {
    const room = await Room.create();
    await room.exec();
    await room.join("a");
    await room.join("b");
    const expired = await room.pairCode(false);
    room.clock += 600_001;
    await room.send("x1", "hb");
    expect((await room.ask("a", "pair.claim", { code: expired }))?.p.code).toBe("E_PAIR_CODE_INVALID");
    const old = await room.pairCode(false);
    const fresh = await room.pairCode(false);
    expect((await room.ask("a", "pair.claim", { code: old }))?.p.code).toBe("E_PAIR_CODE_INVALID");
    await room.send("a", "pair.claim", { code: fresh });
    expect((await room.ask("b", "pair.claim", { code: await room.pairCode(false) }))?.p.code).toBe("E_STATE_CONFLICT");
    await room.send("b", "pair.claim", { code: await room.pairCode(true) });
    expect(room.s).toMatchObject({ payerId: "m1", hostId: "m2" });
    // 出額度者換手機：執行端撤銷後再配
    await room.send("x1", "pair.revoke");
    expect(room.s.payerId).toBeNull();
    expect(room.events("pair.unbound")).toHaveLength(1);
  });

  it("遠端控制只有出額度者能發；模型不在清單回 E_MODEL_NOT_OFFERED；30 秒沒回 E_EXECUTOR_TIMEOUT；executor.leave 一律拒絕", async () => {
    const room = await playing();
    expect((await room.ask("b", "ctl", { ctlId: "x", cmd: { kind: "refresh_quota" } }))?.p.code).toBe("E_FORBIDDEN");
    expect((await room.ask("a", "ctl", { ctlId: "y", cmd: { kind: "set_next", route: "or", model: "沒有這個" } }))?.p.code).toBe("E_MODEL_NOT_OFFERED");
    await room.send("a", "ctl", { ctlId: "z", cmd: { kind: "refresh_quota" } }, "zz");
    room.clock += 15_000;
    await room.send("x1", "hb");
    await room.tick(15_000);
    expect(room.frames("a", "err").at(-1)?.p).toMatchObject({ id: "zz", code: "E_EXECUTOR_TIMEOUT" });
    expect((await room.ask("x1", "executor.leave"))?.p.code).toBe("E_STATE_CONFLICT");
  });
});
