// 核心與平台殼之間的接線（協定 §12.1）：每房一條序列化佇列 → reduce → 一筆交易落地 → 成功後才送訊框。
// Workers 殼與 Node 殼（包 7）都只實作下面四個介面，再交給 RoomRunner。
import { reduce } from "./reduce";
import {
  TO_EXECUTOR,
  type ConnInfo,
  type CoreInput,
  type CoreOutput,
  type Frame,
  type Live,
  type RoomState,
  type StoredEvent,
} from "./types";

export interface RoomStore {
  load(): Promise<{ state: RoomState | null }>; // 協定 §12.1 另回 tailSeq；這裡序號在狀態裡，不必另回
  commit(state: RoomState, events: StoredEvent[]): Promise<void>;
  eventsAfter(seq: number, limit: number): Promise<StoredEvent[]>;
  eventsBefore(seq: number, limit: number): Promise<StoredEvent[]>;
  deleteAll(): Promise<void>;
}
export interface Connections {
  send(conn: string, text: string): void;
  close(conn: string, code: number): void;
  list(): ConnInfo[];
}
export interface Alarm {
  set(at: number | null): Promise<void>;
}
/** v1：轉成 WebSocket 送桌面版（預設）；v2：程序內執行端 */
export interface ExecutorPort {
  deliver(frame: Frame): void;
}

export class RoomRunner {
  private state: RoomState | null | undefined; // undefined＝還沒從 storage 載入（DO 剛喚醒）
  private readonly live: Live = {
    stream: null,
    seed: null,
    buckets: {},
    uploads: {},
    cardReqs: {},
  };
  private queue: Promise<unknown> = Promise.resolve();

  constructor(
    private readonly store: RoomStore,
    private readonly conns: Connections,
    private readonly alarm: Alarm,
    private readonly opts: {
      now?: () => number;
      rng?: (n: number) => Uint8Array;
      executor?: ExecutorPort;
    } = {},
  ) {}

  /** DO 的單執行緒保證在 await（WebCrypto、storage）之間不成立，所以每房自己排隊 */
  run(input: CoreInput): Promise<CoreOutput | null> {
    const next = this.queue.then(() => this.step(input));
    this.queue = next.catch(() => {});
    return next;
  }

  private async step(input: CoreInput): Promise<CoreOutput | null> {
    if (this.state === undefined) {
      this.state = (await this.store.load()).state;
      // 休眠期間消失的連線（部署重啟等）補一次離線
      const alive = new Set(this.conns.list().map((c) => c.id));
      for (const [id, c] of Object.entries(this.state?.conns ?? {}))
        if (!alive.has(id)) await this.apply({ kind: "disconnect", conn: { id, role: c.role } });
    }
    return this.apply(input);
  }

  private async apply(input: CoreInput): Promise<CoreOutput | null> {
    const prev = this.state ?? null;
    const conn = "conn" in input ? input.conn.id : null;
    const internal = () =>
      conn && this.conns.send(conn, JSON.stringify({ t: "err", p: { code: "E_INTERNAL" } }));
    let out: CoreOutput;
    try {
      out = await reduce(prev, input, {
        now: this.opts.now?.() ?? Date.now(),
        rng: this.opts.rng ?? ((n) => crypto.getRandomValues(new Uint8Array(n))),
        live: this.live,
        store: this.store,
      });
      if (out.deleteRoom) await this.store.deleteAll();
      else if (out.state && (out.state !== prev || out.events.length > 0))
        await this.store.commit(out.state, out.events);
    } catch {
      internal(); // 落地失敗：不廣播、狀態不前進（記憶體仍是舊狀態，下一則輸入照舊狀態算）
      return null;
    }
    this.state = out.state;
    const executor = this.opts.executor ?? {
      deliver: (f: Frame) =>
        this.state?.execConn && this.conns.send(this.state.execConn, JSON.stringify(f)),
    };
    for (const { to, frame } of out.sends) {
      if (to === TO_EXECUTOR) executor.deliver(frame);
      else this.conns.send(to, JSON.stringify(frame));
    }
    for (const c of out.close) this.conns.close(c.conn, c.code);
    await this.alarm.set(out.alarmAt).catch(() => {});
    return out;
  }
}
