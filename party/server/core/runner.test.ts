// 殼的資料遺失路徑（協定 §12.2 唯一的故障注入）：commit 失敗時不廣播、狀態不前進。
import { describe, expect, it } from "vitest";
import { playing } from "./room-test-kit";

describe("RoomRunner 落地失敗", () => {
  it("commit 拋錯：不送任何 ev、只回發送者 E_INTERNAL；記憶體與 storage 都停在舊狀態，下一則照舊狀態算", async () => {
    const room = await playing();
    const before = structuredClone(room.s);
    const sent = (id: string) => (room.out[id] ?? []).length;
    const [a, b, x] = [sent("a"), sent("b"), sent("x1")];
    room.store.failNext = true;
    await room.send("b", "action.submit", { round: 2, text: "這句沒落地" });
    expect(room.out.b.slice(b)).toEqual([{ t: "err", p: { code: "E_INTERNAL" } }]);
    expect([sent("a"), sent("x1")]).toEqual([a, x]);
    expect(room.s).toEqual(before);
    // 重送同一句：照舊狀態（仍是 acting、b 未送出）算，成功後才廣播
    await room.send("b", "action.submit", { round: 2, text: "重送" });
    expect(room.frames("a", "ev", "action.submitted").map((f) => f.p.e.text)).toEqual(["重送"]);
    expect(room.events("action.submitted").map((e) => e.seq)).toEqual([before.seq + 1]);
  });
});
