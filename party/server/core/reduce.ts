// 核心入口（協定 §12.1）：(狀態, 輸入) → 新狀態＋事件＋要送的訊框。只用語言內建、TextEncoder 與 WebCrypto。
// 與協定 §12.1 的差異：reduce 是 async（WebCrypto 的 SHA-256 只有非同步版，讀事件也要 await），
// 所以單執行緒保證改由 RoomRunner 的每房序列化佇列提供；ctx 多帶 live（不落地的記憶體值）與 store 的讀取。
import { CARD_EXECUTOR, CARD_MEMBER, cardFetch, cardsAlarm, failPending, memberByDevice, settleCards, uploadBody, uploadReserve } from "./cards";
import { EXECUTOR } from "./executor";
import { toWebSave } from "./export";
import { closeRoom, evFrame, MEMBER, pairClaim, rate } from "./member";
import { announceExec, nextAlarm, onAlarm, pushDispatch, setExecOnline, settle } from "./round";
import { sameHash, secret, sha256Hex } from "./secrets";
import { liveValues, sendSnapshot } from "./snapshot";
import { Fail, need, Tx } from "./tx";
import type { CoreCtx, CoreInput, CoreOutput, ErrorCode, Frame, RoomState } from "./types";
import { cleanName, shape, validateFrame } from "./validate";

export { buildSnapshot } from "./snapshot";
export { toWebSave } from "./export";
export { validateFrame } from "./validate";

const CLOSE: Partial<Record<ErrorCode, number>> = {
  E_BAD_FRAME: 4400, E_VERSION: 4426, E_AUTH_REQUIRED: 4401, E_AUTH_FAILED: 4401, E_ROOM_NOT_FOUND: 4404, E_BANNED: 4403,
};
const HTTP: Partial<Record<ErrorCode, number>> = {
  E_ROOM_NOT_FOUND: 404, E_DISPATCH_UNKNOWN: 404, E_AUTH_FAILED: 401, E_FORBIDDEN: 403, E_BANNED: 403, E_SEAT_REQUIRED: 403, E_STATE_CONFLICT: 409,
  E_ROOM_CLOSED: 410, E_LENGTH_REQUIRED: 411, E_TOO_LARGE: 413, E_UNSUPPORTED_FORMAT: 415, E_RATE_LIMITED: 429, E_EXECUTOR_OFFLINE: 503,
};
const MEMBER_ALL = { ...MEMBER, ...CARD_MEMBER };
const EXECUTOR_ALL = { ...EXECUTOR, ...CARD_EXECUTOR };
const READONLY = new Set(["ping", "history.page"]);
const TOKEN = (n: number) => new RegExp(`^[A-Za-z0-9_-]{${n}}$`);
const hashOf = (s: RoomState, secretText: string) => sha256Hex(`${s.code}:${secretText}`);

function initialState(code: string, now: number, idleDays: number, label: string, joinHash: string, execHash: string): RoomState {
  return {
    code, status: "lobby", settings: { maxPlayers: 6, actionTimeoutSec: null, shuffleOrder: false, allowCustomCards: true },
    hostId: null, payerId: null, scene: 0, sceneSettings: null, nextSettings: null, transition: null,
    createdAt: now, lastActive: now, idleDays, closedAt: null, joinHash, execHash,
    exec: { label, online: false, since: now, lastSeen: now, keepAwake: null, busy: false, caps: null, quota: null }, execConn: null,
    seq: 0, roundNo: 0, sceneStartSeq: 0, summarySeqs: [], seeded: false, round: null, members: [], memberNo: 0, seatNo: 0,
    cards: [], drafts: {}, pair: null, pairFails: 0, pairLockUntil: 0, departures: [], jobs: [], doneJobs: [], lastDoneDispatch: null, ctls: {}, conns: {},
  };
}

/** 重連（§4.3）：落後 ≤ 500 筆逐筆補 `ev`，否則（含第一次）給快照。快照在任何新事件之前送出 */
async function welcome(tx: Tx, lastSeq: number, extra: Record<string, unknown>) {
  const s = tx.s;
  const who = s.conns[tx.conn].who!;
  if (lastSeq > 0 && lastSeq <= s.seq && s.seq - lastSeq <= 500) {
    tx.send(tx.conn, "welcome", { ...(who !== "exec" ? { memberId: who } : {}), live: liveValues(s, tx.ctx.live, who), ...extra });
    for (const e of [...(await tx.ctx.store.eventsAfter(lastSeq, 500)), ...tx.events]) if (e.seq > lastSeq) tx.send(tx.conn, "ev", evFrame(e));
    return;
  }
  await sendSnapshot(tx, tx.conn, who, extra);
}

