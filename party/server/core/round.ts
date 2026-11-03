// 回合狀態機（協定 §3）：開回合、結算、派工、離座、倒數暫停與單一鬧鐘。
import { ulid } from "./secrets";
import { Fail, type Tx } from "./tx";
import type { Dispatch, ExecutorView, Member, PublicExecutorError, RoomState, RosterEntry, RoundKind, SceneSettings } from "./types";

export const MIN = 60_000;
export const DAY = 1440 * MIN;
export const GEN_IDLE_MS = 15 * MIN; // 生成與換幕無進度
export const HB_TIMEOUT_MS = 30_000;
export const CTL_TIMEOUT_MS = 30_000;
export const HELLO_MS = 10_000;

export const seatedMembers = (s: RoomState) => s.members.filter((m) => m.cardId).sort((a, b) => a.seat - b.seat);
export const cardName = (s: RoomState, cardId: string) => s.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
export const isOnline = (s: RoomState, who: string) => Object.values(s.conns).some((c) => c.who === who);

export const execView = (s: RoomState): ExecutorView => ({
  label: s.exec.label, online: s.exec.online, since: s.exec.since, keepAwake: s.exec.keepAwake, paused: !s.exec.online && s.exec.paused === true,
  modelLabel: (s.nextSettings ?? s.sceneSettings ?? s.exec.caps?.current)?.modelLabel ?? null,
});
/** 給某位成員看的執行端狀態：出額度者另帶路線與模型清單（遠端控制要選） */
export const execViewFor = (s: RoomState, who: string | null) => ({ ...execView(s), ...(who !== null && who === s.payerId ? { routes: s.exec.caps?.routes ?? null } : {}) });
export const announceExec = (tx: Tx) => {
  for (const [id, c] of Object.entries(tx.s.conns)) if (c.who !== null) tx.send(id, "vol", { type: "executor", ...execViewFor(tx.s, c.who) });
};

/** 路線／模型要在執行端自報的清單裡（§7.3） */
export function offered(s: RoomState, route: unknown, model: unknown): SceneSettings {
  const found = s.exec.caps?.routes.find((r) => r.route === route)?.models.find((m) => m.id === model);
  if (!found) throw new Fail("E_MODEL_NOT_OFFERED");
  return { route: route as string, model: found.id, modelLabel: found.label };
}

export function openRound(tx: Tx, kind: RoundKind) {
  const s = tx.w;
  const arrivals: string[] = [];
  const roster = kind !== "play" ? [] : seatedMembers(s).map((m): RosterEntry => {
    if (m.arriving) arrivals.push(m.cardId!);
    m.arriving = false;
    return { memberId: m.id, cardId: m.cardId!, status: s.drafts[m.id] ? "drafting" : "pending", text: null };
  });
  const ms = kind === "play" && s.settings.actionTimeoutSec !== null ? s.settings.actionTimeoutSec * 1000 : null;
  const run = ms !== null && roster.length > 0 && s.exec.online;
  s.lastError = null; // 新回合，上一回合的錯誤細節作廢（客戶端同樣在 round.opened 清掉）
  s.round = {
    round: ++s.roundNo, kind, scene: s.scene, phase: kind === "play" ? "acting" : "queued", roster,
    deadline: run ? tx.now + ms : null, pausedRemainingMs: run ? null : ms, dispatchId: null, hold: null,
    batch: [], arrivals, dispatch: null, partsEnded: 0, speaker: null, genAt: tx.now,
  };
  tx.emit("round.opened", { round: s.round.round, kind, roster, deadline: s.round.deadline });
  if (kind !== "play") pushDispatch(tx);
}

