// 客戶端 → 伺服器指令（協定 §5.1）：每則先查角色再查狀態，失敗拋 Fail、狀態不動。
import { resnapshot } from "./snapshot";
import {
  activeMembers,
  announceExec,
  cancelRound,
  delegate,
  enterRoster,
  execReady,
  MIN,
  offered,
  openRound,
  isOnline,
  resolve,
  seatedMembers,
  undelegate,
  unseat,
} from "./round";
import { CROCKFORD8, secret, sameHash, sha256Hex, ulid } from "./secrets";
import { Fail, need, type Tx } from "./tx";
import type {
  HostTurn,
  Member,
  RemoteCommand,
  SceneSettings,
  RoomState,
  RosterEntry,
  StoredEvent,
} from "./types";
import { cleanName, cleanText, shape } from "./validate";

export type Handler = (tx: Tx, p: any, m: Member) => void | "later" | Promise<void | "later">;
const STORY = new Set([
  "story.part",
  "round.resolved",
  "round.undone",
  "story.truncated",
  "scene.summary",
  "scene.started",
]);
export const evFrame = (e: StoredEvent) => ({
  seq: e.seq,
  at: e.at,
  e: { type: e.type, ...e.payload },
});

export function rate(tx: Tx, key: string, max: number, windowMs: number) {
  const b = tx.ctx.live.buckets;
  if (!b[key] || tx.now >= b[key].start + windowMs) b[key] = { start: tx.now, n: 0 };
  if (++b[key].n > max)
    throw new Fail("E_RATE_LIMITED", undefined, b[key].start + windowMs - tx.now);
}

const host = (tx: Tx, m: Member) => need(tx.s.hostId === m.id, "E_FORBIDDEN");
const hostOrPayer = (tx: Tx, m: Member) =>
  need(tx.s.hostId === m.id || tx.s.payerId === m.id, "E_FORBIDDEN");
const open = (s: RoomState) =>
  need(s.status === "lobby" || s.status === "playing", "E_STATE_CONFLICT");

/** 針對某回合的指令：回合號＝版本號（§3.3） */
function roundAt(tx: Tx, round: number, phase = "acting") {
  need(!tx.s.undo, "E_ROUND_NOT_ACTING"); // 收回在等執行端
  const r = tx.s.round;
  need(r && round === r.round, "E_ROUND_STALE");
  need(r.phase === phase, phase === "acting" ? "E_ROUND_NOT_ACTING" : "E_STATE_CONFLICT");
  return r;
}
function myEntry(tx: Tx, m: Member, round: number): RosterEntry {
  need(m.cardId, "E_SEAT_REQUIRED");
  roundAt(tx, round);
  const e = tx.w.round!.roster.find((x) => x.memberId === m.id);
  need(e, "E_STATE_CONFLICT");
  return e;
}

export function claim(tx: Tx, m: Member, cardId: unknown) {
  const s = tx.s;
  open(s);
  need(!m.banned, "E_FORBIDDEN"); // 匯入、自建卡的結果晚到時，人可能已被封鎖
  need(!m.cardId, "E_STATE_CONFLICT");
  const card = s.cards.find((c) => c.cardId === cardId);
  need(card && card.playable && !card.hidden && card.claimedBy === null, "E_SEAT_TAKEN");
  need(!card.reserved || (s.reserved && sameHash(m.dev, s.reserved.dev)), "E_SEAT_TAKEN");
  need(seatedMembers(s).length < s.settings.maxPlayers, "E_ROOM_FULL");
  const w = tx.w;
  Object.assign(m, { cardId: card.cardId, seat: ++w.seatNo });
  w.cards = w.cards.map((c) => (c.cardId === card.cardId ? { ...c, claimedBy: m.id } : c));
  tx.emit("seat.claimed", { memberId: m.id, cardId: card.cardId });
  const r = w.round;
  if (w.status !== "playing") return;
  // 名單空著停住的 acting：直接加進當前名單並算登場；其餘下一次開 acting 才進（§3.4）
  if (r?.phase !== "acting" || r.roster.length > 0) return void (m.arriving = true);
  enterRoster(tx, m, true);
}

