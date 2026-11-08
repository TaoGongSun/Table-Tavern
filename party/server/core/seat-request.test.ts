// 換裝置認領座位：玩家認領、房主確認，暱稱不是憑證。
import { describe, expect, it } from "vitest";
import { applyFrame, type RoomView } from "../../src/features/room/room-state";
import { playing, type Room } from "./room-test-kit";
import { buildSnapshot } from "./snapshot";

const LIVE = { stream: null, seed: null, buckets: {}, uploads: {}, cardReqs: {} };
/** a 房主兼出額度者（c1）、b 坐 c2 後離線、d 坐 c3 後離線；c 是新裝置 */
async function room3() {
  const room = await playing(["a", "b", "d"]);
  await room.disconnect("b");
  await room.disconnect("d");
  await room.join("c", "格羅");
  return room;
}
const member = (room: Room, nick: string) => room.s.members.find((m) => m.nickname === nick);
const code = async (room: Room, id: string, t: string, p: unknown) =>
  (await room.ask(id, t, p))?.p.code;
const requests = (room: Room) => room.frames("a", "vol", "seat_requests").at(-1)!.p.list;

describe("認領座位", () => {
  it("確認：一個訊息移座＋移除舊列；舊裝置回來提示一次、是沒座位的新成員", async () => {
    const room = await room3();
    await room.send("c", "seat.request", { cardId: "c2" });
    const [r] = requests(room);
    expect(r).toMatchObject({ memberId: "m4", cardId: "c2", occupantId: "m2" });
    expect(r.offlineSince).toBeTypeOf("number");
    const before = room.s.seq;
    await room.send("a", "seat.request.resolve", {
      memberId: "m4",
      requestId: r.requestId,
      accept: true,
    });
    expect(member(room, "b")).toBeUndefined();
    expect(member(room, "格羅")).toMatchObject({ cardId: "c2" });
    expect(member(room, "格羅")!.seatRequest).toBeUndefined();
    expect(room.s.cards.find((x) => x.cardId === "c2")!.claimedBy).toBe("m4");
    expect(room.store.events.filter((e) => e.seq > before).map((e) => e.type)).toEqual([
      "seat.reassigned",
      "member.kicked",
    ]);
    await room.join("b", "b");
    expect(room.frames("b", "vol", "seat_handed")[0].p.name).toBe("角色2");
    expect(member(room, "b")).toMatchObject({ id: "m5", cardId: null });
    expect(room.s.handedOver).toEqual({});
    // 房主能用既有入口把座位交回舊裝置
    await room.send("a", "member.reassign", { from: "m4", to: "m5" });
    expect(member(room, "b")!.cardId).toBe("c2");
  });

  it("舊頁籤自動重連帶舊 lastSeq：拿完整快照換成新身分，之後選角更新得到自己", async () => {
    const room = await room3();
    await room.send("c", "seat.request", { cardId: "c2" });
    const { requestId } = requests(room)[0];
    let view = applyFrame(null, room.frames("a", "welcome")[0] as any)!;
    const lastSeq = room.s.seq;
    await room.send("a", "seat.request.resolve", { memberId: "m4", requestId, accept: true });
    await room.join("b", "b", { lastSeq });
    const welcome = room.frames("b", "welcome").at(-1)!;
    expect(welcome.p.snapshot.me.memberId).toBe("m5");
    view = applyFrame(view, welcome as any)!;
    await room.send("a", "seat.release");
    await room.send("b", "seat.claim", { cardId: "c1" });
    view = applyFrame(view, room.frames("b", "ev", "seat.claimed").at(-1)! as any)!;
    expect(view.me.cardId).toBe("c1");
  });

  it("認領不公開：其他成員與執行端收不到，公開名單與快照不帶", async () => {
    const room = await room3();
    await room.join("e", "雷恩");
    await room.send("c", "seat.request", { cardId: "c2" });
    const { requestId } = requests(room)[0];
    for (const id of ["e", "x1"]) expect(JSON.stringify(room.out[id])).not.toContain(requestId);
    const peer = buildSnapshot(room.s, [], "m5", LIVE);
    expect(JSON.stringify(peer)).not.toContain(requestId);
    expect(peer.seatRequests).toBeUndefined();
    expect(JSON.stringify(buildSnapshot(room.s, [], "exec", LIVE))).not.toContain(requestId);
    expect(buildSnapshot(room.s, [], "m1", LIVE).seatRequests).toHaveLength(1);
    expect(buildSnapshot(room.s, [], "m4", LIVE).me!.seatRequest).toBe("c2");
  });

  it("非房主不能確認；requestId 不符被拒，延遲的確認批不到新認領", async () => {
    const room = await room3();
    await room.join("e", "雷恩");
    await room.send("c", "seat.request", { cardId: "c2" });
    const old = requests(room)[0].requestId;
    const answer = { memberId: "m4", requestId: old, accept: true };
    expect(await code(room, "e", "seat.request.resolve", answer)).toBe("E_FORBIDDEN");
    await room.send("a", "seat.request.resolve", { ...answer, accept: false });
    expect(room.frames("c", "vol", "seat_request").at(-2)!.p.refused).toBe("c2");
    await room.send("c", "seat.request", { cardId: "c3" });
    expect(await code(room, "a", "seat.request.resolve", answer)).toBe("E_STATE_CONFLICT");
    expect(member(room, "格羅")!.cardId).toBeNull();
    expect(member(room, "d")!.cardId).toBe("c3");
  });

  it("舊裝置回來：認領清掉、延遲的確認被拒、座位不動", async () => {
    const room = await room3();
    await room.send("c", "seat.request", { cardId: "c2" });
    const { requestId } = requests(room)[0];
    await room.join("b", "b");
    expect(requests(room)).toEqual([]);
    expect(room.frames("c", "vol", "seat_request").at(-1)!.p.cardId).toBeNull();
    const answer = { memberId: "m4", requestId, accept: true };
    expect(await code(room, "a", "seat.request.resolve", answer)).toBe("E_STATE_CONFLICT");
    expect(member(room, "b")!.cardId).toBe("c2");
  });

  it("回合中交接：名單與草稿跟著走；代管中交接：照接回處理", async () => {
    const room = await playing(["a", "b", "d"]);
    await room.send("b", "action.draft", { text: "我拔斧" });
    await room.send("a", "seat.delegate", { memberId: "m3", on: true });
    await room.disconnect("b");
    await room.disconnect("d");
    await room.join("c", "格羅");
    await room.join("e", "雷恩");
    await room.send("c", "seat.request", { cardId: "c2" });
    await room.send("e", "seat.request", { cardId: "c3" });
    for (const r of requests(room))
      await room.send("a", "seat.request.resolve", {
        memberId: r.memberId,
        requestId: r.requestId,
        accept: true,
      });
    expect(room.s.round!.roster.map((e) => e.memberId)).toContain("m4");
    expect(room.s.drafts.m4).toBe("我拔斧");
    expect(member(room, "雷恩")).toMatchObject({ cardId: "c3", delegated: false });
    expect(room.s.members.map((m) => m.id).sort()).toEqual(["m1", "m4", "m5"]);
  });

  it("送出驗證：有座位、佔位者在線、房主或出額度者的座位、重複認領、限頻", async () => {
    const room = await room3();
    await room.join("e", "雷恩");
    expect(await code(room, "a", "seat.request", { cardId: "c2" })).toBe("E_STATE_CONFLICT"); // 有座位
    expect(await code(room, "c", "seat.request", { cardId: "c1" })).toBe("E_STATE_CONFLICT"); // 房主兼出額度者、也在線
    expect(await code(room, "c", "seat.request", { cardId: "nope" })).toBe("E_STATE_CONFLICT");
    await room.send("c", "seat.request", { cardId: "c2" });
    expect(await code(room, "c", "seat.request", { cardId: "c3" })).toBe("E_STATE_CONFLICT");
    for (let i = 0; i < 2; i++) {
      const { requestId } = requests(room)[0];
      await room.send("a", "seat.request.resolve", { memberId: "m4", requestId, accept: false });
      await room.send("c", "seat.request", { cardId: "c2" });
    }
    const { requestId } = requests(room)[0];
    await room.send("a", "seat.request.resolve", { memberId: "m4", requestId, accept: false });
    expect(await code(room, "c", "seat.request", { cardId: "c2" })).toBe("E_RATE_LIMITED");
  });

  it("失效掃描：開始建卡、上傳卡檔、佔位者變房主都清掉", async () => {
    const room = await room3();
    await room.join("e", "雷恩");
    const cleared = (id: string, nick: string) => {
      expect(member(room, nick)!.seatRequest).toBeUndefined();
      expect(room.frames(id, "vol", "seat_request").at(-1)!.p.cardId).toBeNull();
    };
    await room.send("c", "seat.request", { cardId: "c2" });
    await room.send("c", "seat.create", { name: "新角", blurb: "" });
    cleared("c", "格羅");
    expect(await code(room, "c", "seat.request", { cardId: "c2" })).toBe("E_STATE_CONFLICT"); // 建卡還沒回

    await room.send("e", "seat.request", { cardId: "c2" });
    await room.run({
      kind: "http",
      route: "upload_reserve",
      deviceId: room.deviceId("e"),
      uploadKind: "card",
      length: 10,
    });
    cleared("e", "雷恩");
    expect(await code(room, "e", "seat.request", { cardId: "c3" })).toBe("E_STATE_CONFLICT"); // 上傳還沒派工

    await room.join("f", "卡爾");
    await room.send("f", "seat.request", { cardId: "c2" });
    await room.send("a", "host.transfer", { memberId: "m2" });
    cleared("f", "卡爾");
  });

  it("房主換人：新房主拿到清單、舊房主清空；離房清掉認領", async () => {
    const room = await room3();
    await room.join("e", "雷恩");
    await room.send("c", "seat.request", { cardId: "c2" });
    await room.send("a", "host.transfer", { memberId: "m5" });
    expect(requests(room)).toEqual([]);
    expect(room.frames("e", "vol", "seat_requests").at(-1)!.p.list).toHaveLength(1);
    await room.send("c", "member.leave");
    expect(room.frames("e", "vol", "seat_requests").at(-1)!.p.list).toEqual([]);
  });

  it("member.leave 刪列並放座位，同裝置回來算新成員；斷線、房主、出額度者不刪", async () => {
    const room = await room3();
    expect(member(room, "b")).toBeDefined(); // 斷線不刪列
    expect(await code(room, "a", "member.leave", {})).toBe("E_FORBIDDEN");
    const id = member(room, "格羅")!.id;
    await room.send("c", "member.leave");
    expect(member(room, "格羅")).toBeUndefined();
    expect(room.frames("a", "ev").some((f) => f.p.e.type === "member.left")).toBe(true);
    await room.join("c2", "格羅");
    expect(member(room, "格羅")!.id).not.toBe(id);
  });

  it("休眠醒來補的斷線不記時間；舊房間與舊快照缺欄位照常", async () => {
    const room = await playing();
    await room.run({ kind: "disconnect", conn: { id: "b", role: "member" }, stale: true });
    room.open.delete("b");
    expect(member(room, "b")!.offlineSince).toBeUndefined();
    await room.join("c", "格羅");
    await room.send("c", "seat.request", { cardId: "c2" });
    expect(requests(room)[0].offlineSince).toBeNull();

    const snap = buildSnapshot(room.s, [], "m1", LIVE) as any;
    delete snap.seatRequests;
    delete snap.me.seatRequest;
    const view = applyFrame(
      null as unknown as RoomView,
      {
        t: "welcome",
        p: { memberId: "m1", snapshot: snap },
      } as any,
    ) as RoomView;
    expect(view.seatRequests).toEqual([]);
    expect(view.me.seatRequest).toBeUndefined();
  });
});
