// Cloudflare Durable Object 殼（協定 §12.1）：一房一個 DO，SQLite storage＋WebSocket Hibernation＋單一 alarm。
// 規則全在 core/；這裡只把平台介面接到 RoomRunner。
import { RoomRunner, type Connections, type RoomStore } from "../core/runner";
import type { ConnInfo, ConnRole, RoomState, StoredEvent } from "../core/types";

export interface Env {
  ROOM: DurableObjectNamespace;
  CREATE_LIMITER?: RateLimit;
  ROOM_IDLE_DAYS?: string;
}
type Attachment = ConnInfo & { at: number };
const MEMBER_STALE_MS = 45_000;
const json = (body: unknown, status: number) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

/** 與協定 §9.1 的差異：不拆 room／members／cards／drafts／codes／events 六張表，改三張——
 *  room 單列放整份狀態 JSON（含成員、草稿、配對碼，不含名冊）、cards 單列放名冊（含頭像，只在換名冊時重寫）、
 *  events 一事件一列。核心每次輸出整份狀態，拆表只多映射。DO SQLite 單列上限 2 MB，所以名冊整份限 1 MiB（core/executor.ts）。 */
export class SqlRoomStore implements RoomStore {
  private writtenCards: unknown = undefined;
  constructor(private readonly storage: DurableObjectStorage) {
    storage.sql.exec(
      "CREATE TABLE IF NOT EXISTS room (id INTEGER PRIMARY KEY, state TEXT NOT NULL);" +
        "CREATE TABLE IF NOT EXISTS cards (id INTEGER PRIMARY KEY, cards TEXT NOT NULL);" +
        "CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY, at INTEGER NOT NULL, type TEXT NOT NULL, payload TEXT NOT NULL);",
    );
  }
  async load() {
    const row = this.storage.sql.exec<{ state: string }>("SELECT state FROM room WHERE id = 1").toArray()[0];
    if (!row) return { state: null };
    const state = JSON.parse(row.state) as RoomState;
    state.cards = JSON.parse(this.storage.sql.exec<{ cards: string }>("SELECT cards FROM cards WHERE id = 1").one().cards);
    this.writtenCards = state.cards;
    return { state };
  }
  async commit(state: RoomState, events: StoredEvent[]) {
    this.storage.transactionSync(() => {
      const sql = this.storage.sql;
      sql.exec("INSERT OR REPLACE INTO room (id, state) VALUES (1, ?)", JSON.stringify({ ...state, cards: undefined }));
      if (state.cards !== this.writtenCards) sql.exec("INSERT OR REPLACE INTO cards (id, cards) VALUES (1, ?)", JSON.stringify(state.cards));
      for (const e of events) sql.exec("INSERT INTO events (seq, at, type, payload) VALUES (?, ?, ?, ?)", e.seq, e.at, e.type, JSON.stringify(e.payload));
    });
    this.writtenCards = state.cards;
  }
  private rows(query: string, ...args: number[]): StoredEvent[] {
    return this.storage.sql.exec<{ seq: number; at: number; type: string; payload: string }>(query, ...args).toArray().map((r) => ({ seq: r.seq, at: r.at, type: r.type, payload: JSON.parse(r.payload) }));
  }
  async eventsAfter(seq: number, limit: number) {
    return this.rows("SELECT * FROM events WHERE seq > ? ORDER BY seq LIMIT ?", seq, limit);
  }
  async eventsBefore(seq: number, limit: number) {
    return this.rows("SELECT * FROM events WHERE seq < ? ORDER BY seq DESC LIMIT ?", seq, limit).reverse();
  }
  async deleteAll() {
    await this.storage.deleteAll();
    this.writtenCards = undefined;
  }
}

export class RoomDO implements DurableObject {
  private readonly runner: RoomRunner;

  constructor(private readonly ctx: DurableObjectState, private readonly env: Env) {
    // 成員心跳走自動回應、不喚醒 DO（§4.2）；執行端的 hb 是一般訊息
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('{"t":"ping"}', '{"t":"pong"}'));
    const find = (id: string) => ctx.getWebSockets().find((ws) => this.info(ws)?.id === id);
    const conns: Connections = {
      send: (id, text) => tryIt(() => find(id)?.send(text)),
      close: (id, code) => tryIt(() => find(id)?.close(code)),
      list: () => ctx.getWebSockets().flatMap((ws) => this.info(ws) ?? []),
    };
    const alarm = { set: (at: number | null) => (at === null ? ctx.storage.deleteAlarm() : ctx.storage.setAlarm(at)) };
    this.runner = new RoomRunner(new SqlRoomStore(ctx.storage), conns, alarm);
  }

  private info(ws: WebSocket): Attachment | null {
    return ws.deserializeAttachment() as Attachment | null;
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    if (url.pathname === "/create") {
      const idleDays = Math.min(365, Math.max(1, Math.trunc(Number(this.env.ROOM_IDLE_DAYS)) || 30));
      const body = await req.json().catch(() => null);
      const out = await this.runner.run({ kind: "http", route: "create", body, code: req.headers.get("x-room-code") ?? "", origin: req.headers.get("x-origin") ?? "", idleDays });
      return json(out?.http?.body ?? { code: "E_INTERNAL" }, out?.http?.status ?? 500);
    }
    if (url.pathname.endsWith("/export")) {
      const deviceId = (req.headers.get("Authorization") ?? "").replace(/^Bearer\s+/i, "");
      const out = await this.runner.run({ kind: "http", route: "export", deviceId });
      return json(out?.http?.body ?? { code: "E_INTERNAL" }, out?.http?.status ?? 500);
    }
    if (req.headers.get("Upgrade")?.toLowerCase() !== "websocket") return new Response("Expected WebSocket", { status: 426 });
    const pair = new WebSocketPair();
    await this.accept(pair[1], url.searchParams.get("role") === "executor" ? "executor" : "member");
    return new Response(null, { status: 101, webSocket: pair[0] });
  }

  async accept(ws: WebSocket, role: ConnRole) {
    const conn: Attachment = { id: crypto.randomUUID(), role, at: Date.now() };
    this.ctx.acceptWebSocket(ws);
    ws.serializeAttachment(conn);
    await this.runner.run({ kind: "connect", conn: { id: conn.id, role } });
  }

  /** 喚醒時清掉 45 秒沒有自動回應的成員連線（成員在線只影響顯示） */
  private async sweep(except?: WebSocket) {
    const now = Date.now();
    for (const ws of this.ctx.getWebSockets()) {
      const c = this.info(ws);
      if (!c || ws === except || c.role !== "member" || now - (this.ctx.getWebSocketAutoResponseTimestamp(ws)?.getTime() ?? c.at) <= MEMBER_STALE_MS) continue;
      tryIt(() => ws.close(1001));
      await this.runner.run({ kind: "disconnect", conn: { id: c.id, role: c.role } });
    }
  }

  async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer) {
    const c = this.info(ws);
    if (!c) return;
    await this.sweep(ws);
    await this.runner.run({ kind: "frame", conn: { id: c.id, role: c.role }, raw: typeof message === "string" ? message : "" });
  }

  async webSocketClose(ws: WebSocket) {
    const c = this.info(ws);
    if (c) await this.runner.run({ kind: "disconnect", conn: { id: c.id, role: c.role } });
  }

  async webSocketError(ws: WebSocket) {
    await this.webSocketClose(ws);
  }

  async alarm() {
    await this.sweep();
    await this.runner.run({ kind: "alarm" });
  }
}

function tryIt(fn: () => void) {
  try {
    fn();
  } catch {
    // 對方已斷線：送不出去就算了，關閉事件會另外進來
  }
}
