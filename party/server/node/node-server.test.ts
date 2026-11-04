// Node 殼：真 http＋ws＋node:sqlite 暫存檔。驗：ev 送出當下已落地、關掉重開同一 DB 接著跑、鬧鐘排回與分段、
// 成員心跳與掃描、錯誤訊框不拖垮程序、卸載與重連交錯不會出現兩個 runner、限流、來源、靜態檔。
import { EventEmitter } from "node:events";
import {
  mkdtempSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import type { Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { createPartyServer } from "./http";
import { Rooms } from "./rooms";
import { RoomDb } from "./sqlite-store";

const ORIGIN = "https://party.test";
const hello = (p: Record<string, unknown>) =>
  JSON.stringify({ t: "hello", id: "h", p: { protocol: 1, lastSeq: 0, client: {}, ...p } });
const card = {
  cardId: "c1",
  name: "甲",
  color: null,
  avatar: "data:image/png;base64,AAAA",
  blurb: "",
  playable: true,
  hidden: false,
};
const open: Array<() => Promise<void>> = [];
afterEach(async () => {
  while (open.length) await open.pop()!();
});

function tmp() {
  const dir = mkdtempSync(join(tmpdir(), "party-node-"));
  mkdirSync(join(dir, "dist", "assets"), { recursive: true });
  writeFileSync(join(dir, "dist", "index.html"), "<!doctype html>app");
  writeFileSync(join(dir, "dist", "assets", "a.js"), "export {}");
  writeFileSync(join(dir, "secret.txt"), "nope");
  return dir;
}

async function start(dir: string, opts: { now?: () => number } = {}) {
  const db = new RoomDb(join(dir, "rooms.sqlite"));
  const rooms = new Rooms(db, opts);
  const server: Server = createPartyServer(rooms, {
    publicOrigin: ORIGIN,
    trustProxy: false,
    idleDays: 30,
    staticDir: join(dir, "dist"),
  });
  rooms.restore();
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  const stop = async () => {
    server.close();
    await rooms.shutdown(2000);
    db.db.close();
  };
  open.push(async () => {
    if (db.db.isOpen) await stop();
  });
  return { db, rooms, base, stop, dir };
}

class Client {
  frames: any[] = [];
  closed: number | null = null;
  private waiters: Array<() => void> = [];
  constructor(
    readonly ws: WebSocket,
    onFrame: (f: any) => void = () => {},
  ) {
    ws.on("message", (d) => {
      const f = JSON.parse(String(d));
      onFrame(f);
      this.frames.push(f);
      this.waiters.splice(0).forEach((w) => w());
    });
    ws.on("close", (code) => {
      this.closed = code;
      this.waiters.splice(0).forEach((w) => w());
    });
    ws.on("error", () => {});
  }
  static async connect(
    base: string,
    code: string,
    role: "member" | "executor",
    onFrame?: (f: any) => void,
    origin: string | undefined = ORIGIN,
  ) {
    const ws = new WebSocket(
      `${base.replace("http", "ws")}/api/rooms/${code}/ws?role=${role}`,
      origin ? { origin } : {},
    );
    const c = new Client(ws, onFrame);
    await new Promise((r, j) => (ws.once("open", r), ws.once("error", j)));
    return c;
  }
  async until(pred: (f: any) => boolean, ms = 3000): Promise<any> {
    const end = Date.now() + ms;
    for (;;) {
      const hit = this.frames.find(pred);
      if (hit) return hit;
      if (Date.now() > end)
        throw new Error(`timeout; got ${JSON.stringify(this.frames.map((f) => f.t))}`);
      await new Promise<void>((r) => (this.waiters.push(r), setTimeout(r, 50)));
    }
  }
  async closedWith(ms = 3000) {
    const end = Date.now() + ms;
    while (this.closed === null && Date.now() < end) await new Promise((r) => setTimeout(r, 20));
    return this.closed;
  }
}

async function createRoom(base: string) {
  const res = await fetch(`${base}/api/rooms`, {
    method: "POST",
    body: JSON.stringify({ protocol: 1, label: "電腦", client: { name: "desktop", version: "0" } }),
  });
  expect(res.status).toBe(201);
  return (await res.json()) as {
    roomCode: string;
    joinSecret: string;
    executorToken: string;
    inviteUrl: string;
  };
}

describe("Node 自架殼", () => {
  it("建房、執行端與成員連線；ev 送出當下已落地；關掉用同一 DB 重開後序號接續、舊連線補離線", async () => {
    const dir = tmp();
    const a = await start(dir);
    const room = await createRoom(a.base);
    expect(room.inviteUrl).toBe(`${ORIGIN}/r/${room.roomCode}#k=${room.joinSecret}`);
    const landed = (f: any) => {
      if (f.t === "ev")
        expect(
          a.db.db
            .prepare("SELECT seq FROM events WHERE code = ? AND seq = ?")
            .all(room.roomCode, f.p.seq),
        ).toHaveLength(1);
    };
    const exec = await Client.connect(a.base, room.roomCode, "executor", landed, undefined);
    exec.ws.send(
      hello({
        role: "executor",
        roomCode: room.roomCode.toLowerCase(),
        executorToken: room.executorToken,
        inflight: null,
      }),
    );
    await exec.until((f) => f.t === "welcome");
    exec.ws.send(JSON.stringify({ t: "roster.update", p: { cards: [card] } }));
    const member = await Client.connect(a.base, room.roomCode, "member", landed);
    member.ws.send(
      hello({
        role: "member",
        deviceId: "d".repeat(22),
        joinSecret: room.joinSecret,
        nickname: "阿明",
      }),
    );
    const welcome = await member.until((f) => f.t === "welcome");
    expect(welcome.p.snapshot.cards[0].avatar).toBe(card.avatar);
    await exec.until((f) => f.p?.e?.type === "member.joined");
    member.ws.send('{"t":"ping"}');
    await member.until((f) => f.t === "pong");

    await a.stop();
    expect(await member.closedWith()).toBe(1001);
    const b = await start(dir);
    const again = await Client.connect(b.base, room.roomCode, "member");
    again.ws.send(
      hello({
        role: "member",
        deviceId: "d".repeat(22),
        joinSecret: room.joinSecret,
        nickname: "阿明",
        lastSeq: welcome.p.snapshot.seq ?? 0,
      }),
    );
    await again.until((f) => f.t === "welcome");
    again.ws.send(JSON.stringify({ t: "seat.claim", id: "s1", p: { cardId: "c1" } }));
    await again.until((f) => f.t === "ok" && f.p.id === "s1");
    const seqs = (
      b.db.db
        .prepare("SELECT seq FROM events WHERE code = ? ORDER BY seq")
        .all(room.roomCode) as Array<{ seq: number }>
    ).map((r) => r.seq);
    expect(seqs).toEqual(seqs.map((_, i) => i + 1)); // 重開後序號接續、不重號
    expect(
      (
        b.db.db.prepare("SELECT type FROM events WHERE code = ?").all(room.roomCode) as Array<{
          type: string;
        }>
      ).map((r) => r.type),
    ).toContain("seat.claimed");
  });

  it("錯誤訊框（超大、非法 UTF-8）與畸形路徑不拖垮程序，別的房照常；靜態檔、穿越、來源、限流", async () => {
    const s = await start(tmp());
    const r1 = await createRoom(s.base);
    const big = await Client.connect(s.base, r1.roomCode, "executor", undefined, undefined);
    big.ws.send("x".repeat(1024 * 1024 + 10));
    expect(await big.closedWith()).toBe(1009);
    const bad = await Client.connect(s.base, r1.roomCode, "member");
    bad.ws.send(Buffer.from([0x7b, 0xff, 0xfe, 0x7d]), { binary: false });
    expect(await bad.closedWith()).toBe(1007);
    expect((await fetch(`${s.base}/%E0%A4%A`)).status).toBe(400);
    expect((await fetch(`${s.base}/..%2Fsecret.txt`)).status).toBe(404);
    expect(await (await fetch(`${s.base}/%2e%2e/secret.txt`)).text()).toBe("<!doctype html>app"); // URL 解析先把 .. 收掉，落到單頁
    expect(await (await fetch(`${s.base}/r/${r1.roomCode}`)).text()).toBe("<!doctype html>app");
    // 根內的符號連結指向根外：檔案、目錄都不送
    symlinkSync(join(s.dir, "secret.txt"), join(s.dir, "dist", "leak.txt"));
    symlinkSync(s.dir, join(s.dir, "dist", "up"));
    expect((await fetch(`${s.base}/leak.txt`)).status).toBe(404);
    expect((await fetch(`${s.base}/up/secret.txt`)).status).toBe(404);
    const asset = await fetch(`${s.base}/assets/a.js`);
    expect(asset.headers.get("cache-control")).toContain("immutable");
    await expect(
      Client.connect(s.base, r1.roomCode, "member", undefined, "https://evil.test"),
    ).rejects.toThrow(/403/);
    expect((await fetch(`${s.base}/api/rooms/XXXX/ws`)).status).toBe(404);

    // 另一房照常
    const r2 = await createRoom(s.base);
    const m = await Client.connect(s.base, r2.roomCode, "member");
    m.ws.send(
      hello({
        role: "member",
        deviceId: "e".repeat(22),
        joinSecret: r2.joinSecret,
        nickname: "乙",
      }),
    );
    await m.until((f) => f.t === "welcome");
    // 同 IP 每小時 10 次：已建 2 房，再 8 次過、第 11 次 429
    for (let i = 0; i < 8; i++) await createRoom(s.base);
    const limited = await fetch(`${s.base}/api/rooms`, { method: "POST", body: "{}" });
    expect(limited.status).toBe(429);
    expect(((await limited.json()) as { retryAfterMs: number }).retryAfterMs).toBeGreaterThan(
      3_500_000,
    );
  });

  it("45 秒沒心跳的成員被 1001 關掉並廣播離線；送過 ping 的留下", async () => {
    let clock = Date.now();
    const s = await start(tmp(), { now: () => clock });
    const r = await createRoom(s.base);
    const [a, b] = [
      await Client.connect(s.base, r.roomCode, "member"),
      await Client.connect(s.base, r.roomCode, "member"),
    ];
    a.ws.send(
      hello({ role: "member", deviceId: "a".repeat(22), joinSecret: r.joinSecret, nickname: "a" }),
    );
    await a.until((f) => f.t === "welcome");
    b.ws.send(
      hello({ role: "member", deviceId: "b".repeat(22), joinSecret: r.joinSecret, nickname: "b" }),
    );
    await b.until((f) => f.t === "welcome");
    clock += 46_000;
    b.ws.send('{"t":"ping"}');
    await b.until((f) => f.t === "pong");
    s.rooms.sweepAll();
    expect(await a.closedWith()).toBe(1001);
    await b.until(
      (f) => f.p?.type === "presence" && f.p.connected === false && f.p.memberId === "m1",
    );
  });

  it("鬧鐘：超過 setTimeout 上限分段排；啟動掃描把缺列的房間當作立刻到期", async () => {
    const dir = tmp();
    const db = new RoomDb(join(dir, "rooms.sqlite"));
    const timers: Array<{ fn: () => void; ms: number }> = [];
    let clock = 0;
    const rooms = new Rooms(db, {
      now: () => clock,
      setTimer: (fn, ms) => timers.push({ fn, ms }),
      clearTimer: () => {},
    });
    rooms.arm("AAAA2222", 40 * 86_400_000);
    expect(timers[0].ms).toBe(2 ** 31 - 1);
    clock = 2 ** 31 - 1;
    timers[0].fn();
    expect(timers[1].ms).toBe(40 * 86_400_000 - (2 ** 31 - 1)); // 還沒到：再排剩下的
    db.db.prepare("INSERT INTO room (code, state) VALUES (?, ?)").run("BBBB3333", "{}");
    db.setAlarm("CCCC4444", 5); // 沒有 room 列的殘留鬧鐘不排
    expect(db.pendingAlarms()).toEqual([{ code: "BBBB3333", at: 0 }]);
    db.db.close();
  });

  it("卸載與重連交錯：沒有連線的房間有工作在等時，另一段工作先結束也不卸載，重連拿到同一個 Room", async () => {
    const dir = tmp();
    const db = new RoomDb(join(dir, "rooms.sqlite"));
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const store = db.store.bind(db);
    db.store = (code) => {
      const s = store(code);
      const load = s.load.bind(s);
      s.load = async () => (await gate, load());
      return s;
    };
    const rooms = new Rooms(db);
    const alarm = rooms.with("DDDD5555", (r) => r.run({ kind: "alarm" })); // 卡在 load，房間沒有連線
    const first = rooms.peek("DDDD5555");
    rooms.sweepAll(); // 這段工作立刻結束；不計工作數的話會在這裡把房間卸掉
    await new Promise((r) => setTimeout(r, 10));
    const w = Object.assign(new EventEmitter(), {
      send: () => {},
      close: () => {},
    }) as unknown as WebSocket;
    const attached = rooms.attach("DDDD5555", w, "member");
    expect(rooms.peek("DDDD5555")).toBe(first);
    release();
    await Promise.all([alarm, attached]);
    w.emit("close");
    await new Promise((r) => setTimeout(r, 20));
    expect(rooms.size).toBe(0);
    db.db.close();
  });

  it("玩家卡上傳、執行端取檔、存秘密段走真 HTTP 與 WS；上傳本體與秘密段不寫進 SQLite 檔", async () => {
    const dir = tmp();
    const s = await start(dir);
    const room = await createRoom(s.base);
    const exec = await Client.connect(s.base, room.roomCode, "executor", undefined, undefined);
    exec.ws.send(
      hello({
        role: "executor",
        roomCode: room.roomCode,
        executorToken: room.executorToken,
        inflight: null,
      }),
    );
    await exec.until((f) => f.t === "welcome");
    const member = await Client.connect(s.base, room.roomCode, "member");
    member.ws.send(
      hello({
        role: "member",
        deviceId: "d".repeat(22),
        joinSecret: room.joinSecret,
        nickname: "阿明",
      }),
    );
    await member.until((f) => f.t === "welcome");
    const SECRET = "祕密標記NODE";
    const url = `${s.base}/api/rooms/${room.roomCode}/cards`;
    const auth = { authorization: `Bearer ${"d".repeat(22)}` };
    expect(
      (
        await fetch(`${url}?kind=card`, {
          method: "POST",
          headers: { ...auth, origin: "https://evil.test" },
          body: "{}",
        })
      ).status,
    ).toBe(403);
    expect(
      (
        await fetch(`${url}?kind=card`, {
          method: "POST",
          headers: auth,
          body: new Uint8Array(9 << 20),
        })
      ).status,
    ).toBe(413); // 本體沒讀也要回得到
    expect(
      (
        await fetch(`${url}?kind=image`, {
          method: "POST",
          headers: auth,
          body: new Uint8Array(1 << 20),
        })
      ).status,
    ).toBe(403);
    const up = await fetch(`${url}?kind=card`, {
      method: "POST",
      headers: auth,
      body: `{"name":"${SECRET}"}`,
    });
    expect(up.status).toBe(202);
    const job = await exec.until((f) => f.t === "job.card_import");
    expect(job.p.uploadId).toBe(((await up.json()) as { uploadId: string }).uploadId);
    const got = await fetch(`${url}/${job.p.uploadId}`, {
      headers: { authorization: `Bearer ${room.executorToken}` },
    });
    expect(got.headers.get("content-type")).toBe("application/octet-stream");
    expect(await got.text()).toBe(`{"name":"${SECRET}"}`);
    expect(
      (
        await fetch(`${url}/${job.p.uploadId}`, {
          headers: { authorization: `Bearer ${room.executorToken}` },
        })
      ).status,
    ).toBe(404);
    exec.ws.send(JSON.stringify({ t: "roster.update", p: { cards: [card] } }));
    exec.ws.send(
      JSON.stringify({
        t: "job.result",
        p: { jobId: job.p.jobId, ok: true, result: { cardId: "c1" } },
      }),
    );
    expect((await member.until((f) => f.p?.type === "card_upload")).p).toMatchObject({
      ok: true,
      cardId: "c1",
    });
    member.ws.send(
      JSON.stringify({ t: "card.save", id: "s1", p: { cardId: "c1", privateMd: SECRET } }),
    );
    const save = await exec.until((f) => f.t === "card.save");
    exec.ws.send(JSON.stringify({ t: "card.save.result", p: { reqId: save.p.reqId, ok: true } }));
    await member.until((f) => f.t === "ok" && f.p.id === "s1");
    await s.stop();
    for (const f of readdirSync(dir).filter((n) => n.startsWith("rooms.sqlite")))
      expect(readFileSync(join(dir, f)).includes(Buffer.from(SECRET))).toBe(false);
  });

  it("單頁 fallback 的 index.html 是指向根外的連結時不送", async () => {
    const dir = tmp();
    writeFileSync(join(dir, "outside.html"), "outside");
    const s = await start(dir);
    rmSync(join(dir, "dist", "index.html"));
    symlinkSync(join(dir, "outside.html"), join(dir, "dist", "index.html"));
    expect((await fetch(`${s.base}/r/ABCD2345`)).status).toBe(404);
  });
});
