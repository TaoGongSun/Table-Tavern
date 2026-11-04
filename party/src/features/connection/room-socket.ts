// 成員連線（協定 §4）：第一則 hello（帶 lastSeq）、每 15 秒心跳、請求／回覆配對、斷線退避重連。
// 會讓伺服器關連線而且重連也沒用的錯誤（被踢、被取代、連結失效、房間沒了）記成 fatal，之後不再重連。
// 手機熄螢幕或切 App 後 onclose 常收不到：記最後收到訊框的時間，心跳那一拍或畫面回到前景時太久沒收到就自己斷開重連。
import type { ErrorCode, Frame } from "../../../server/core/types";

export type Reply = { ok: true; result?: any } | { ok: false; code: ErrorCode | "E_OFFLINE"; retryAfterMs?: number; detail?: string };
export interface SocketLike {
  readyState: number;
  send(text: string): void;
  close(code?: number): void;
  // 參數型別放寬成 any，瀏覽器的 WebSocket 才指派得進來
  onopen: ((ev: any) => void) | null;
  onmessage: ((ev: any) => void) | null;
  onclose: ((ev: any) => void) | null;
}

// 伺服器 45 秒沒心跳就關成員連線；遠端控制等執行端最多 30 秒，請求逾時要比它長
const TIMING = { staleMs: 40_000, requestMs: 35_000, now: () => Date.now() };

export interface SocketOptions {
  url: string;
  open: (url: string) => SocketLike;
  hello: (lastSeq: number) => Record<string, unknown>;
  lastSeq: () => number;
  onFrame: (frame: Frame) => void;
  onStatus: (status: "connecting" | "online" | "offline", fatal: ErrorCode | null) => void;
  /** 沒有請求在等的錯誤（含 hello 那一刻的錯誤，例：配對碼不對） */
  onNotice: (code: ErrorCode, retryAfterMs?: number) => void;
  retryDelays?: number[];
  timing?: Partial<typeof TIMING>;
}

const FATAL = new Set<string>(["E_AUTH_FAILED", "E_ROOM_NOT_FOUND", "E_BANNED", "E_KICKED", "E_SESSION_REPLACED", "E_VERSION", "E_BAD_FRAME"]);
// 只在 hello 被拒時才致命（別的指令回這兩個碼只是那一則不成）：重連也一樣被拒，不如停下來說原因
const HELLO_FATAL = new Set<string>(["E_ROOM_FULL", "E_NAME_INVALID"]);
const NO_RETRY_CLOSE = new Set([1000, 4400, 4403, 4404, 4409, 4410, 4426]);
const CLOSE_CODE: Record<number, ErrorCode> = { 4404: "E_ROOM_NOT_FOUND", 4409: "E_SESSION_REPLACED", 4410: "E_ROOM_CLOSED", 4426: "E_VERSION" };
const DELAYS = [1000, 2000, 5000, 10000, 30000];

export class RoomSocket {
  private ws: SocketLike | null = null;
  private n = 0;
  private attempt = 0;
  private pending = new Map<string, (r: Reply) => void>();
  private beat: ReturnType<typeof setInterval> | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private fatal: ErrorCode | null = null;
  private stopped = false;
  private fresh = false; // 下一次 hello 帶 lastSeq 0＝要整份快照
  private ready = false; // 收到 welcome 之後才能送指令（hello 前送別的，伺服器回 E_AUTH_REQUIRED 並關連線）
  private helloId = "";
  private lastRx = 0;
  private readonly time: typeof TIMING;
  private readonly onVisible = () => globalThis.document?.visibilityState === "visible" && this.checkAlive();

  constructor(private readonly o: SocketOptions) {
    this.time = { ...TIMING, ...o.timing };
    globalThis.document?.addEventListener("visibilitychange", this.onVisible);
  }

