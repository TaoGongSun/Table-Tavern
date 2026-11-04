// 執行端 → 伺服器訊息（協定 §6.2）：派工結果以 dispatchId／(dispatchId, part)／jobId 冪等收下。
import { uploadResult } from "./cards";
import { claim, type Handler } from "./member";
import { announceExec, cancelRound, hold, MIN, openRound, reopen, unseat } from "./round";
import { crockford, sameHash, sha256Hex } from "./secrets";
import { Fail, need, type Tx } from "./tx";
import type { ErrorCode, Job, PendingJob, PublicCard, PublicExecutorError } from "./types";
import { cleanCapabilities, cleanCards, cleanError, cleanHistory, cleanQuota, cleanSpeaker, ID64 } from "./validate";

const OTHER: PublicExecutorError = { kind: "other", actions: ["retry", "continue"], resetsAt: null };
const pub = (e: { kind: PublicExecutorError["kind"]; actions: PublicExecutorError["actions"]; resetsAt: number | null } | null): PublicExecutorError =>
  e ? { kind: e.kind, actions: e.actions, resetsAt: e.resetsAt } : OTHER;

/** 當前派工、而且還在跑（queued 或 generating）才收；收到即視為已接手（queued → generating） */
function live(tx: Tx, dispatchId: string) {
  const r = tx.s.round;
  need(r && r.dispatchId === dispatchId && (r.phase === "queued" || r.phase === "generating"), "E_DISPATCH_UNKNOWN");
  const w = tx.w.round!;
  Object.assign(w, { phase: "generating", genAt: tx.now });
  return w;
}

function replaceCards(tx: Tx, incoming: PublicCard[]) {
  // 協定外加的上限：名冊整份 ≤ 1 MiB（64 張×64 KiB 頭像可達 4 MiB），roster.updated 事件與 cards 列才放得進 DO 單列 2 MB
  need(JSON.stringify(incoming).length <= 1 << 20, "E_STATE_CONFLICT", "roster_too_large"); // 不關連線，免得執行端重連再送又被關
  const w = tx.w;
  const owner = new Map(w.cards.map((c) => [c.cardId, c.claimedBy]));
  w.cards = incoming.map((c) => ({ ...c, claimedBy: owner.get(c.cardId) ?? null, ...(w.reserved?.cardId === c.cardId ? { reserved: true } : {}) }));
  for (const m of w.members) if (m.cardId && !w.cards.some((c) => c.cardId === m.cardId && c.playable)) unseat(tx, m);
  tx.emit("roster.updated", { cards: w.cards });
}

function sceneResult(tx: Tx, job: Extract<Job, { kind: "scene_advance" }>, p: any) {
  const w = tx.w;
  Object.assign(w, { transition: null, status: "playing" });
  if (!p.ok) {
    tx.emit("scene.failed", { error: pub(cleanError(p.error)) });
    return openRound(tx, "play"); // 同一幕重開，房主可再試
  }
  const summary = p.result?.summary;
  need(typeof summary === "string" && summary.length <= 64000, "E_TEXT_INVALID");
  tx.emit("scene.summary", { scene: w.scene, text: summary, round: w.roundNo });
  w.summarySeqs.push(w.seq);
  w.lastDispatch = null; // 上一幕不是收回目標
  Object.assign(w, { scene: w.scene + 1, sceneSettings: job.nextSettings, nextSettings: null });
  tx.emit("scene.started", { scene: w.scene, settings: job.nextSettings });
  w.sceneStartSeq = w.seq;
  announceExec(tx);
  openRound(tx, "scene_open");
}

function cardResult(tx: Tx, j: PendingJob, p: any) {
  const job = j.job as Extract<Job, { kind: "card_create" }>;
  const m = tx.w.members.find((x) => x.id === job.memberId);
  const cardId = p.result?.cardId;
  let code: ErrorCode | null = p.ok && m && typeof cardId === "string" ? null : "E_STATE_CONFLICT";
  try {
    if (!code) claim(tx, m!, cardId);
  } catch (e) {
    if (!(e instanceof Fail)) throw e;
    code = e.code;
  }
  if (!j.reply) return;
  if (code) tx.err(j.reply.conn, code, { id: j.reply.id });
  else tx.send(j.reply.conn, "ok", { id: j.reply.id, result: { cardId } });
}

