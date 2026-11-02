// Node 殼的房間表：每房一個 RoomRunner（它就是每房序列化佇列，協定 §14-1），實作 Connections／Alarm。
// 對應 DO 的 Hibernation：沒有連線也沒有進行中工作的房間從記憶體卸載，下次請求或鬧鐘再從 SQLite 載入。
import { randomUUID } from "node:crypto";
import type { WebSocket } from "ws";
import { RoomRunner, type Connections } from "../core/runner";
import type { ConnInfo, ConnRole, CoreInput } from "../core/types";
import type { RoomDb } from "./sqlite-store";

const PING = '{"t":"ping"}';
const PONG = '{"t":"pong"}';
const MEMBER_STALE_MS = 45_000;
const MAX_TIMER_MS = 2 ** 31 - 1; // setTimeout 上限約 24.8 天；閒置期限可到 365 天，超過就分段排

interface Sock {
  ws: WebSocket;
  info: ConnInfo;
  lastPing: number;
  gone: boolean;
}

export interface RoomsOpts {
  now?: () => number;
  setTimer?: (fn: () => void, ms: number) => unknown;
  clearTimer?: (t: unknown) => void;
}

export class Room {
  work = 0; // 取得後到釋放前的工作數（含第一次 await 之前），卸載只看這個
  readonly socks = new Map<string, Sock>();
  readonly runner: RoomRunner;

  constructor(private readonly rooms: Rooms, readonly code: string) {
    const conns: Connections = {
      send: (id, text) => tryIt(() => this.socks.get(id)?.ws.send(text)),
      close: (id, code) => tryIt(() => this.socks.get(id)?.ws.close(code)),
      list: () => [...this.socks.values()].map((s) => s.info),
    };
    const alarm = { set: async (at: number | null) => rooms.setAlarm(code, at) };
    this.runner = new RoomRunner(rooms.db.store(code), conns, alarm, { now: rooms.now });
  }

  /** 45 秒沒心跳的成員關掉並排進離線；同步排進佇列，不會和後面的訊框調換順序 */
  sweep(): Promise<unknown>[] {
    const now = this.rooms.now();
    return [...this.socks.values()].filter((s) => s.info.role === "member" && now - s.lastPing > MEMBER_STALE_MS).map((s) => this.drop(s, 1001));
  }

  /** 斷線收尾：error、close、掃描共用，可重複呼叫，只送一次 disconnect */
  drop(s: Sock, code?: number): Promise<unknown> {
    if (s.gone) return Promise.resolve();
    s.gone = true;
    this.socks.delete(s.info.id);
    if (code) tryIt(() => s.ws.close(code));
    return this.runner.run({ kind: "disconnect", conn: s.info });
  }

  async run(input: CoreInput) {
    const swept = this.sweep();
    const out = await this.runner.run(input);
    await Promise.all(swept);
    return out;
  }
}

export class Rooms {
  private readonly map = new Map<string, Room>();
  private readonly timers = new Map<string, unknown>();
  readonly now: () => number;

  constructor(readonly db: RoomDb, private readonly opts: RoomsOpts = {}) {
    this.now = opts.now ?? Date.now;
  }

  peek(code: string) {
    return this.map.get(code);
  }
  get size() {
    return this.map.size;
  }

  /** 取得房間並在第一次 await 前計入工作；fn 結束才釋放，釋放時同步核對身分再卸載 */
  async with<T>(code: string, fn: (room: Room) => Promise<T>): Promise<T> {
    let room = this.map.get(code);
    if (!room) this.map.set(code, (room = new Room(this, code)));
    room.work++;
    try {
      return await fn(room);
    } finally {
      room.work--;
      if (room.work === 0 && room.socks.size === 0 && this.map.get(code) === room) this.map.delete(code);
    }
  }

  attach(code: string, ws: WebSocket, role: ConnRole) {
    return this.with(code, async (room) => {
      const s: Sock = { ws, info: { id: randomUUID(), role }, lastPing: this.now(), gone: false };
      room.socks.set(s.info.id, s);
      // 房間有連線時不會卸載，所以這些回呼拿到的 room 一定還是 Map 裡那個
      const end = () => guard(this.with(code, () => room.drop(s)));
      ws.on("error", end);
      ws.on("close", end);
      ws.on("message", (data, isBinary) => {
        const raw = isBinary ? "" : data.toString();
        s.lastPing = this.now(); // 自己剛送訊框的連線不算死（DO 的 sweep 也排除發送者）
        if (raw === PING) return tryIt(() => ws.send(PONG)); // 對齊 DO 的自動回應：不進核心
        if (!s.gone) guard(this.with(code, () => room.run({ kind: "frame", conn: s.info, raw })));
      });
      await room.run({ kind: "connect", conn: s.info });
    });
  }

  setAlarm(code: string, at: number | null) {
    this.db.setAlarm(code, at);
    this.arm(code, at);
  }

  arm(code: string, at: number | null) {
    const clear = this.opts.clearTimer ?? ((t) => clearTimeout(t as NodeJS.Timeout));
    if (this.timers.has(code)) clear(this.timers.get(code));
    this.timers.delete(code);
    if (at === null) return;
    const set = this.opts.setTimer ?? ((fn, ms) => setTimeout(fn, ms));
    const t = set(() => {
      this.timers.delete(code);
      if (this.now() < at) return this.arm(code, at);
      guard(this.with(code, (room) => room.run({ kind: "alarm" })));
    }, Math.min(MAX_TIMER_MS, Math.max(0, at - this.now())));
    this.timers.set(code, t);
  }

  /** 程序啟動：把每個存活房間的鬧鐘排回去 */
  restore() {
    for (const { code, at } of this.db.pendingAlarms()) this.arm(code, at);
  }

  /** 死 TCP 不一定有 close 事件：定期掃一次已載入房間的成員心跳 */
  sweepAll() {
    for (const code of [...this.map.keys()]) guard(this.with(code, (room) => Promise.all(room.sweep())));
  }

  /** 關機：全部連線 1001（客戶端照退避重連），等各房工作跑完 */
  async shutdown(waitMs = 5000) {
    const clear = this.opts.clearTimer ?? ((t) => clearTimeout(t as NodeJS.Timeout));
    for (const t of this.timers.values()) clear(t);
    this.timers.clear();
    for (const room of this.map.values()) for (const s of room.socks.values()) tryIt(() => s.ws.close(1001));
    const until = Date.now() + waitMs;
    while (this.map.size && Date.now() < until) await new Promise((r) => setTimeout(r, 50));
  }
}

export function guard(p: Promise<unknown>) {
  p.catch((e) => console.error("[party] room task failed:", e));
}

function tryIt(fn: () => void) {
  try {
    fn();
  } catch {
    // 對方已斷線：送不出去就算了，關閉事件會另外進來
  }
}
