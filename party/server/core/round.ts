// 回合狀態機（協定 §3）：開回合、結算、派工、離座、倒數暫停與單一鬧鐘。
import { ulid } from "./secrets";
import { Fail, type Tx } from "./tx";
import type {
  Dispatch,
  ExecutorView,
  HostTurn,
  Member,
  PublicExecutorError,
  RoomState,
  RosterEntry,
  RoundKind,
  SceneSettings,
} from "./types";

export const MIN = 60_000;
export const DAY = 1440 * MIN;
export const GEN_IDLE_MS = 15 * MIN; // 生成與換幕無進度
export const HB_TIMEOUT_MS = 30_000;
export const CTL_TIMEOUT_MS = 30_000;
export const HELLO_MS = 10_000;

export const seatedMembers = (s: RoomState) =>
  s.members.filter((m) => m.cardId).sort((a, b) => a.seat - b.seat);
/** 入座而且不在 AI 代管中：行動名單、在場玩家、開始條件都只算這些人 */
export const activeMembers = (s: RoomState) => seatedMembers(s).filter((m) => !m.delegated);
export const cardName = (s: RoomState, cardId: string) =>
  s.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
export const isOnline = (s: RoomState, who: string) =>
  Object.values(s.conns).some((c) => c.who === who);
/** 執行端可以接新派工：在線而且不在同步閘門裡（重開房間、還沒補完關房期間的劇情，§3.3） */
export const execReady = (s: RoomState) => s.exec.online && s.exec.sync !== "needed";

export const execView = (s: RoomState): ExecutorView => ({
  label: s.exec.label,
  online: s.exec.online,
  since: s.exec.since,
  keepAwake: s.exec.keepAwake,
  paused: !s.exec.online && s.exec.paused === true,
  imageGen: s.exec.imageGen === true,
  imageGenRoute: s.exec.imageGenRoute !== false,
  syncing: s.exec.online && s.exec.sync === "needed",
  modelLabel: (s.nextSettings ?? s.sceneSettings ?? s.exec.caps?.current)?.modelLabel ?? null,
  // 幕號對不上（已換幕、執行端還沒重報）或舊版從不報：不當成到門檻
  capacityHint: !!s.exec.capacity && s.exec.capacity.scene === s.scene && s.exec.capacity.hint,
  diceEnabled: s.exec.diceEnabled === true,
});
/** 給某位成員看的執行端狀態：出額度者另帶路線與模型清單（遠端控制要選） */
export const execViewFor = (s: RoomState, who: string | null) => ({
  ...execView(s),
  ...(who !== null && who === s.payerId ? { routes: s.exec.caps?.routes ?? null } : {}),
});
export const announceExec = (tx: Tx) => {
  for (const [id, c] of Object.entries(tx.s.conns))
    if (c.who !== null) tx.send(id, "vol", { type: "executor", ...execViewFor(tx.s, c.who) });
};

/** 路線／模型要在執行端自報的清單裡（§7.3） */
export function offered(s: RoomState, route: unknown, model: unknown): SceneSettings {
  const found = s.exec.caps?.routes
    .find((r) => r.route === route)
    ?.models.find((m) => m.id === model);
  if (!found) throw new Fail("E_MODEL_NOT_OFFERED");
  return { route: route as string, model: found.id, modelLabel: found.label };
}

/** 已送出的原文還給本人（收回、保留的行動對不上座位）：不碰草稿，已有一筆沒處理的就接在後面 */
export function giveBack(tx: Tx, memberId: string, text: string | null) {
  if (!text || !tx.s.members.some((m) => m.id === memberId && !m.banned)) return;
  const w = tx.w;
  const prev = w.returned?.[memberId];
  const all = prev ? `${prev}\n${text}` : text;
  (w.returned ??= {})[memberId] = all;
  tx.toMember(memberId, "vol", { type: "returned", text: all });
}

/** host 回合在 round.opened 與快照裡帶的保留行動 */
export const carryOf = (s: RoomState, kind: RoundKind) =>
  kind === "host" && s.carry?.length ? { carry: s.carry } : {};