/** 收回的結果（§3.2）：執行端收乾淨才動回合；失敗（那批已不在桌尾）狀態不動、回房主 E_STATE_CONFLICT */
function undoResult(tx: Tx, j: PendingJob, p: any) {
  const w = tx.w;
  const u = w.undo;
  const ld = w.lastDispatch;
  w.undo = null;
  const ok = p.ok && u && ld && j.job.kind === "undo" && ld.dispatchId === j.job.dispatchId;
  if (j.reply) {
    if (ok) tx.send(j.reply.conn, "ok", { id: j.reply.id });
    else tx.err(j.reply.conn, "E_STATE_CONFLICT", { id: j.reply.id });
  }
  if (!ok) return;
  if (w.round?.phase === "acting") cancelRound(tx); // 收回後開出的那回合，已送出的退回草稿
  w.lastDispatch = null; // 只收一層
  tx.emit("round.undone", { round: ld.round });
  if (u.mode === "regenerate") {
    if (u.next) {
      w.sceneSettings = u.next; // 幕中切換之二：重生時換路線／模型，這一幕剩下的都用新的
      tx.emit("scene.settings_switched", { settings: u.next, reason: "regenerate" });
      announceExec(tx);
    }
    return reopen(tx, ld);
  }
  // 原文一律另存給本人，草稿不動；登場與離席放回待帶清單
  for (const a of ld.actions) {
    if (a.kind !== "act" || !a.text || !w.members.some((m) => m.id === a.memberId && !m.banned)) continue;
    (w.returned ??= {})[a.memberId] = a.text;
    tx.toMember(a.memberId, "vol", { type: "returned", text: a.text });
  }
  for (const m of w.members) if (m.cardId && ld.arrivals.includes(m.cardId)) m.arriving = true;
  w.departures = [...ld.departures.filter((d) => !w.departures.some((x) => x.cardId === d.cardId)), ...w.departures];
  openRound(tx, "play");
}

/** 工作：當前的照收；已收過的 jobId 回 ok 不重做（冪等）；其餘 E_DISPATCH_UNKNOWN */
function findJob(tx: Tx, jobId: string): PendingJob | null {
  const j = tx.s.jobs.find((x) => x.job.jobId === jobId);
  need(j || tx.s.doneJobs.includes(jobId), "E_DISPATCH_UNKNOWN");
  return j ?? null;
}
function progress(tx: Tx, p: { jobId: string }) {
  if (findJob(tx, p.jobId)?.job.kind === "scene_advance") Object.assign(tx.w.transition!, { progressAt: tx.now, stalled: false });
}