/** 結束房間（房主 `room.close` 或執行端憑證 HTTP）：關連線；執行端憑證雜湊留著給重送的結束請求驗身分，執行端連線另擋 */
export function closeRoom(tx: Tx) {
  need(tx.s.status !== "closed", "E_ROOM_CLOSED");
  const w = tx.w;
  Object.assign(w, {
    status: "closed",
    closedAt: tx.now,
    transition: null,
    jobs: [],
    pair: null,
    execConn: null,
  });
  w.exec.online = false;
  if (w.round) Object.assign(w.round, { deadline: null, pausedRemainingMs: null }); // 關房後倒數不再到期
  tx.emit("room.closed");
  for (const id of Object.keys(w.conns)) tx.shut(id, 4410);
}

/** 配對碼（§1.2）：全房錯 5 次鎖 10 分鐘；錯誤計數要落地，所以不拋 Fail 而是回錯誤後照常提交 */
export async function pairClaim(tx: Tx, m: Member, raw: string): Promise<"later" | void> {
  const s = tx.s;
  need(s.status !== "closed", "E_ROOM_CLOSED");
  if (s.pairLockUntil > tx.now)
    throw new Fail("E_PAIR_LOCKED", undefined, s.pairLockUntil - tx.now);
  const code = raw.toUpperCase().replace(/-/g, "");
  const p = s.pair;
  if (
    !(
      CROCKFORD8.test(code) &&
      p &&
      p.expiresAt > tx.now &&
      sameHash(await sha256Hex(`${s.code}:${code}`), p.hash)
    )
  ) {
    const w = tx.w;
    if (++w.pairFails >= 5) Object.assign(w, { pairFails: 0, pairLockUntil: tx.now + 10 * MIN });
    tx.err(tx.conn, "E_PAIR_CODE_INVALID", { id: tx.reqId });
    return "later" as const;
  }
  need(!s.payerId || p.grantHost, "E_STATE_CONFLICT");
  const w = tx.w;
  Object.assign(w, { pair: null, pairFails: 0 });
  // grantHost 碼只有執行端那台電腦發得出來：拿到它＝GM 機器授權，房主連同出額度者一起轉（房主交給手機、接回電腦）
  if (s.payerId !== m.id && (!s.payerId || p.grantHost)) {
    if (s.payerId) tx.emit("pair.unbound", { memberId: s.payerId });
    w.payerId = m.id;
    tx.emit("pair.bound", { memberId: m.id });
    announceExec(tx); // 新的出額度者拿到路線與模型清單
    await resnapshot(tx, m.id); // 快照裡出額度者專屬的額度明細與錯誤細節（hello 時那份是配對前的）
  }
  if (p.grantHost) {
    w.hostId = m.id;
    tx.emit("host.changed", { memberId: m.id });
  }
  tx.toExec("pair.claimed", { memberId: m.id, nickname: m.nickname, grantHost: p.grantHost });
}

