// DO 殼的關鍵路徑：薄的假 DurableObjectState（SQLite 用 node:sqlite）＋假 WebSocket。
// 驗：ev 一定在 events 表落地後才送、喚醒（新實例）後從 storage 接著跑、成員心跳逾時被清掉、匯出走 HTTP。
// @ts-expect-error -- 殼測試在 Node 跑；party 的型別只帶 workers-types，不另引 @types/node
import { DatabaseSync } from "node:sqlite";
import { describe, expect, it } from "vitest";
import { RoomDO } from "./room-do";

(globalThis as Record<string, unknown>).WebSocketRequestResponsePair ??= class {
  constructor(
    readonly request: string,
    readonly response: string,
  ) {}
};

class FakeSocket {
  sent: any[] = [];
  closedWith: number | null = null;
  autoResponseAt: Date | null = null;
  private att: unknown = null;
  constructor(private readonly onSend: (frame: any) => void = () => {}) {}
  send(text: string) {
    const frame = JSON.parse(text);
    this.onSend(frame);
    this.sent.push(frame);
  }
  close(code: number) {
    this.closedWith = code;
  }
  serializeAttachment(v: unknown) {
    this.att = structuredClone(v);
  }
  deserializeAttachment() {
    return this.att;
  }
}

function fakeCtx() {
  const db = new DatabaseSync(":memory:");
  const sockets: FakeSocket[] = [];
  const cursor = (rows: unknown[]) => ({ toArray: () => rows, one: () => rows[0] });
  const sql = {
    exec(query: string, ...args: any[]) {
      if (/^\s*SELECT/i.test(query)) return cursor(db.prepare(query).all(...args));
      if (args.length) db.prepare(query).run(...args);
      else db.exec(query);
      return cursor([]);
    },
  };
  const storage = {
    sql,
    alarm: null as number | null,
    transactionSync<T>(fn: () => T): T {
      db.exec("BEGIN");
      try {
        const r = fn();
        db.exec("COMMIT");
        return r;
      } catch (e) {
        db.exec("ROLLBACK");
        throw e;
      }
    },
    async setAlarm(at: number) {
      storage.alarm = at;
    },
    async deleteAlarm() {
      storage.alarm = null;
    },
    async deleteAll() {
      db.exec("DELETE FROM room; DELETE FROM cards; DELETE FROM events;");
    },
  };
  const ctx = {
    storage,
    acceptWebSocket: (ws: FakeSocket) => void sockets.push(ws),
    getWebSockets: () => sockets.filter((ws) => ws.closedWith === null),
    setWebSocketAutoResponse: () => {},
    getWebSocketAutoResponseTimestamp: (ws: FakeSocket) => ws.autoResponseAt,
  };
  return { ctx, db };
}

const make = (ctx: unknown) =>
  new RoomDO(ctx as DurableObjectState, { ROOM: null as unknown as DurableObjectNamespace });
const hello = (p: Record<string, unknown>) =>
  JSON.stringify({ t: "hello", id: "h", p: { protocol: 1, lastSeq: 0, client: {}, ...p } });