export const EXECUTOR: Record<string, (tx: Tx, p: any) => ReturnType<Handler>> = {
  hb: () => {}, // lastSeen 在入口處更新
  capabilities: (tx, p) => {
    const caps = cleanCapabilities(p);
    need(caps, "E_BAD_FRAME");
    tx.w.exec.caps = caps;
    announceExec(tx);
  },
  "room.seed": (tx, p) => {
    need(tx.s.status === "lobby" && !tx.s.seeded, "E_STATE_CONFLICT");
    const l = tx.ctx.live;
    if (p.part === 0) l.seed = { next: 0, cards: null, history: [] };
    // 分片緩衝在記憶體；DO 中途被逐出就從第 0 片重送
    need(l.seed && l.seed.next === p.part && p.part < p.total, "E_STATE_CONFLICT", "seed_restart");
    if (p.cards !== undefined) l.seed.cards = [...(l.seed.cards ?? []), ...(cleanCards(p.cards) ?? Fail.throw("E_BAD_FRAME"))];
    if (p.history !== undefined) l.seed.history.push(...(cleanHistory(p.history, l.seed.cards ?? tx.s.cards) ?? Fail.throw("E_BAD_FRAME")));
    if (++l.seed.next < p.total) return;
    const { cards, history } = l.seed;
    l.seed = null;
    tx.w.seeded = true;
    if (cards) replaceCards(tx, cleanCards(cards) ?? Fail.throw("E_BAD_FRAME"));
    history.forEach((h, part) => tx.emit("story.part", { ...h, part }));
  },
  "roster.update": (tx, p) => replaceCards(tx, cleanCards(p.cards) ?? Fail.throw("E_BAD_FRAME")),
  // 執行端存好了重生的邀請密語：這時才換掉入場雜湊並回房主（舊連結到此作廢）；不是等著的那份就不理
  "invite.ack": async (tx, p) => {
    const r = tx.s.inviteRegen;
    if (!r || !sameHash(await sha256Hex(`${tx.s.code}:${p.joinSecret}`), r.hash)) return;
    const w = tx.w;
    Object.assign(w, { joinHash: r.hash, inviteRegen: null });
    tx.emit("invite.regenerated");
    tx.send(r.conn, "ok", { id: r.id, result: { joinSecret: p.joinSecret } }); // 只回給房主，不廣播
  },
  // 電腦自己的玩家卡（桌面版）保留給電腦那台裝置：朋友不能先認走〔作者裁決 2026-11-03〕
  "seat.reserve": async (tx, p) => {
    need(p.cardId === null || ID64.test(p.cardId), "E_BAD_FRAME");
    need(/^[A-Za-z0-9_-]{22}$/.test(p.deviceId), "E_BAD_FRAME");
    const w = tx.w;
    w.reserved = p.cardId === null ? null : { cardId: p.cardId, dev: await sha256Hex(`${w.code}:${p.deviceId}`) };
    w.cards = w.cards.map(({ reserved: _, ...c }) => (c.cardId === p.cardId ? { ...c, reserved: true } : c));
    tx.emit("roster.updated", { cards: w.cards });
  },
  "dispatch.ack": (tx, p) => void live(tx, p.dispatchId),
  "gen.part.begin": (tx, p) => {
    const r = live(tx, p.dispatchId);
    need(p.part === r.partsEnded && p.part <= 31, "E_STATE_CONFLICT");
    r.speaker = cleanSpeaker(p.speaker, tx.s.cards) ?? Fail.throw("E_BAD_FRAME");
    tx.ctx.live.stream = { round: r.round, part: p.part, speaker: r.speaker, text: "", announced: false };
  },
  "gen.delta": (tx, p) => {
    const r = tx.s.round;
    need(r && r.dispatchId === p.dispatchId && r.phase === "generating", "E_DISPATCH_UNKNOWN");
    if (tx.now - r.genAt > MIN) tx.w.round!.genAt = tx.now; // 增量不落地；只有久久一次刷新無進度計時
    const st = tx.ctx.live.stream;
    if (!st || st.round !== r.round || st.part !== p.part) return; // DO 被逐出後緩衝已失，只等 part.end
    st.text += p.text;
    const first = !st.announced;
    st.announced = true;
    for (const [id, c] of Object.entries(tx.s.conns)) {
      if (c.who !== null && c.who !== "exec") tx.send(id, "vol", { type: "delta", round: r.round, part: p.part, ...(first ? { speaker: st.speaker } : {}), text: p.text });
    }
  },
  "gen.part.end": (tx, p) => {
    const s = tx.s;
    if (p.dispatchId === s.lastDoneDispatch || (s.round?.dispatchId === p.dispatchId && p.part < s.round!.partsEnded)) return; // 重送
    const r = live(tx, p.dispatchId);
    need(p.part === r.partsEnded && r.speaker, "E_STATE_CONFLICT");
    need(p.text.length <= 64000, "E_TEXT_INVALID");
    tx.emit("story.part", { round: r.round, part: p.part, speaker: r.speaker, text: p.text, interrupted: p.interrupted, at: tx.now });
    Object.assign(r, { partsEnded: r.partsEnded + 1, speaker: null });
    tx.ctx.live.stream = null;
  },
  "gen.done": (tx, p) => {
    if (p.dispatchId === tx.s.lastDoneDispatch) return;
    const r = live(tx, p.dispatchId);
    const w = tx.w;
    w.lastDoneDispatch = p.dispatchId;
    tx.ctx.live.stream = null;
    const back = (p.cardsArrived ?? []).filter((id: unknown) => typeof id === "string");
    if (back.length) {
      w.cards = w.cards.map((c) => (back.includes(c.cardId) ? { ...c, hidden: false } : c));
      tx.emit("roster.updated", { cards: w.cards });
    }
    w.lastDispatch = r.dispatch;
    tx.emit("gen.done", { round: r.round, aborted: p.aborted });
    // 房主「GM 旁白」點到的 NPC：只信還能被請發言的卡，記在下一個 acting、只告訴房主（「讓 X 接話」）
    const called = r.host?.mode === "narrate" && w.cards.some((c) => c.cardId === p.called && c.playable && !c.hidden && c.claimedBy === null) ? (p.called as string) : null;
    openRound(tx, "play");
    if (called) {
      w.round!.called = called;
      tx.toMember(w.hostId, "vol", { type: "called", round: w.round!.round, cardId: called });
    }
  },
  "gen.failed": (tx, p) => {
    const s = tx.s;
    if (s.round?.phase === "held" && s.round.dispatchId === p.dispatchId) return; // 重送
    const r = live(tx, p.dispatchId);
    const error = cleanError(p.error) ?? Fail.throw("E_BAD_FRAME");
    hold(tx, pub(error));
    tx.w.lastDispatch = r.dispatch;
    tx.w.lastError = { round: r.round, error }; // 快照與重連時帶給出額度者
    tx.toMember(s.payerId, "vol", { type: "error_detail", round: r.round, error }); // 細節只給出額度者
  },
  "job.ack": progress,
  "job.progress": progress,
  "job.result": (tx, p) => {
    const j = findJob(tx, p.jobId);
    if (!j) return;
    const w = tx.w;
    w.jobs = w.jobs.filter((x) => x.job.jobId !== p.jobId);
    w.doneJobs = [...w.doneJobs.slice(-19), p.jobId];
    if (j.job.kind === "scene_advance") sceneResult(tx, j.job, p);
    else if (j.job.kind === "card_create") cardResult(tx, j, p);
    else if (j.job.kind === "undo") undoResult(tx, j, p);
    else uploadResult(tx, j.job, p);
  },
  "pair.create": async (tx, p) => {
    need(tx.s.status !== "closed", "E_ROOM_CLOSED");
    const pairCode = crockford(tx.ctx.rng(8), 8);
    const pair = { hash: await sha256Hex(`${tx.s.code}:${pairCode}`), expiresAt: tx.now + 10 * MIN, grantHost: p.grantHost };
    tx.w.pair = pair; // 再發新碼＝舊碼作廢
    tx.ok({ pairCode, expiresAt: pair.expiresAt });
    return "later" as const;
  },
  "pair.revoke": (tx) => {
    const payer = tx.s.payerId;
    need(payer, "E_STATE_CONFLICT");
    tx.w.payerId = null;
    tx.emit("pair.unbound", { memberId: payer });
  },
  status: (tx, p) => {
    Object.assign(tx.w.exec, { keepAwake: p.keepAwake, busy: p.busy, paused: p.paused === true, imageGen: p.imageGen === true, imageGenRoute: p.imageGenRoute !== false });
    announceExec(tx);
  },
  quota: (tx, p) => {
    const q = cleanQuota(p) ?? Fail.throw("E_BAD_FRAME");
    tx.w.exec.quota = q;
    tx.toAll("vol", { type: "quota", coarse: { state: q.state, resetsAt: q.resetsAt } });
    tx.toMember(tx.s.payerId, "vol", { type: "quota_detail", report: q });
  },
  "ctl.result": (tx, p) => {
    const c = tx.s.ctls[p.ctlId];
    need(c, "E_DISPATCH_UNKNOWN");
    const w = tx.w;
    delete w.ctls[p.ctlId];
    if (p.ok && c.settings) {
      w.sceneSettings = c.settings; // 幕中切換之一（§7.3；另一種是重生），執行端切完自己接著重送
      tx.emit("scene.settings_switched", { settings: c.settings, reason: c.hold?.kind ?? "other" });
    }
    if (!p.ok && c.hold && w.round?.phase === "generating") Object.assign(w.round, { phase: "held", hold: c.hold });
    const error = p.ok ? undefined : cleanError(p.error) ?? undefined;
    tx.send(c.conn, "ok", { id: c.id, result: { ok: p.ok, ...(error ? { error } : {}) } });
  },
  "executor.leave": () => Fail.throw("E_STATE_CONFLICT"), // 位子留給執行端交接另案（§11）
};