/** 遠端控制（§7.3）：pause 由房主下，其餘只有出額度者；retry／切換會先把 held 轉回 generating，執行端回失敗再退回 */
function ctl(tx: Tx, m: Member, p: { ctlId: string; cmd: any }) {
  const s = tx.s;
  need(p.cmd.kind === "pause" ? s.hostId === m.id : s.payerId === m.id, "E_FORBIDDEN");
  need(s.exec.online, "E_EXECUTOR_OFFLINE");
  need(/^[A-Za-z0-9_-]{1,36}$/.test(p.ctlId) && !s.ctls[p.ctlId], "E_STATE_CONFLICT");
  const k = p.cmd.kind;
  const r = s.round;
  const prevHold = r?.hold ?? null; // 下面會把 hold 清掉；執行端回失敗時退回這一份
  const w = tx.w;
  let cmd: RemoteCommand = { kind: k };
  let settings;
  if (k === "retry" || k === "switch_model" || k === "switch_route") {
    need(
      !s.undo && r?.phase === "held" && (k === "retry" || r.hold?.actions.includes(k)),
      "E_STATE_CONFLICT",
    ); // 收回在等執行端時不重試
    need(execReady(s), "E_EXECUTOR_OFFLINE"); // 重開補送還沒完：桌上與房間還沒對齊，不重跑
    if (k !== "retry") {
      settings = offered(
        s,
        k === "switch_route" ? p.cmd.route : s.sceneSettings!.route,
        p.cmd.model,
      );
      cmd =
        k === "switch_route"
          ? { kind: k, route: settings.route, model: settings.model }
          : { kind: k, model: settings.model };
    }
    Object.assign(w.round!, { phase: "generating", hold: null, genAt: tx.now });
  } else if (k === "abort") need(r?.phase === "generating", "E_STATE_CONFLICT");
  else if (k === "set_next") {
    w.nextSettings = offered(s, p.cmd.route, p.cmd.model);
    cmd = { kind: k, route: w.nextSettings.route, model: w.nextSettings.model };
    announceExec(tx);
  } else if (k === "pause")
    need(!Object.values(s.ctls).some((c) => c.cmd.kind === "pause"), "E_STATE_CONFLICT");
  else need(k === "refresh_quota", "E_BAD_FRAME");
  w.ctls[p.ctlId] = { conn: tx.conn, id: tx.reqId, at: tx.now, cmd, settings, hold: prevHold };
  tx.toExec("ctl", { ctlId: p.ctlId, by: m.id, cmd });
  return "later" as const;
}

/** 存草稿（不公開，別人只看到狀態） */
function setDraft(tx: Tx, m: Member, raw: unknown) {
  need(m.cardId, "E_SEAT_REQUIRED");
  need(tx.s.status === "playing" || tx.s.status === "transition", "E_STATE_CONFLICT");
  const text = cleanText(raw, 0);
  need(text !== null, "E_TEXT_INVALID");
  const s = tx.w;
  if (text) s.drafts[m.id] = text;
  else delete s.drafts[m.id];
  const e =
    s.round?.phase === "acting" ? s.round.roster.find((x) => x.memberId === m.id) : undefined;
  if (!e || e.status === "submitted" || e.status === "skipped") return;
  e.status = text ? "drafting" : "pending";
  tx.toAll("vol", {
    type: "roster_status",
    round: s.round!.round,
    memberId: m.id,
    status: e.status,
  });
}

/** 開或關一位的代管；狀態沒變回 E_STATE_CONFLICT（整筆交易不動） */
function setDelegated(tx: Tx, by: Member, memberId: string, on: boolean) {
  const t = tx.w.members.find((x) => x.id === memberId && !x.banned);
  need(t?.cardId, "E_SEAT_REQUIRED");
  need(!!t.delegated !== on, "E_STATE_CONFLICT");
  if (on) delegate(tx, t);
  else undelegate(tx, t);
  tx.emit("seat.delegated", { memberId: t.id, cardId: t.cardId, on, by: by.id });
}