export function openRound(tx: Tx, kind: RoundKind, host?: HostTurn) {
  const s = tx.w;
  const arrivals: string[] = [];
  const roster =
    kind !== "play"
      ? []
      : activeMembers(s).map((m): RosterEntry => {
          if (m.arriving) arrivals.push(m.cardId!);
          m.arriving = false;
          return {
            memberId: m.id,
            cardId: m.cardId!,
            status: s.drafts[m.id] ? "drafting" : "pending",
            text: null,
          };
        });
  const ms =
    kind === "play" && s.settings.actionTimeoutSec !== null
      ? s.settings.actionTimeoutSec * 1000
      : null;
  // 房主 GM 鈕保留的行動：同一位、同一張卡還在名單上就原樣放回，其餘原文還給本人
  if (kind === "play" && s.carry) {
    for (const c of s.carry) {
      const e = roster.find((x) => x.memberId === c.memberId && x.cardId === c.cardId);
      if (e) Object.assign(e, { status: c.status, text: c.text });
      else giveBack(tx, c.memberId, c.text);
    }
    s.carry = null;
  }
  const run = ms !== null && roster.length > 0 && execReady(s);
  s.lastError = null; // 新回合，上一回合的錯誤細節作廢（客戶端同樣在 round.opened 清掉）
  s.round = {
    round: ++s.roundNo,
    kind,
    scene: s.scene,
    phase: kind === "play" ? "acting" : "queued",
    roster,
    deadline: run ? tx.now + ms : null,
    pausedRemainingMs: run ? null : ms,
    dispatchId: null,
    hold: null,
    batch: [],
    arrivals,
    dispatch: null,
    partsEnded: 0,
    speaker: null,
    genAt: tx.now,
    called: null,
    ...(host ? { host } : {}),
  };
  tx.emit("round.opened", {
    round: s.round.round,
    kind,
    roster,
    deadline: s.round.deadline,
    ...carryOf(s, kind),
  });
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
    // 倒數到期比照房主結算：沒送出的算觀望、草稿留著〔作者裁決 2026-11-03〕
    if (e.status === "submitted") return { ...base, kind: "act" as const, text: e.text! };
    return {
      ...base,
      kind: "watch" as const,
      watchReason:
        e.status === "skipped"
          ? ("skipped" as const)
          : by === "timeout"
            ? ("timeout" as const)
            : ("host_resolved" as const),
    };
  });
  Object.assign(r, { phase: "queued", deadline: null, pausedRemainingMs: null });
  tx.emit("round.resolved", { round: r.round, by, batch: r.batch });
  pushDispatch(tx);
}

/** 作廢當前回合：已送出的行動退回草稿（換幕、補送），`keep`＝改存進 carry 等 GM 跑完放回（房主 GM 鈕）；
 *  沒放回就作廢的 carry 原文還給本人。還沒派工的回合裡剛登場的人，下一次開 acting 再登場 */
export function cancelRound(tx: Tx, keep = false) {
  const w = tx.w;
  const r = w.round!;
  if (keep)
    w.carry = r.roster
      .filter((e) => e.status === "submitted" || e.status === "skipped")
      .map((e) => ({ ...e }));
  else {
    for (const e of r.roster) if (e.status === "submitted" && e.text) w.drafts[e.memberId] = e.text;
    for (const c of w.carry ?? []) giveBack(tx, c.memberId, c.text);
    w.carry = null;
  }
  if (!r.dispatch)
    for (const m of w.members) if (m.cardId && r.arrivals.includes(m.cardId)) m.arriving = true;
  tx.emit("round.cancelled", { round: r.round, ...(keep ? { kept: true } : {}) });
}

/** 同一回合只派一次：dispatchId 只在這裡產生一次，之後重送、重連、retry 都用同一份。
 *  `from`＝重生：players／arrivals／departures（之後的欄位也一樣）照原派工快照，s.departures 留給下一個正常派工 */
