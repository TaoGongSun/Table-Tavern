// Node 殼的儲存：所有房間共用一個 SQLite 檔（自架者只要備份一個檔）。
// 表照 workers/room-do.ts 的 SqlRoomStore（room／cards／events）各加房間碼欄，另加 alarms 給啟動掃描用。
import { DatabaseSync } from "node:sqlite";
import type { RoomStore } from "../core/runner";
import type { RoomState, StoredEvent } from "../core/types";

export class RoomDb {
  readonly db: DatabaseSync;
  constructor(path: string) {
    this.db = new DatabaseSync(path);
    this.db.exec(
      "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;" +
        "CREATE TABLE IF NOT EXISTS room (code TEXT PRIMARY KEY, state TEXT NOT NULL);" +
        "CREATE TABLE IF NOT EXISTS cards (code TEXT PRIMARY KEY, cards TEXT NOT NULL);" +
        "CREATE TABLE IF NOT EXISTS events (code TEXT NOT NULL, seq INTEGER NOT NULL, at INTEGER NOT NULL, type TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY (code, seq));" +
        "CREATE TABLE IF NOT EXISTS alarms (code TEXT PRIMARY KEY, at INTEGER NOT NULL);",
    );
  }
  tx(fn: () => void) {
    this.db.exec("BEGIN");
    try {
      fn();
      this.db.exec("COMMIT");
    } catch (e) {
      this.db.exec("ROLLBACK");
      throw e;
    }
  }
  setAlarm(code: string, at: number | null) {
    if (at === null) this.db.prepare("DELETE FROM alarms WHERE code = ?").run(code);
    else this.db.prepare("INSERT OR REPLACE INTO alarms (code, at) VALUES (?, ?)").run(code, at);
  }
  /** 啟動掃描：每個存活房間的鬧鐘；缺列（崩潰在 commit 與 alarm 之間）當作立刻到期 */
  pendingAlarms(): Array<{ code: string; at: number }> {
    return this.db
      .prepare(
        "SELECT room.code AS code, COALESCE(alarms.at, 0) AS at FROM room LEFT JOIN alarms USING (code)",
      )
      .all() as Array<{ code: string; at: number }>;
  }
  store(code: string): RoomStore {
    return new SqliteRoomStore(this, code);
  }
}

class SqliteRoomStore implements RoomStore {
  private writtenCards: unknown = undefined;
  constructor(
    private readonly d: RoomDb,
    private readonly code: string,
  ) {}
  private get db() {
    return this.d.db;
  }
  async load() {
    const row = this.db.prepare("SELECT state FROM room WHERE code = ?").get(this.code) as
      | { state: string }
      | undefined;
    if (!row) return { state: null };
    const state = JSON.parse(row.state) as RoomState;
    state.cards = JSON.parse(
      (
        this.db.prepare("SELECT cards FROM cards WHERE code = ?").get(this.code) as {
          cards: string;
        }
      ).cards,
    );
    this.writtenCards = state.cards;
    return { state };
  }
  async commit(state: RoomState, events: StoredEvent[]) {
    this.d.tx(() => {
      this.db
        .prepare("INSERT OR REPLACE INTO room (code, state) VALUES (?, ?)")
        .run(this.code, JSON.stringify({ ...state, cards: undefined }));
      if (state.cards !== this.writtenCards)
        this.db
          .prepare("INSERT OR REPLACE INTO cards (code, cards) VALUES (?, ?)")
          .run(this.code, JSON.stringify(state.cards));
      const ins = this.db.prepare(
        "INSERT INTO events (code, seq, at, type, payload) VALUES (?, ?, ?, ?, ?)",
      );
      for (const e of events) ins.run(this.code, e.seq, e.at, e.type, JSON.stringify(e.payload));
    });
    this.writtenCards = state.cards;
  }
  private rows(query: string, seq: number, limit: number): StoredEvent[] {
    const rows = this.db.prepare(query).all(this.code, seq, limit) as Array<{
      seq: number;
      at: number;
      type: string;
      payload: string;
    }>;
    return rows.map((r) => ({
      seq: r.seq,
      at: r.at,
      type: r.type,
      payload: JSON.parse(r.payload),
    }));
  }
  async eventsAfter(seq: number, limit: number) {
    return this.rows(
      "SELECT * FROM events WHERE code = ? AND seq > ? ORDER BY seq LIMIT ?",
      seq,
      limit,
    );
  }
  async eventsBefore(seq: number, limit: number) {
    return this.rows(
      "SELECT * FROM events WHERE code = ? AND seq < ? ORDER BY seq DESC LIMIT ?",
      seq,
      limit,
    ).reverse();
  }
  async deleteAll() {
    this.d.tx(() => {
      for (const t of ["room", "cards", "events", "alarms"])
        this.db.prepare(`DELETE FROM ${t} WHERE code = ?`).run(this.code);
    });
    this.writtenCards = undefined;
  }
}