describe("RoomDO", () => {
  it("建房、執行端與成員連線；每則 ev 送出當下 events 表已有那一筆；新實例從 storage 接著跑", async () => {
    const { ctx, db } = fakeCtx();
    const room = make(ctx);
    const res = await room.fetch(
      new Request("https://room/create", {
        method: "POST",
        headers: { "x-room-code": "ABCD2345", "x-origin": "https://t" },
        body: JSON.stringify({ protocol: 1, label: "電腦", client: {} }),
      }),
    );
    expect(res.status).toBe(201);
    const { executorToken, joinSecret, inviteUrl } = (await res.json()) as Record<string, string>;
    expect(inviteUrl).toBe(`https://t/r/ABCD2345#k=${joinSecret}`);

    const landed = (frame: any) => {
      if (frame.t === "ev")
        expect(db.prepare("SELECT seq FROM events WHERE seq = ?").all(frame.p.seq)).toHaveLength(1);
    };
    const exec = new FakeSocket(landed);
    await room.accept(exec as unknown as WebSocket, "executor");
    await room.webSocketMessage(
      exec as unknown as WebSocket,
      hello({ role: "executor", roomCode: "abcd2345", executorToken, inflight: null }),
    );
    expect(exec.sent[0].t).toBe("welcome");
    const card = {
      cardId: "c1",
      name: "甲",
      color: null,
      avatar: "data:image/png;base64,AAAA",
      blurb: "",
      playable: true,
      hidden: false,
    };
    await room.webSocketMessage(
      exec as unknown as WebSocket,
      JSON.stringify({ t: "roster.update", p: { cards: [card] } }),
    );

    const member = new FakeSocket(landed);
    await room.accept(member as unknown as WebSocket, "member");
    await room.webSocketMessage(
      member as unknown as WebSocket,
      hello({ role: "member", deviceId: "d".repeat(22), joinSecret, nickname: "阿明" }),
    );
    expect(member.sent[0].p.snapshot.cards[0].avatar).toBe(card.avatar);
    expect(exec.sent.filter((f) => f.t === "ev").map((f) => f.p.e.type)).toEqual([
      "roster.updated",
      "member.joined",
    ]);
    expect(ctx.storage.alarm).not.toBeNull();

    // 喚醒：同一份 storage、新的 DO 實例；名冊從 cards 表讀回、序號接著配
    const woken = make(ctx);
    await woken.webSocketMessage(
      member as unknown as WebSocket,
      JSON.stringify({ t: "seat.claim", p: { cardId: "c1" } }),
    );
    const claimed = exec.sent.find((f) => f.p?.e?.type === "seat.claimed");
    expect(claimed.p.seq).toBe(4);
    const exportRes = await woken.fetch(
      new Request("https://room/api/rooms/ABCD2345/export", {
        headers: { Authorization: `Bearer ${"d".repeat(22)}` },
      }),
    );
    expect(exportRes.status).toBe(403); // 沒配對 grantHost，還沒有房主
  });

  it("上傳走 DO 的 fetch：先占名額再串流讀本體、派工後執行端帶憑證取檔一次", async () => {
    const { ctx } = fakeCtx();
    const room = make(ctx);
    const res = await room.fetch(
      new Request("https://room/create", {
        method: "POST",
        headers: { "x-room-code": "ABCD2345", "x-origin": "https://t" },
        body: JSON.stringify({ protocol: 1, label: "電腦", client: {} }),
      }),
    );
    const { executorToken, joinSecret } = (await res.json()) as Record<string, string>;
    const exec = new FakeSocket();
    await room.accept(exec as unknown as WebSocket, "executor");
    await room.webSocketMessage(
      exec as unknown as WebSocket,
      hello({ role: "executor", roomCode: "ABCD2345", executorToken, inflight: null }),
    );
    const member = new FakeSocket();
    await room.accept(member as unknown as WebSocket, "member");
    await room.webSocketMessage(
      member as unknown as WebSocket,
      hello({ role: "member", deviceId: "d".repeat(22), joinSecret, nickname: "阿明" }),
    );
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3]);
    const headers = {
      Authorization: `Bearer ${"d".repeat(22)}`,
      "Content-Length": String(png.length),
      Origin: "https://room",
    };
    const up = await room.fetch(
      new Request("https://room/api/rooms/ABCD2345/cards?kind=card", {
        method: "POST",
        headers,
        body: png,
      }),
    );
    expect(up.status).toBe(202);
    const job = exec.sent.find((f) => f.t === "job.card_import");
    const get = () =>
      room.fetch(
        new Request(`https://room/api/rooms/ABCD2345/cards/${job.p.uploadId}`, {
          headers: { Authorization: `Bearer ${executorToken}` },
        }),
      );
    expect(new Uint8Array(await (await get()).arrayBuffer())).toEqual(png);
    expect((await get()).status).toBe(404);
  });

  it("喚醒時 45 秒沒有自動回應的成員連線被關掉並廣播離線", async () => {
    const { ctx } = fakeCtx();
    const room = make(ctx);
    const res = await room.fetch(
      new Request("https://room/create", {
        method: "POST",
        headers: { "x-room-code": "ABCD2345", "x-origin": "https://t" },
        body: JSON.stringify({ protocol: 1, label: "電腦", client: {} }),
      }),
    );
    const { joinSecret } = (await res.json()) as Record<string, string>;
    const [a, b] = [new FakeSocket(), new FakeSocket()];
    for (const [ws, id] of [
      [a, "a"],
      [b, "b"],
    ] as const) {
      await room.accept(ws as unknown as WebSocket, "member");
      await room.webSocketMessage(
        ws as unknown as WebSocket,
        hello({ role: "member", deviceId: id.repeat(22), joinSecret, nickname: id }),
      );
    }
    a.autoResponseAt = new Date(Date.now() - 46_000);
    b.autoResponseAt = new Date();
    await room.alarm();
    expect(a.closedWith).toBe(1001);
    expect(b.sent.findLast((f) => f.p?.type === "presence")?.p).toEqual({
      type: "presence",
      memberId: "m1",
      connected: false,
    });
  });
});
