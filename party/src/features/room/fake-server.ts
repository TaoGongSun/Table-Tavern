// 測試用：把伺服器核心的 room-test-kit（記憶體 storage＋序列化佇列）接成手機端看得到的 WebSocket。
// 每次伺服器處理完一則輸入就把各連線新收到的訊框依序交出去；伺服器關掉的連線觸發 onclose（帶關閉碼）。
import type { Room } from "../../../server/core/room-test-kit";
import type { SocketLike } from "../connection/room-socket";

export class FakeSocket implements SocketLike {
  readyState = 0;
  onopen: SocketLike["onopen"] = null;
  onmessage: SocketLike["onmessage"] = null;
  onclose: SocketLike["onclose"] = null;
  /** 暫停送達：模擬手機暫時收不到（訊框留著，放開後依序補上） */
  held = false;
  readonly sent: any[] = [];
  private cursor = 0;

  constructor(
    private readonly hub: Hub,
    readonly id: string,
  ) {
    void hub.room.connect(id).then(() => {
      this.readyState = 1;
      this.onopen?.({});
    });
  }
  send(text: string) {
    this.sent.push(JSON.parse(text));
    void this.hub.room
      .run({ kind: "frame", conn: { id: this.id, role: "member" }, raw: text })
      .then(() => this.hub.flush());
  }
  close(code = 1000) {
    if (this.readyState === 3) return;
    this.readyState = 3;
    void this.hub.room.disconnect(this.id).then(() => this.hub.flush());
    this.onclose?.({ code });
  }
  pump() {
    if (this.held || this.readyState !== 1) return;
    const out = this.hub.room.out[this.id] ?? [];
    while (this.cursor < out.length) this.onmessage?.({ data: JSON.stringify(out[this.cursor++]) });
    const code = this.hub.room.closed[this.id];
    if (code === undefined) return;
    this.readyState = 3;
    this.onclose?.({ code });
  }
}

export class Hub {
  readonly sockets: FakeSocket[] = [];
  /** 送達時包一層（測試裡給 React 的 act，免得畫面更新跑在 act 外） */
  wrap: (fn: () => void) => void = (fn) => fn();
  constructor(readonly room: Room) {}
  open = () => {
    const s = new FakeSocket(this, `p${this.sockets.length + 1}`);
    this.sockets.push(s);
    return s;
  };
  flush() {
    this.wrap(() => {
      for (const s of this.sockets) s.pump();
    });
  }
  /** 執行端送一則訊框，然後把結果交給各手機 */
  async exec(t: string, p: unknown = {}) {
    await this.room.send("x1", t, p);
    this.flush();
  }
}