async function memberHello(tx: Tx, p: any) {
  need(p.protocol === 1, "E_VERSION");
  need(p.role === "member" && TOKEN(22).test(p.deviceId), "E_AUTH_FAILED");
  const s = tx.s;
  const dev = await hashOf(s, p.deviceId);
  const known = s.members.find((x) => sameHash(x.dev, dev));
  need(!known?.banned, "E_BANNED");
  const w = tx.w;
  let m = known && w.members.find((x) => x.id === known.id);
  if (!m) {
    need(s.status !== "closed", "E_ROOM_CLOSED");
    need(typeof p.joinSecret === "string" && TOKEN(22).test(p.joinSecret) && sameHash(await hashOf(s, p.joinSecret), s.joinHash), "E_AUTH_FAILED");
    const nickname = cleanName(p.nickname);
    need(nickname, "E_NAME_INVALID");
    need(s.members.filter((x) => !x.banned).length < 12, "E_ROOM_FULL");
    m = { id: `m${++w.memberNo}`, dev, nickname, cardId: null, seat: 0, banned: false, arriving: false };
    w.members.push(m);
    tx.emit("member.joined", { memberId: m.id, nickname });
  }
  for (const [id, c] of Object.entries(w.conns)) if (c.who === m.id) tx.shut(id, 4409, "E_SESSION_REPLACED");
  w.conns[tx.conn].who = m.id;
  await welcome(tx, p.lastSeq, {});
  tx.toAll("vol", { type: "presence", memberId: m.id, connected: true });
  if (typeof p.pairCode !== "string") return;
  try {
    await pairClaim(tx, m, p.pairCode); // 出額度者的 QR：加入與配對一步完成
  } catch (e) {
    if (!(e instanceof Fail)) throw e;
    tx.err(tx.conn, e.code, { retryAfterMs: e.retryAfterMs });
  }
}

async function executorHello(tx: Tx, p: any) {
  const s = tx.s;
  need(p.protocol === 1, "E_VERSION");
  const token = p.role === "executor" && String(p.roomCode).toUpperCase() === s.code && TOKEN(43).test(p.executorToken);
  need(token && s.execHash && sameHash(await hashOf(s, p.executorToken), s.execHash), "E_AUTH_FAILED");
  need(s.status !== "closed", "E_ROOM_CLOSED");
  failPending(tx, "E_EXECUTOR_OFFLINE"); // 上一個執行端連線收到的讀卡、存卡、上傳不會再有結果
  const w = tx.w;
  if (w.execConn && w.execConn !== tx.conn) tx.shut(w.execConn, 4409, "E_EXECUTOR_REPLACED");
  w.conns[tx.conn].who = "exec";
  Object.assign(w.exec, { online: true, since: tx.now, lastSeen: tx.now, paused: false });
  w.execConn = tx.conn;
  const r = w.round;
  // 重連去重（§6.5）：同一回合的派工沿用原 dispatchId；執行端報告它已經接手（任何狀態）就視為 generating
  const dispatch = r?.dispatch && (r.phase === "queued" || r.phase === "generating") ? r.dispatch : null;
  if (dispatch) {
    r!.genAt = tx.now;
    if (p.inflight?.dispatch?.dispatchId === dispatch.dispatchId) r!.phase = "generating";
  }
  const jobs = w.jobs.map((j) => j.job);
  await welcome(tx, p.lastSeq, { dispatch, job: jobs[0] ?? null });
  announceExec(tx);
  for (const job of jobs.slice(1)) tx.toExec(`job.${job.kind}`, job);
  if (r && !r.dispatch) pushDispatch(tx); // 執行端離線時停在 queued 的回合，回來才產生派工
}

async function onFrame(tx: Tx, role: "member" | "executor", f: Frame) {
  need(tx.exists, "E_ROOM_NOT_FOUND");
  const c = tx.s.conns[tx.conn];
  if (f.t === "hello") {
    need(!c?.who, "E_STATE_CONFLICT");
    tx.w.conns[tx.conn] ??= { role, who: null, at: tx.now };
    return role === "member" ? memberHello(tx, f.p) : executorHello(tx, f.p);
  }
  need(c?.who, "E_AUTH_REQUIRED");
  if (role === "executor") {
    if (tx.now - tx.s.exec.lastSeen >= 5000 || f.t === "hb") tx.w.exec.lastSeen = tx.now; // 任何訊框都算在線，寫入節流
    if ((await EXECUTOR_ALL[f.t](tx, f.p)) !== "later") tx.ok();
    return;
  }
  rate(tx, `c:${tx.conn}`, 20, 1000);
  if (tx.s.status === "closed") need(READONLY.has(f.t) || f.t === "member.leave", "E_ROOM_CLOSED");
  const m = (READONLY.has(f.t) ? tx.s : tx.w).members.find((x) => x.id === c.who)!;
  if ((await MEMBER_ALL[f.t](tx, f.p, m)) !== "later") tx.ok();
}