export function ensureDispatch(tx: Tx, from?: Dispatch): Dispatch {
  if (tx.s.round!.dispatch) return tx.s.round!.dispatch;
  const s = tx.w;
  const r = s.round!;
  const ids = {
    dispatchId: ulid(tx.now, tx.ctx.rng),
    round: r.round,
    kind: r.kind,
    scene: r.scene,
    settings: s.sceneSettings!,
    actions: r.batch,
    rolls: s.pendingRolls ?? [],
  };
  s.pendingRolls = [];
  if (from) r.dispatch = { ...from, ...ids };
  else {
    const players = activeMembers(s)
      .filter((m) => !m.arriving)
      .map((m) => ({
        cardId: m.cardId!,
        memberId: m.id,
        name: cardName(s, m.cardId!),
        connected: isOnline(s, m.id),
      }));
    // 只消耗這次真的登場的接回卡（GM 推進之類 arrivals 為空的回合不吃掉 returning）
    const returns = (s.returning ?? []).filter((c) => r.arrivals.includes(c));
    if (returns.length) s.returning = s.returning!.filter((c) => !returns.includes(c));
    r.dispatch = {
      ...ids,
      players,
      arrivals: r.arrivals,
      departures: s.departures,
      returns,
      ...(r.host ? { host: r.host } : {}),
    };
    s.departures = [];
  }
  Object.assign(r, { dispatchId: r.dispatch.dispatchId, genAt: tx.now });
  tx.emit("gen.dispatched", { round: r.round, dispatchId: r.dispatchId });
  return r.dispatch;
}
/** 重生：照原派工的種類與批次開新回合，直接 queued 派工（play 另發 round.resolved，玩家行動才回到故事串） */
export function reopen(tx: Tx, from: Dispatch) {
  const s = tx.w;
  s.lastError = null;
  s.round = {
    round: ++s.roundNo,
    kind: from.kind,
    scene: s.scene,
    phase: "queued",
    roster: [],
    deadline: null,
    pausedRemainingMs: null,
    dispatchId: null,
    hold: null,
    batch: from.actions,
    arrivals: from.arrivals,
    dispatch: null,
    partsEnded: 0,
    speaker: null,
    genAt: tx.now,
    called: null,
    ...(from.host ? { host: from.host } : {}),
  };
  tx.emit("round.opened", {
    round: s.round.round,
    kind: from.kind,
    roster: [],
    deadline: null,
    phase: "queued",
    ...carryOf(s, from.kind),
  });
  if (from.kind === "play")
    tx.emit("round.resolved", { round: s.round.round, by: "host", batch: from.actions });
  ensureDispatch(tx, from);
  pushDispatch(tx);
}

export function pushDispatch(tx: Tx) {
  if (execReady(tx.s) && tx.s.round?.phase === "queued") tx.toExec("dispatch", ensureDispatch(tx));
}

export function hold(tx: Tx, error: PublicExecutorError, code?: string) {
  const r = tx.w.round!;
  Object.assign(r, { phase: "held", hold: error });
  tx.ctx.live.stream = null;
  tx.emit("gen.failed", { round: r.round, error, ...(code ? { code } : {}) });
}

/** 離座（主動、被踢、名冊移除）：卡立刻從 roster 拿掉，房主沒決定前離席處置預設 absent（§3.4）。
 *  代管一併清掉；還沒派出的暫離項改寫成離席處置，不然卡會以「暫離」派出、之後卻沒人接回 */
export function unseat(tx: Tx, m: Member, mode?: "npc" | "absent") {
  const s = tx.w;
  const cardId = m.cardId;
  if (!cardId) return;
  Object.assign(m, { cardId: null, arriving: false, delegated: false });
  s.cards = s.cards.map((c) => (c.cardId === cardId ? { ...c, claimedBy: null } : c));
  if (s.round) s.round.roster = s.round.roster.filter((e) => e.memberId !== m.id);
  if (s.returning?.includes(cardId)) s.returning = s.returning.filter((c) => c !== cardId);
  if (s.status !== "lobby") {
    const i = s.departures.findIndex((d) => d.cardId === cardId);
    if (i < 0) s.departures.push({ cardId, mode: mode ?? "absent" });
    else if (mode || s.departures[i].mode === "delegate")
      s.departures[i] = { cardId, mode: mode ?? "absent" };
  }
  tx.emit("seat.released", { memberId: m.id, cardId });
}

/** 加進當前（acting）名單；`arrival`＝本回合算登場（名單空著停住時中途入座、從代管接回） */
export function enterRoster(tx: Tx, m: Member, arrival: boolean) {
  const r = tx.w.round!;
  const status = tx.s.drafts[m.id] ? "drafting" : "pending";
  r.roster.push({ memberId: m.id, cardId: m.cardId!, status, text: null });
  if (arrival) r.arrivals.push(m.cardId!);
  tx.toAll("vol", { type: "roster_status", round: r.round, memberId: m.id, status });
}

/** 交給 AI 代管（room-seat-delegate §3.1）：已送出的先退回草稿（action.retracted 讓手機即時拿回原文）、移出名單，
 *  再記暫離項。不在這裡結算：交易尾端統一 settle，派工才帶得到這次暫離 */
export function delegate(tx: Tx, m: Member) {
  const s = tx.w;
  const cardId = m.cardId!;
  m.delegated = true;
  if (s.status === "lobby") return void (m.arriving = true); // 還沒開演：開演時不算在場，接回時才登場
  const r = s.round;
  const e = r?.phase === "acting" ? r.roster.find((x) => x.memberId === m.id) : undefined;
  if (r && e) {
    if (e.status === "submitted" || e.status === "skipped") {
      if (e.text) s.drafts[m.id] = e.text;
      tx.emit("action.retracted", { round: r.round, memberId: m.id });
    }
    r.roster = r.roster.filter((x) => x !== e);
    // 名單空著時剛加進來、本回合才要登場的人：當作還沒登場
    if (r.arrivals.includes(cardId)) {
      r.arrivals = r.arrivals.filter((c) => c !== cardId);
      m.arriving = true;
    }
  }
  if (m.arriving) return; // 執行端從沒把他當在場玩家：不寫暫離
  s.departures = [...s.departures.filter((d) => d.cardId !== cardId), { cardId, mode: "delegate" }];
}