  connect() {
    const ws = (this.ws = this.o.open(this.o.url));
    this.ready = false;
    this.o.onStatus("connecting", null);
    ws.onopen = () => {
      this.helloId = this.nextId();
      ws.send(JSON.stringify({ t: "hello", id: this.helloId, p: this.o.hello(this.fresh ? 0 : this.o.lastSeq()) }));
    };
    ws.onmessage = (ev) => this.receive(String(ev.data));
    ws.onclose = (ev) => this.closed(ws, ev.code);
  }

  /** 送請求並等回覆；斷線中或逾時回 E_OFFLINE（客戶端自己的碼，不是協定碼） */
  request(t: string, p: unknown = {}, waitMs = this.time.requestMs): Promise<Reply> {
    if (!this.ready || this.ws?.readyState !== 1) return Promise.resolve({ ok: false, code: "E_OFFLINE" });
    const id = this.nextId();
    this.ws.send(JSON.stringify({ t, id, p }));
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      setTimeout(() => this.pending.delete(id) && resolve({ ok: false, code: "E_OFFLINE" }), waitMs);
    });
  }

  /** 太久沒收到任何訊框（pong 也算）＝連線其實死了：不等 onclose，直接當斷線走重連（補事件路徑） */
  checkAlive() {
    const ws = this.ws;
    if (!ws || !this.ready || ws.readyState !== 1 || this.time.now() - this.lastRx <= this.time.staleMs) return;
    Object.assign(ws, { onclose: null, onmessage: null });
    ws.close(4000);
    this.closed(ws, 4000);
  }

  /** E_STATE_CONFLICT 的處理：重抓快照、以畫面為準（§8） */
  resync() {
    this.fresh = true;
    this.ws?.close(4000);
  }

  stop() {
    this.stopped = true;
    globalThis.document?.removeEventListener("visibilitychange", this.onVisible);
    this.ws?.close(1000);
  }

  private nextId = () => `c${++this.n}`;

  private receive(text: string) {
    const f = JSON.parse(text) as Frame;
    this.lastRx = this.time.now();
    if (f.t === "welcome") {
      this.ready = true;
      this.attempt = 0;
      this.fresh = false;
      this.beat ??= setInterval(() => {
        this.checkAlive();
        if (this.ws?.readyState === 1) this.ws.send('{"t":"ping"}');
      }, 15_000);
      this.o.onStatus("online", null);
    }
    const done = (f.t === "ok" || f.t === "err") && f.p.id ? this.pending.get(f.p.id) : undefined;
    if (done) {
      this.pending.delete(f.p.id);
      done(f.t === "ok" ? { ok: true, result: f.p.result } : { ok: false, code: f.p.code, retryAfterMs: f.p.retryAfterMs, detail: f.p.detail });
    } else if (f.t === "err") {
      // hello 的錯誤帶 hello 的 id、伺服器主動的錯誤不帶 id：都沒有請求在等，交給畫面顯示
      const helloRejected = !this.ready && f.p.id === this.helloId;
      if (FATAL.has(f.p.code) || (helloRejected && HELLO_FATAL.has(f.p.code))) this.fatal = f.p.code;
      if (this.fatal && helloRejected) this.ws?.close(1000); // 伺服器 10 秒後才斷，自己先停
      else this.o.onNotice(f.p.code, f.p.retryAfterMs);
    }
    this.o.onFrame(f);
  }

  private closed(ws: SocketLike, code: number) {
    if (ws !== this.ws) return;
    this.ready = false;
    for (const resolve of this.pending.values()) resolve({ ok: false, code: "E_OFFLINE" });
    this.pending.clear();
    if (this.beat) clearInterval(this.beat);
    this.beat = null;
    this.fatal ??= CLOSE_CODE[code] ?? null;
    const end = this.stopped || this.fatal !== null || NO_RETRY_CLOSE.has(code);
    this.o.onStatus("offline", end ? this.fatal : null);
    if (end) return;
    const delays = this.o.retryDelays ?? DELAYS;
    const wait = this.fresh ? 0 : delays[Math.min(this.attempt++, delays.length - 1)];
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => !this.stopped && this.connect(), wait);
  }
}