export const MEMBER: Record<string, Handler> = {
  ping: (tx) => {
    tx.send(tx.conn, "pong", undefined);
    return "later" as const;
  },
  "profile.rename": (tx, p, m) => {
    const nickname = cleanName(p.nickname);
    need(nickname, "E_NAME_INVALID");
    m.nickname = nickname;
    tx.emit("member.renamed", { memberId: m.id, nickname });
  },
  "seat.claim": (tx, p, m) => claim(tx, m, p.cardId),
  // 交給 AI 代管／接回（room-seat-delegate §3.1）：本人或房主；座位、卡的認領、草稿都保留
  "seat.delegate": (tx, p, m) => {
    need(p.memberId === m.id || tx.s.hostId === m.id, "E_FORBIDDEN");
    setDelegated(tx, m, p.memberId, p.on);
  },
  // 房主一次代管好幾位（一鍵代管）：同一筆交易做完，結算留給交易尾端，已送出的人一律先退回草稿、不會先被結算進去
  "seat.delegate_many": (tx, p, m) => {
    host(tx, m);
    const ids: unknown[] = p.memberIds;
    need(
      ids.length > 0 &&
        ids.length <= 12 &&
        ids.every((x) => typeof x === "string") &&
        new Set(ids).size === ids.length,
      "E_BAD_FRAME",
    );
    for (const id of ids as string[]) setDelegated(tx, m, id, true);
  },
  "seat.release": (tx, _p, m) => {
    open(tx.s);
    need(m.cardId, "E_SEAT_REQUIRED");
    unseat(tx, m);
  },
  "seat.create": (tx, p, m) => {
    const s = tx.s;
    open(s);
    need(!m.cardId, "E_STATE_CONFLICT");
    need(s.settings.allowCustomCards, "E_FORBIDDEN");
    const name = cleanName(p.name, 40);
    const blurb = cleanText(p.blurb, 0, 1000);
    need(name, "E_NAME_INVALID");
    need(blurb !== null, "E_TEXT_INVALID");
    need(s.exec.online, "E_EXECUTOR_OFFLINE");
    const job = {
      jobId: ulid(tx.now, tx.ctx.rng),
      kind: "card_create" as const,
      memberId: m.id,
      name,
      blurb,
    };
    tx.w.jobs.push({ job, reply: { conn: tx.conn, id: tx.reqId } }); // 等執行端建好卡、替他入座後才回
    tx.toExec("job.card_create", job);
    return "later" as const;
  },
  "action.draft": (tx, p, m) => {
    rate(tx, `d:${m.id}`, 1, 500);
    setDraft(tx, m, p.text);
  },
  "action.submit": (tx, p, m) => {
    const e = myEntry(tx, m, p.round);
    need(e.status !== "submitted" && e.status !== "skipped", "E_STATE_CONFLICT");
    const text = cleanText(p.text, 1);
    need(text, "E_TEXT_INVALID");
    Object.assign(e, { status: "submitted", text });
    delete tx.w.drafts[m.id];
    tx.emit("action.submitted", { round: p.round, memberId: m.id, text }); // 送出即公開（裁決 9）
  },
  "action.skip": (tx, p, m) => {
    const e = myEntry(tx, m, p.round);
    need(e.status !== "submitted" && e.status !== "skipped", "E_STATE_CONFLICT");
    Object.assign(e, { status: "skipped", text: null });
    tx.emit("action.skipped", { round: p.round, memberId: m.id });
  },
  "action.retract": (tx, p, m) => {
    const e = myEntry(tx, m, p.round);
    need(e.status === "submitted" || e.status === "skipped", "E_STATE_CONFLICT");
    if (e.text) tx.w.drafts[m.id] = e.text;
    Object.assign(e, { status: tx.s.drafts[m.id] ? "drafting" : "pending", text: null });
    tx.emit("action.retracted", { round: p.round, memberId: m.id });
  },
  "round.resolve": (tx, p, m) => {
    host(tx, m);
    need(roundAt(tx, p.round).roster.length > 0, "E_STATE_CONFLICT");
    need(execReady(tx.s), "E_EXECUTOR_OFFLINE");
    resolve(tx, "host");
  },
  // 房主 GM 鈕（room-host-actions §3.1）：沒人入座也能按；這回合已送出的行動退回草稿，GM 跑完開新的 acting
  "host.gm": (tx, p, m) => {
    host(tx, m);
    const r = roundAt(tx, p.round);
    need(p.mode === "narrate" || p.mode === "advance" || p.mode === "call", "E_BAD_FRAME");
    need(execReady(tx.s), "E_EXECUTOR_OFFLINE");
    let turn: HostTurn = { mode: p.mode };
    if (p.mode === "call") {
      const card = tx.s.cards.find((c) => c.cardId === p.cardId);
      need(card && card.playable && !card.hidden && card.claimedBy === null, "E_SEAT_TAKEN");
      turn = { mode: "call", cardId: card.cardId, call: r.called === card.cardId }; // GM 剛點到他＝「讓 X 接話」，先寫點名列
    }
    cancelRound(tx, true); // 已送出的行動留著，GM 跑完開新的 acting 時放回（room-speak-request §3）
    openRound(tx, "host", turn);
  },
  // 房主催促一位還沒送出的玩家（room-speak-request §1）：只私下通知本人，不動回合、不進日誌；同一對象 30 秒一次
  "host.nudge": (tx, p, m) => {
    host(tx, m);
    const r = roundAt(tx, p.round);
    const e = r.roster.find((x) => x.memberId === p.memberId);
    need(
      e && (e.status === "pending" || e.status === "drafting") && isOnline(tx.s, e.memberId),
      "E_STATE_CONFLICT",
    );
    rate(tx, `n:${m.id}:${e.memberId}`, 1, 30_000);
    tx.toMember(e.memberId, "vol", { type: "nudged", round: r.round });
  },
  // 收回／重生上一回合（§3.2）：存 undo、請執行端把那批事件從桌尾收掉，等 job.result 才動回合
  "round.undo": (tx, p, m) => {
    host(tx, m);
    const s = tx.s;
    const r = s.round;
    const ld = s.lastDispatch;
    need(!s.undo && (p.mode === "regenerate" || p.mode === "edit"), "E_STATE_CONFLICT");
    const target =
      r &&
      ld &&
      ld.round === p.round &&
      ld.scene === s.scene &&
      ((r.phase === "acting" && r.round === ld.round + 1) ||
        (r.phase === "held" && r.round === ld.round));
    need(target, "E_STATE_CONFLICT");
    need(execReady(s), "E_EXECUTOR_OFFLINE");
    let next: SceneSettings | null = null;
    if (p.next !== undefined) {
      need(s.payerId === m.id, "E_FORBIDDEN"); // 只有出額度者能同時換路線／模型（同換幕）
      need(p.mode === "regenerate" && shape(p.next, "route:s model:s"), "E_BAD_FRAME");
      next = offered(s, p.next.route, p.next.model);
    }
    const job = {
      jobId: ulid(tx.now, tx.ctx.rng),
      kind: "undo" as const,
      dispatchId: ld!.dispatchId,
    };
    tx.w.undo = { jobId: job.jobId, round: ld!.round, mode: p.mode, next };
    tx.w.jobs.push({ job, reply: { conn: tx.conn, id: tx.reqId } });
    tx.toExec("job.undo", job);
    return "later" as const;
  },
  // 丟掉，或帶回（`draft`＝放進輸入框後的整段草稿）：草稿同一筆交易存好才清原文，存不進去（太長）原文照留
  "action.returned_dismiss": (tx, p, m) => {
    if (tx.s.returned?.[m.id] === undefined) return;
    if (p.draft !== undefined) setDraft(tx, m, p.draft);
    delete tx.w.returned![m.id];
    tx.toMember(m.id, "vol", { type: "returned", text: null });
  },
  "round.continue": (tx, p, m) => {
    hostOrPayer(tx, m);
    roundAt(tx, p.round, "held");
    tx.w.lastDispatch = null; // 放棄的回合不再是收回目標
    openRound(tx, "play");
  },
  "gen.abort": (tx, p, m) => {
    hostOrPayer(tx, m);
    const r = roundAt(tx, p.round, "generating");
    need(tx.s.exec.online, "E_EXECUTOR_OFFLINE");
    tx.toExec("abort", { dispatchId: r.dispatchId });
  },
  "room.start": (tx, _p, m) => {
    host(tx, m);
    const s = tx.s;
    need(s.status === "lobby" && activeMembers(s).length > 0 && s.exec.caps, "E_STATE_CONFLICT");
    need(execReady(s), "E_EXECUTOR_OFFLINE");
    const w = tx.w;
    Object.assign(w, {
      status: "playing",
      scene: 1,
      sceneSettings: w.nextSettings ?? s.exec.caps.current,
      nextSettings: null,
    });
    tx.emit("room.started", { scene: 1, settings: w.sceneSettings });
    openRound(tx, "open");
  },
  "room.settings": (tx, p, m) => {
    host(tx, m);
    open(tx.s);
    const next: Record<string, unknown> = { ...tx.s.settings };
    for (const [k, v] of Object.entries(p.patch)) {
      const int = (lo: number, hi: number) =>
        Number.isInteger(v) && (v as number) >= lo && (v as number) <= hi;
      const ok =
        k === "maxPlayers"
          ? int(2, 8)
          : k === "actionTimeoutSec"
            ? v === null || int(30, 3600)
            : (k === "shuffleOrder" || k === "allowCustomCards") && typeof v === "boolean";
      need(ok, "E_SETTING_INVALID", k);
      next[k] = v;
    }
    tx.w.settings = next as unknown as RoomState["settings"];
    tx.emit("room.settings_changed", { settings: next });
  },
  // 新密語要先存進房主電腦（執行端）才生效：電腦不在線就不給重生；伺服器只存雜湊，不存明文（協定外加 `invite`／`invite.ack`）
  "invite.regenerate": async (tx, _p, m) => {
    host(tx, m);
    need(tx.s.exec.online, "E_EXECUTOR_OFFLINE");
    const prev = tx.s.inviteRegen;
    if (prev) tx.err(prev.conn, "E_STATE_CONFLICT", { id: prev.id }); // 上一次還在等：作廢，以這次為準
    const joinSecret = secret(tx.ctx.rng, 16);
    tx.w.inviteRegen = {
      hash: await sha256Hex(`${tx.s.code}:${joinSecret}`),
      conn: tx.conn,
      id: tx.reqId,
      at: tx.now,
    };
    tx.toExec("invite", { joinSecret });
    return "later" as const; // 執行端 ack 之後才回房主
  },
  "host.transfer": (tx, p, m) => {
    host(tx, m);
    need(
      tx.s.members.some((x) => x.id === p.memberId && !x.banned),
      "E_STATE_CONFLICT",
    );
    tx.w.hostId = p.memberId;
    tx.emit("host.changed", { memberId: p.memberId });
  },
  "member.kick": (tx, p, m) => {
    host(tx, m);
    const s = tx.w;
    const t = s.members.find((x) => x.id === p.memberId && !x.banned);
    need(t && t.id !== m.id, "E_STATE_CONFLICT");
    need(p.mode === undefined || p.mode === "npc" || p.mode === "absent", "E_BAD_FRAME");
    const cardId = t.cardId;
    unseat(tx, t, p.mode); // 移除時一起決定角色處置（§3.2）：一則到位，不卡在中間狀態
    if (cardId && p.mode && s.status !== "lobby")
      tx.emit("seat.departure", { cardId, mode: p.mode });
    if (s.payerId === t.id) {
      s.payerId = null;
      tx.emit("pair.unbound", { memberId: t.id });
    }
    if (p.ban)
      t.banned = true; // 封鎖只記裝置雜湊
    else s.members = s.members.filter((x) => x !== t);
    delete s.drafts[t.id];
    if (s.returned) delete s.returned[t.id];
    tx.emit("member.kicked", { memberId: t.id, ban: p.ban });
    for (const [id, c] of Object.entries(s.conns))
      if (c.who === t.id) tx.shut(id, 4403, p.ban ? "E_BANNED" : "E_KICKED");
  },
  "member.departure": (tx, p, m) => {
    host(tx, m);
    need(tx.s.status === "playing", "E_STATE_CONFLICT");
    need(p.mode === "npc" || p.mode === "absent", "E_BAD_FRAME");
    need(
      tx.s.cards.some((c) => c.cardId === p.cardId && c.claimedBy === null),
      "E_STATE_CONFLICT",
    );
    const w = tx.w;
    w.departures = [
      ...w.departures.filter((d) => d.cardId !== p.cardId),
      { cardId: p.cardId, mode: p.mode },
    ];
    tx.emit("seat.departure", { cardId: p.cardId, mode: p.mode });
  },
  "member.reassign": (tx, p, m) => {
    host(tx, m);
    open(tx.s);
    const s = tx.w;
    const from = s.members.find((x) => x.id === p.from && x.cardId);
    const to = s.members.find((x) => x.id === p.to && !x.banned && !x.cardId);
    need(from && to, "E_STATE_CONFLICT");
    const wasDelegated = !!from.delegated;
    Object.assign(to, {
      cardId: from.cardId,
      seat: from.seat,
      arriving: from.arriving,
      delegated: wasDelegated,
    });
    Object.assign(from, { cardId: null, arriving: false, delegated: false });
    if (s.drafts[from.id] !== undefined) s.drafts[to.id] = s.drafts[from.id];
    delete s.drafts[from.id];
    if (s.returned?.[from.id] !== undefined) {
      s.returned[to.id] = s.returned[from.id];
      delete s.returned[from.id];
    }
    s.cards = s.cards.map((c) => (c.cardId === to.cardId ? { ...c, claimedBy: to.id } : c));
    for (const e of [...(s.round?.roster ?? []), ...(s.carry ?? [])])
      if (e.memberId === from.id) e.memberId = to.id;
    tx.emit("seat.reassigned", { from: from.id, to: to.id, cardId: to.cardId });
    // 代管中的座位有人接手＝人在玩：照接回處理（暫離沒派出就抵消，否則下一回合登場）
    if (wasDelegated) {
      undelegate(tx, to);
      tx.emit("seat.delegated", { memberId: to.id, cardId: to.cardId, on: false, by: m.id });
    }
  },
  "member.leave": (tx, _p, m) => {
    if (tx.s.status !== "closed") unseat(tx, m);
    tx.emit("member.left", { memberId: m.id });
    tx.ok();
    for (const [id, c] of Object.entries(tx.s.conns)) if (c.who === m.id) tx.shut(id, 1000);
    return "later" as const;
  },
  "scene.advance": (tx, p, m) => {
    host(tx, m);
    const s = tx.s;
    const r = s.round;
    need(s.status === "playing" && r, "E_STATE_CONFLICT");
    need(p.round === r.round, "E_ROUND_STALE");
    need(
      !s.undo && (r.phase === "acting" || (r.phase === "held" && r.hold?.kind === "scene_full")),
      "E_ROUND_NOT_ACTING",
    );
    need(execReady(s), "E_EXECUTOR_OFFLINE");
    let next = s.nextSettings ?? s.sceneSettings!;
    if (p.next !== undefined) {
      need(s.payerId === m.id, "E_FORBIDDEN"); // 執行端不換，只換路線與模型（裁決 8）
      need(shape(p.next, "route:s model:s"), "E_BAD_FRAME");
      next = offered(s, p.next.route, p.next.model);
    }
    const w = tx.w;
    cancelRound(tx);
    const job = {
      jobId: ulid(tx.now, tx.ctx.rng),
      kind: "scene_advance" as const,
      scene: s.scene,
      nextSettings: next,
    };
    Object.assign(w, {
      round: null,
      status: "transition",
      transition: { jobId: job.jobId, since: tx.now, progressAt: tx.now, stalled: false },
    });
    w.jobs.push({ job, reply: null });
    tx.emit("scene.advancing", { jobId: job.jobId });
    tx.toExec("job.scene_advance", job);
  },
  "scene.advance_cancel": (tx, _p, m) => {
    host(tx, m);
    need(tx.s.transition?.stalled, "E_STATE_CONFLICT");
    need(tx.s.exec.online, "E_EXECUTOR_OFFLINE");
    tx.toExec("job.cancel", { jobId: tx.s.transition.jobId }); // 房間仍等 job.result
  },
  "room.close": (tx, _p, m) => {
    host(tx, m);
    tx.ok();
    closeRoom(tx);
    return "later" as const;
  },
  "pair.claim": (tx, p, m) => pairClaim(tx, m, p.code),
  "pair.leave": (tx, _p, m) => {
    need(tx.s.payerId === m.id, "E_FORBIDDEN");
    tx.w.payerId = null;
    tx.emit("pair.unbound", { memberId: m.id });
  },
  ctl: (tx, p, m) => ctl(tx, m, p),
  "history.page": async (tx, p) => {
    need(p.limit >= 1 && p.limit <= 200, "E_BAD_FRAME");
    rate(tx, `h:${tx.conn}`, 2, 1000);
    const scanned = await tx.ctx.store.eventsBefore(p.beforeSeq, p.limit);
    // 只回劇情類；`before` 是這次掃到的最小序號，濾掉之後客戶端拿它接著往回翻
    tx.ok({
      events: scanned.filter((e) => STORY.has(e.type)).map((e) => ({ t: "ev", p: evFrame(e) })),
      before: scanned[0]?.seq ?? 0,
    });
    return "later" as const;
  },
};