/** 從 AI 代管接回：還沒登場的只清旗標；暫離還沒派出就抵消（直接回名單、不寫登場）；否則照中途加入登場並記 returning */
export function undelegate(tx: Tx, m: Member) {
  const s = tx.w;
  const cardId = m.cardId!;
  m.delegated = false;
  if (s.status === "lobby") return void (m.arriving = false);
  if (m.arriving) return;
  const r = s.round;
  if (s.departures.some((d) => d.cardId === cardId && d.mode === "delegate")) {
    s.departures = s.departures.filter((d) => d.cardId !== cardId);
    if (r?.phase === "acting") enterRoster(tx, m, false);
    return;
  }
  s.returning = [...(s.returning ?? []).filter((c) => c !== cardId), cardId];
  if (r?.phase === "acting" && r.roster.length === 0) enterRoster(tx, m, true);
  else m.arriving = true;
}

/** 遠端控制 retry／切換先把 held 轉成 generating；逾時或執行端離線而它之後沒送過任何 gen.*（genAt 仍是發令那一刻）就退回原錯誤 */
function revertCtl(tx: Tx, c: RoomState["ctls"][string]) {
  const r = tx.s.round;
  if (c.hold && r?.phase === "generating" && r.genAt === c.at)
    Object.assign(tx.w.round!, { phase: "held", hold: c.hold });
}

export function setExecOnline(tx: Tx, online: boolean) {
  const s = tx.w;
  Object.assign(s.exec, { online, since: tx.now, lastSeen: tx.now });
  if (!online) s.execConn = null;
  if (!online) for (const c of Object.values(s.ctls)) revertCtl(tx, c);
  // 遠端暫停等回覆時執行端下線：它已報過暫停就算成功（關房流程送完 status 才斷線），否則回離線
  if (!online)
    for (const [ctlId, c] of Object.entries(s.ctls)) {
      if (c.cmd.kind !== "pause") continue;
      if (s.exec.paused) tx.send(c.conn, "ok", { id: c.id, result: { ok: true } });
      else tx.err(c.conn, "E_EXECUTOR_OFFLINE", { id: c.id });
      delete s.ctls[ctlId];
    }
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
  const run = r.roster.length > 0 && execReady(s) && !s.undo; // 收回在等執行端：倒數暫停、不結算
  if (run ? r.pausedRemainingMs !== null : r.deadline !== null) {
    const w = tx.w.round!;
    if (run) Object.assign(w, { deadline: tx.now + w.pausedRemainingMs!, pausedRemainingMs: null });
    else Object.assign(w, { pausedRemainingMs: Math.max(0, w.deadline! - tx.now), deadline: null });
    tx.emit("round.deadline", {
      round: w.round,
      deadline: w.deadline,
      pausedRemainingMs: w.pausedRemainingMs,
    });
  }
  if (run && r.roster.every((e) => e.status === "submitted" || e.status === "skipped"))
    resolve(tx, "all");
}

const idleAt = (s: RoomState) =>
  s.closedAt !== null ? s.closedAt + 7 * DAY : s.lastActive + s.idleDays * DAY;
const generating = (s: RoomState) =>
  s.exec.online &&
  (s.round?.phase === "generating" || (s.round?.phase === "queued" && s.round.dispatch !== null));

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
  for (const [id, c] of Object.entries(tx.s.conns))
    if (c.who === null && now >= c.at + HELLO_MS) tx.shut(id, 4401, "E_AUTH_REQUIRED");
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
  if (due && s.status === "playing" && execReady(s) && !s.undo && r.roster.length > 0)
    resolve(tx, "timeout");
  else if (generating(tx.s) && now >= r!.genAt + GEN_IDLE_MS)
    hold(
      tx,
      { kind: "provider_unreachable", actions: ["retry", "switch_route"], resetsAt: null },
      "E_GEN_TIMEOUT",
    );
  const t = tx.s.transition;
  if (t && !t.stalled && now >= t.progressAt + GEN_IDLE_MS) {
    tx.w.transition!.stalled = true;
    tx.emit("scene.stalled", { jobId: t.jobId });
  }
}