/** 結算（§3.2）：組批次 → queued → 執行端在線即派工。呼叫端已確認只會在 acting 呼叫一次 */
export function resolve(tx: Tx, by: "all" | "host" | "timeout") {
  const s = tx.w;
  const r = s.round!;
  const order = [...r.roster];
  if (s.settings.shuffleOrder) {
    const rnd = tx.ctx.rng(order.length);
    for (let i = order.length - 1; i > 0; i--) {
      const j = rnd[i] % (i + 1);
      [order[i], order[j]] = [order[j], order[i]];
    }
  }
  r.batch = order.map((e) => {
    const base = { memberId: e.memberId, cardId: e.cardId, name: cardName(s, e.cardId) };
    const draft = by === "timeout" && e.status !== "skipped" && e.status !== "submitted" ? s.drafts[e.memberId] : undefined;
    if (draft) delete s.drafts[e.memberId];
    if (e.status === "submitted" || draft) return { ...base, kind: "act" as const, text: e.text ?? draft };
    return { ...base, kind: "watch" as const, watchReason: e.status === "skipped" ? ("skipped" as const) : by === "timeout" ? ("timeout" as const) : ("host_resolved" as const) };
  });
  Object.assign(r, { phase: "queued", deadline: null, pausedRemainingMs: null });
  tx.emit("round.resolved", { round: r.round, by, batch: r.batch });
  pushDispatch(tx);
}

/** 同一回合只派一次：dispatchId 只在這裡產生一次，之後重送、重連、retry 都用同一份 */
export function ensureDispatch(tx: Tx): Dispatch {
  if (tx.s.round!.dispatch) return tx.s.round!.dispatch;
  const s = tx.w;
  const r = s.round!;
  const players = seatedMembers(s).filter((m) => !m.arriving).map((m) => ({ cardId: m.cardId!, memberId: m.id, name: cardName(s, m.cardId!), connected: isOnline(s, m.id) }));
  r.dispatch = { dispatchId: ulid(tx.now, tx.ctx.rng), round: r.round, kind: r.kind, scene: r.scene, settings: s.sceneSettings!, actions: r.batch, players, arrivals: r.arrivals, departures: s.departures };
  Object.assign(r, { dispatchId: r.dispatch.dispatchId, genAt: tx.now });
  s.departures = [];
  tx.emit("gen.dispatched", { round: r.round, dispatchId: r.dispatchId });
  return r.dispatch;
}
export function pushDispatch(tx: Tx) {
  if (tx.s.exec.online && tx.s.round?.phase === "queued") tx.toExec("dispatch", ensureDispatch(tx));
}

export function hold(tx: Tx, error: PublicExecutorError, code?: string) {
  const r = tx.w.round!;
  Object.assign(r, { phase: "held", hold: error });
  tx.ctx.live.stream = null;
  tx.emit("gen.failed", { round: r.round, error, ...(code ? { code } : {}) });
}

/** 離座（主動、被踢、名冊移除）：卡立刻從 roster 拿掉，房主沒決定前離席處置預設 absent（§3.4） */
export function unseat(tx: Tx, m: Member) {
  const s = tx.w;
  const cardId = m.cardId;
  if (!cardId) return;
  Object.assign(m, { cardId: null, arriving: false });
  s.cards = s.cards.map((c) => (c.cardId === cardId ? { ...c, claimedBy: null } : c));
  if (s.round) s.round.roster = s.round.roster.filter((e) => e.memberId !== m.id);
  if (s.status !== "lobby" && !s.departures.some((d) => d.cardId === cardId)) s.departures.push({ cardId, mode: "absent" });
  tx.emit("seat.released", { memberId: m.id, cardId });
}

/** 遠端控制 retry／切換先把 held 轉成 generating；逾時或執行端離線而它之後沒送過任何 gen.*（genAt 仍是發令那一刻）就退回原錯誤 */
function revertCtl(tx: Tx, c: RoomState["ctls"][string]) {
  const r = tx.s.round;
  if (c.hold && r?.phase === "generating" && r.genAt === c.at) Object.assign(tx.w.round!, { phase: "held", hold: c.hold });
}

export function setExecOnline(tx: Tx, online: boolean) {
  const s = tx.w;
  Object.assign(s.exec, { online, since: tx.now, lastSeen: tx.now });
  if (!online) s.execConn = null;
  if (!online) for (const c of Object.values(s.ctls)) revertCtl(tx, c);
  if (!online && s.inviteRegen) {
    tx.err(s.inviteRegen.conn, "E_EXECUTOR_OFFLINE", { id: s.inviteRegen.id }); // 舊連結照常有效
    s.inviteRegen = null;
  }
  announceExec(tx);
}

