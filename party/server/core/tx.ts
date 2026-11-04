// 一次輸入的交易：讀舊狀態、寫時才複製、收集事件與要送的訊框。拋 Fail＝整筆作廢、只回錯誤。
import {
  TO_EXECUTOR,
  type CoreCtx,
  type CoreOutput,
  type ErrorCode,
  type Frame,
  type RoomState,
  type StoredEvent,
} from "./types";

export class Fail extends Error {
  constructor(
    readonly code: ErrorCode,
    readonly detail?: string,
    readonly retryAfterMs?: number,
  ) {
    super(code);
  }
  static throw(code: ErrorCode): never {
    throw new Fail(code);
  }
}
export function need(cond: unknown, code: ErrorCode, detail?: string): asserts cond {
  if (!cond) throw new Fail(code, detail);
}

// 名冊（含頭像）可能上 MiB，不跟著深拷貝；改名冊一律換新陣列
const clone = (s: RoomState): RoomState => ({
  ...structuredClone({ ...s, cards: [] }),
  cards: s.cards,
});

export class Tx {
  private copy: RoomState | null = null;
  readonly events: StoredEvent[] = [];
  readonly sends: CoreOutput["sends"] = [];
  readonly close: CoreOutput["close"] = [];
  http?: CoreOutput["http"];
  deleteRoom = false;

  constructor(
    private readonly orig: RoomState | null,
    readonly ctx: CoreCtx,
    readonly conn = "",
    readonly reqId?: string,
  ) {}
  get now() {
    return this.ctx.now;
  }
  get exists() {
    return (this.copy ?? this.orig) !== null;
  }
  get s(): RoomState {
    return (this.copy ?? this.orig)!;
  }
  /** 要改狀態時用這個：第一次取用才複製，沒改過的輸入（串流增量）不必落地 */
  get w(): RoomState {
    return (this.copy ??= clone(this.orig!));
  }
  get dirty() {
    return this.copy !== null;
  }
  init(state: RoomState) {
    this.copy = state;
  }

  send(to: string, t: string, p: unknown) {
    this.sends.push({ to, frame: { t, p } as Frame });
  }
  /** 已通過 hello 的所有連線（成員與執行端） */
  toAll(t: string, p: unknown) {
    for (const [id, c] of Object.entries(this.s.conns)) if (c.who !== null) this.send(id, t, p);
  }
  toMember(memberId: string | null, t: string, p: unknown) {
    for (const [id, c] of Object.entries(this.s.conns))
      if (memberId !== null && c.who === memberId) this.send(id, t, p);
  }
  toExec(t: string, p: unknown) {
    if (this.s.exec.online) this.send(TO_EXECUTOR, t, p);
  }
  /** 持久事件：配序號、記活動時間、廣播 `ev`（殼在落地成功後才真的送出）。
   *  與協定 §12.1 的差異：seq 由核心配（狀態本身記著最後序號），不是殼配；commit 失敗時殼丟掉新狀態，序號自然不前進 */
  emit(type: string, payload: Record<string, unknown> = {}) {
    const s = this.w;
    const ev: StoredEvent = { seq: ++s.seq, at: this.now, type, payload };
    s.lastActive = this.now;
    this.events.push(ev);
    this.toAll("ev", { seq: ev.seq, at: ev.at, e: { type, ...payload } });
  }
  ok(result?: unknown) {
    if (this.reqId !== undefined)
      this.send(this.conn, "ok", { id: this.reqId, ...(result !== undefined ? { result } : {}) });
  }
  err(
    conn: string,
    code: ErrorCode,
    extra: { id?: string; retryAfterMs?: number; detail?: string } = {},
  ) {
    this.send(conn, "err", { code, ...extra });
  }
  shut(conn: string, code: number, err?: ErrorCode) {
    if (err) this.err(conn, err);
    this.close.push({ conn, code });
    if (this.exists && this.s.conns[conn]) delete this.w.conns[conn];
  }
}
