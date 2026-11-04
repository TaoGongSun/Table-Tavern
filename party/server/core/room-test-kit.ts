// 測試用：記憶體 storage＋假連線＋可撥的時鐘，透過 RoomRunner 走與殼同一條路（序列化佇列、落地後才送）。
import { RoomRunner, type RoomStore } from "./runner";
import type { CoreInput, Frame, RoomState, StoredEvent } from "./types";

export class MemStore implements RoomStore {
  state: RoomState | null = null;
  events: StoredEvent[] = [];
  failNext = false;
  async load() {
    return { state: this.state && structuredClone(this.state) };
  }
  async commit(state: RoomState, events: StoredEvent[]) {
    if (this.failNext) {
      this.failNext = false;
      throw new Error("disk full");
    }
    this.state = structuredClone(state);
    this.events.push(...structuredClone(events));
  }
  async eventsAfter(seq: number, limit: number) {
    return this.events.filter((e) => e.seq > seq).slice(0, limit);
  }
  async eventsBefore(seq: number, limit: number) {
    return this.events.filter((e) => e.seq < seq).slice(-limit);
  }
  async deleteAll() {
    this.state = null;
    this.events = [];
  }
}

export const CARDS = ["c1", "c2", "c3"].map((cardId, i) => ({
  cardId,
  name: `角色${i + 1}`,
  color: null,
  avatar: null,
  blurb: "",
  playable: true,
  hidden: false,
}));
export const STAGE = {
  cardId: "npc",
  name: "店主",
  color: "#aabbcc",
  avatar: null,
  blurb: "舞台卡",
  playable: false,
  hidden: false,
};
const CAPS = {
  routes: [
    {
      route: "or",
      label: "OpenRouter",
      models: [
        { id: "m-a", label: "模型甲", laxity: 2 },
        { id: "m-b", label: "模型乙", laxity: null },
      ],
    },
  ],
  current: { route: "or", model: "m-a", modelLabel: "模型甲" },
  maxRoundSpeakers: 3,
};

export class Room {
  readonly store = new MemStore();
  clock = 1_700_000_000_000;
  readonly out: Record<string, Frame[]> = {};
  readonly closed: Record<string, number> = {};
  readonly open = new Set<string>();
  alarmAt: number | null = null;
  code = "ABCD2345";
  joinSecret = "";
  executorToken = "";
  private n = 0;
  readonly runner = new RoomRunner(
    this.store,
    {
      send: (id, text) => void (this.out[id] ??= []).push(JSON.parse(text)),
      close: (id, code) => {
        this.closed[id] = code;
        this.open.delete(id);
      },
      list: () =>
        [...this.open].map((id) => ({
          id,
          role: id.startsWith("x") ? ("executor" as const) : ("member" as const),
        })),
    },
    { set: async (at) => void (this.alarmAt = at) },
    { now: () => this.clock, rng: (n) => crypto.getRandomValues(new Uint8Array(n)) },
  );

  get s(): RoomState {
    return this.store.state!;
  }
  run(input: CoreInput) {
    return this.runner.run(input);
  }
  static async create() {
    const room = new Room();
    const out = await room.run({
      kind: "http",
      route: "create",
      body: { protocol: 1, label: "家裡的電腦", client: { name: "desktop", version: "0" } },
      code: room.code,
      origin: "https://t.example",
      idleDays: 30,
    });
    Object.assign(room, out!.http!.body);
    return room;
  }
  /** 「成員 a」連線 id 是 a，裝置碼由 id 衍生；執行端連線 id 以 x 開頭 */
  async connect(id: string) {
    this.open.add(id);
    await this.run({
      kind: "connect",
      conn: { id, role: id.startsWith("x") ? "executor" : "member" },
    });
  }
  async disconnect(id: string) {
    this.open.delete(id);
    await this.run({
      kind: "disconnect",
      conn: { id, role: id.startsWith("x") ? "executor" : "member" },
    });
  }
  send(id: string, t: string, p: unknown = {}, reqId = `q${++this.n}`) {
    return this.run({
      kind: "frame",
      conn: { id, role: id.startsWith("x") ? "executor" : "member" },
      raw: JSON.stringify({ t, id: reqId, p }),
    });
  }
  /** 送出並回傳這則的回覆（ok 或 err） */
  async ask(id: string, t: string, p: unknown = {}) {
    const reqId = `q${++this.n}`;
    await this.send(id, t, p, reqId);
    return (
      (this.out[id] ?? []).findLast((f) => (f.t === "ok" || f.t === "err") && f.p.id === reqId) ??
      null
    );
  }
  frames(id: string, t?: string, type?: string) {
    return (this.out[id] ?? []).filter(
      (f) => (!t || f.t === t) && (!type || f.p?.type === type || f.p?.e?.type === type),
    );
  }
  events(type: string) {
    return this.store.events.filter((e) => e.type === type);
  }
  async tick(ms: number) {
    this.clock += ms;
    await this.run({ kind: "alarm" });
  }
  deviceId = (id: string) => `dev-${id}`.padEnd(22, "x");
  async join(id: string, nickname = id, extra: Record<string, unknown> = {}) {
    await this.connect(id);
    return this.ask(id, "hello", {
      protocol: 1,
      role: "member",
      deviceId: this.deviceId(id),
      joinSecret: this.joinSecret,
      nickname,
      lastSeq: 0,
      client: { name: "party", version: "0" },
      ...extra,
    });
  }
  async exec(id = "x1", extra: Record<string, unknown> = {}) {
    await this.connect(id);
    const reply = await this.ask(id, "hello", {
      protocol: 1,
      role: "executor",
      roomCode: this.code,
      executorToken: this.executorToken,
      lastSeq: 0,
      inflight: null,
      client: { name: "desktop", version: "0" },
      ...extra,
    });
    await this.send(id, "capabilities", CAPS);
    return reply;
  }
  async pairCode(grantHost: boolean, execId = "x1") {
    return (await this.ask(execId, "pair.create", { grantHost }))!.p.result.pairCode as string;
  }
  dispatchId() {
    return this.s.round!.dispatchId!;
  }
  /** 執行端跑完當前派工：ack → 一段 GM（含增量）→ done */
  async finishGen(text = "GM 說話", execId = "x1") {
    const dispatchId = this.dispatchId();
    await this.send(execId, "dispatch.ack", { dispatchId });
    const part = this.s.round!.partsEnded;
    await this.send(execId, "gen.part.begin", { dispatchId, part, speaker: { kind: "gm" } });
    await this.send(execId, "gen.delta", { dispatchId, part, text });
    await this.send(execId, "gen.part.end", { dispatchId, part, text, interrupted: false });
    await this.send(execId, "gen.done", { dispatchId, aborted: false });
  }
}

/** 標準開局：執行端上線、名冊三張可玩卡＋一張舞台卡；a 掃 QR 成為房主與出額度者，a、b 入座；開房跑完第 1 回合 → acting(2) */
export async function playing(players = ["a", "b"]) {
  const room = await Room.create();
  await room.exec();
  await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
  for (const [i, id] of players.entries()) {
    await room.join(id, id, i === 0 ? { pairCode: await room.pairCode(true) } : {});
    await room.send(id, "seat.claim", { cardId: CARDS[i].cardId });
  }
  await room.send(players[0], "room.start");
  await room.finishGen("開場");
  return room;
}