/** 每次輸入的結尾：倒數照「名單非空且執行端在線」暫停或續跑，條件 1（全員已行動或跳過）成立就結算 */
export function settle(tx: Tx) {
  const s = tx.s;
  const r = s.round;
  if (s.status !== "playing" || r?.phase !== "acting") return;
  const run = r.roster.length > 0 && s.exec.online;
  if (run ? r.pausedRemainingMs !== null : r.deadline !== null) {
    const w = tx.w.round!;
    if (run) Object.assign(w, { deadline: tx.now + w.pausedRemainingMs!, pausedRemainingMs: null });
    else Object.assign(w, { pausedRemainingMs: Math.max(0, w.deadline! - tx.now), deadline: null });
    tx.emit("round.deadline", { round: w.round, deadline: w.deadline, pausedRemainingMs: w.pausedRemainingMs });
  }
  if (run && r.roster.every((e) => e.status === "submitted" || e.status === "skipped")) resolve(tx, "all");
}

const idleAt = (s: RoomState) => (s.closedAt !== null ? s.closedAt + 7 * DAY : s.lastActive + s.idleDays * DAY);
const generating = (s: RoomState) => s.exec.online && (s.round?.phase === "generating" || (s.round?.phase === "queued" && s.round.dispatch !== null));

/** 單一鬧鐘：取所有計時器最早者（§12.1） */
export function nextAlarm(s: RoomState | null): number | null {
  if (!s) return null;
  const at = [idleAt(s), ...Object.values(s.ctls).map((c) => c.at + CTL_TIMEOUT_MS)];
  if (s.inviteRegen) at.push(s.inviteRegen.at + CTL_TIMEOUT_MS);
  for (const c of Object.values(s.conns)) if (c.who === null) at.push(c.at + HELLO_MS);
  if (s.exec.online) at.push(s.exec.lastSeen + HB_TIMEOUT_MS);
  if (s.round?.deadline != null) at.push(s.round.deadline);
  if (generating(s)) at.push(s.round!.genAt + GEN_IDLE_MS);
  if (s.transition && !s.transition.stalled) at.push(s.transition.progressAt + GEN_IDLE_MS);
  return Math.min(...at);
}

export function onAlarm(tx: Tx) {
  const now = tx.now;
  if (now >= idleAt(tx.s)) {
    for (const id of Object.keys(tx.s.conns)) tx.shut(id, 4410);
    tx.deleteRoom = true;
    return;
  }
  for (const [id, c] of Object.entries(tx.s.conns)) if (c.who === null && now >= c.at + HELLO_MS) tx.shut(id, 4401, "E_AUTH_REQUIRED");
  for (const [ctlId, c] of Object.entries(tx.s.ctls)) {
    if (now < c.at + CTL_TIMEOUT_MS) continue;
    tx.err(c.conn, "E_EXECUTOR_TIMEOUT", { id: c.id });
    revertCtl(tx, c);
    delete tx.w.ctls[ctlId];
  }
  const regen = tx.s.inviteRegen;
  if (regen && now >= regen.at + CTL_TIMEOUT_MS) {
    tx.err(regen.conn, "E_EXECUTOR_TIMEOUT", { id: regen.id }); // 電腦沒存成：不換，舊連結照常有效
    tx.w.inviteRegen = null;
  }
  const execConn = tx.s.execConn;
  if (tx.s.exec.online && now >= tx.s.exec.lastSeen + HB_TIMEOUT_MS) {
    setExecOnline(tx, false);
    if (execConn) tx.shut(execConn, 1001);
  }
  // 心跳逾時已在上面先處理：執行端剛離線的這一刻不結算；關房後也不結算（草稿不得因此公開）
  const s = tx.s;
  const r = s.round;
  const due = r?.phase === "acting" && r.deadline !== null && now >= r.deadline;
  if (due && s.status === "playing" && s.exec.online && r.roster.length > 0) resolve(tx, "timeout");
  else if (generating(tx.s) && now >= r!.genAt + GEN_IDLE_MS) hold(tx, { kind: "provider_unreachable", actions: ["retry", "switch_route"], resetsAt: null }, "E_GEN_TIMEOUT");
  const t = tx.s.transition;
  if (t && !t.stalled && now >= t.progressAt + GEN_IDLE_MS) {
    tx.w.transition!.stalled = true;
    tx.emit("scene.stalled", { jobId: t.jobId });
  }
}