async function create(tx: Tx, input: Extract<CoreInput, { route: "create" }>) {
  need(!tx.exists, "E_STATE_CONFLICT"); // Worker 進入點換一組房間碼重試
  need(shape(input.body, "protocol:i label:s client:o") && input.body.protocol === 1, "E_BAD_FRAME");
  const label = cleanName(input.body.label, 40);
  need(label, "E_NAME_INVALID");
  const joinSecret = secret(tx.ctx.rng, 16);
  const executorToken = secret(tx.ctx.rng, 32);
  const hash = (x: string) => sha256Hex(`${input.code}:${x}`);
  tx.init(initialState(input.code, tx.now, input.idleDays, label, await hash(joinSecret), await hash(executorToken)));
  tx.emit("room.created", { code: input.code });
  tx.http = { status: 201, body: { roomCode: input.code, joinSecret, executorToken, inviteUrl: `${input.origin}/r/${input.code}#k=${joinSecret}` } };
}

async function exportRoom(tx: Tx, deviceId: string) {
  need(tx.exists, "E_ROOM_NOT_FOUND");
  const s = tx.s;
  need((await memberByDevice(s, deviceId))?.id === s.hostId && s.hostId !== null, "E_FORBIDDEN");
  tx.http = { status: 200, body: toWebSave(s, await tx.ctx.store.eventsAfter(0, Number.MAX_SAFE_INTEGER), tx.now) };
}

/** 執行端憑證結束房間（桌面版桌卡「結束並刪除房間」，room-host-desktop-play）：跟房主 `room.close` 同一份收尾 */
async function closeByExecutor(tx: Tx, token: string) {
  need(tx.exists, "E_ROOM_NOT_FOUND");
  const s = tx.s;
  need(TOKEN(43).test(token) && s.execHash && sameHash(await hashOf(s, token), s.execHash), "E_AUTH_FAILED");
  if (s.status !== "closed") closeRoom(tx); // 已結束：重送的請求照樣回成功
  tx.http = { status: 200, body: { ok: true } };
}

function output(tx: Tx): CoreOutput {
  const state = tx.exists && !tx.deleteRoom ? tx.s : null;
  const alarmAt = state && Math.min(nextAlarm(state)!, cardsAlarm(tx.ctx.live));
  return { state, events: tx.events, sends: tx.sends, close: tx.close, alarmAt, ...(tx.deleteRoom ? { deleteRoom: true } : {}), ...(tx.http ? { http: tx.http } : {}) };
}

export async function reduce(state: RoomState | null, input: CoreInput, ctx: CoreCtx): Promise<CoreOutput> {
  const conn = "conn" in input ? input.conn.id : "";
  let tx = new Tx(state, ctx, conn);
  try {
    if (input.kind === "connect" && tx.exists) {
      tx.w.conns[conn] = { role: input.conn.role, who: null, at: ctx.now };
      tx.w.lastActive = ctx.now; // 閒置清理從「任何連線建立」重新起算
    }
    if (input.kind === "disconnect" && tx.s?.conns[conn]) {
      const who = tx.s.conns[conn].who;
      delete tx.w.conns[conn];
      for (const key of Object.keys(ctx.live.buckets)) if (key.endsWith(`:${conn}`)) delete ctx.live.buckets[key];
      if (who === "exec" && tx.s.execConn === conn) setExecOnline(tx, false);
      else if (who && who !== "exec" && !Object.values(tx.s.conns).some((c) => c.who === who)) tx.toAll("vol", { type: "presence", memberId: who, connected: false });
    }
    if (input.kind === "alarm" && tx.exists) onAlarm(tx);
    if (input.kind === "http") {
      if (input.route === "create") await create(tx, input);
      else if (input.route === "close") await closeByExecutor(tx, input.executorToken);
      else if (input.route === "export") await exportRoom(tx, input.deviceId);
      else if (input.route === "upload_reserve") await uploadReserve(tx, input);
      else if (input.route === "upload_body") uploadBody(tx, input);
      else await cardFetch(tx, input);
    }
    if (input.kind === "frame") {
      const v = validateFrame(input.raw, input.conn.role);
      if (!v.ok) throw new Fail(v.code);
      tx = new Tx(state, ctx, conn, v.frame.id);
      await onFrame(tx, input.conn.role, v.frame);
    }
    if (tx.exists && !tx.deleteRoom) settle(tx);
    settleCards(tx);
    return output(tx);
  } catch (e) {
    if (!(e instanceof Fail)) throw e;
    const f = new Tx(state, ctx, conn);
    if (input.kind === "http") f.http = { status: HTTP[e.code] ?? 400, body: { code: e.code, ...(e.retryAfterMs ? { retryAfterMs: e.retryAfterMs } : {}) } };
    else f.err(conn, e.code, { ...(tx.reqId ? { id: tx.reqId } : {}), ...(e.retryAfterMs ? { retryAfterMs: e.retryAfterMs } : {}), ...(e.detail ? { detail: e.detail } : {}) });
    // 還沒通過 hello 的連線遇到房間已關也直接斷（4410）；已在房裡的唯讀成員只回錯誤
    const code = CLOSE[e.code] ?? (e.code === "E_ROOM_CLOSED" && !state?.conns[conn]?.who ? 4410 : undefined);
    if (code && conn) f.shut(conn, code);
    return output(f);
  }
}
