// 手機端的房間畫面狀態：快照整份換掉（§4.3），之後逐筆套持久事件（ev，序號只增）與即時訊息（vol）。
// 規則是伺服器核心的鏡像但只管「畫面上看得到的」；對不上時伺服器為準（E_STATE_CONFLICT 會重抓快照）。
// 劇情段落直接用伺服器的 storyOf 投影（玩家行動段號為負、幕摘要段號 32），快照與即時事件長得一樣。
import { keptCount, storyOf, type Snapshot } from "../../../server/core/snapshot";
import type { ExecutorError, Frame, RosterEntry, RouteOffer, StoryPart } from "../../../server/core/types";

export interface RoomView extends Omit<Snapshot, "quotaDetail"> {
  me: NonNullable<Snapshot["me"]>;
  quotaDetail: Snapshot["quotaDetail"] | null;
  errorDetail: { round: number; error: ExecutorError } | null;
  /** 執行端自報的路線與模型清單（伺服器只給出額度者）；沒有就讓出額度者手填 */
  routes: RouteOffer[] | null;
  /** 這次連線期間 playing 中離座的卡，房主可決定 npc／absent（快照不帶，重新整理後清空） */
  departed: string[];
}

const key = (p: StoryPart) => `${p.round}:${p.part}`;
export function mergeStory(story: StoryPart[], parts: StoryPart[], older = false): StoryPart[] {
  if (!parts.length) return story;
  const out = [...story];
  const fresh: StoryPart[] = [];
  for (const p of parts) {
    const i = out.findIndex((x) => key(x) === key(p));
    if (i >= 0) out[i] = p;
    else fresh.push(p);
  }
  return older ? [...fresh, ...out] : [...out, ...fresh];
}

/** 事件 → 劇情段落（同伺服器快照的投影） */
export const storyOfFrames = (frames: Frame[]) => storyOf(frames.map((f) => ({ seq: f.p.seq, at: f.p.at, type: f.p.e.type, payload: f.p.e })));

function fromSnapshot(s: Snapshot): RoomView {
  return { ...s, me: s.me!, quotaDetail: s.quotaDetail ?? null, errorDetail: s.errorDetail ?? null, routes: s.routes ?? null, departed: [] };
}

/** 寫時複製：成員、名單都很小，每筆淺拷貝；劇情只在有變動時換陣列 */
function copy(v: RoomView): RoomView {
  return { ...v, room: { ...v.room }, me: { ...v.me }, members: v.members.map((m) => ({ ...m })), round: v.round && { ...v.round, roster: v.round.roster.map((e) => ({ ...e })) } };
}

function setSeat(v: RoomView, memberId: string, cardId: string | null) {
  const m = v.members.find((x) => x.memberId === memberId);
  const prev = m?.cardId ?? null;
  if (m) m.cardId = cardId;
  if (memberId === v.me.memberId) v.me.cardId = cardId;
  v.cards = v.cards.map((c) => (c.cardId === cardId ? { ...c, claimedBy: memberId } : c.cardId === prev && c.claimedBy === memberId ? { ...c, claimedBy: null } : c));
}

function onEvent(v: RoomView, e: any, at: number) {
  const entry = (memberId: string): RosterEntry | undefined => v.round?.roster.find((x) => x.memberId === memberId);
  const mine = e.memberId === v.me.memberId;
  const round = v.round;
  switch (e.type) {
    case "room.started":
    case "scene.started":
      Object.assign(v.room, { status: "playing", scene: e.scene, sceneSettings: e.settings, transition: null });
      break;
    case "scene.settings_switched":
      v.room.sceneSettings = e.settings;
      break;
    case "room.settings_changed":
      v.room.settings = e.settings;
      break;
    case "room.closed":
      v.room.status = "closed";
      break;
    case "member.joined":
      if (!v.members.some((m) => m.memberId === e.memberId)) v.members.push({ memberId: e.memberId, nickname: e.nickname, cardId: null, connected: false });
      break;
    case "member.renamed":
      for (const m of v.members) if (m.memberId === e.memberId) m.nickname = e.nickname;
      if (mine) v.me.nickname = e.nickname;
      break;
    case "member.kicked":
      v.members = v.members.filter((m) => m.memberId !== e.memberId);
      break;
    case "host.changed":
      v.room.hostId = e.memberId;
      break;
    case "pair.bound":
      v.room.payerId = e.memberId;
      break;
    case "pair.unbound":
      v.room.payerId = null;
      break;
    case "seat.claimed":
      setSeat(v, e.memberId, e.cardId);
      break;
    case "seat.released":
      setSeat(v, e.memberId, null);
      if (v.round) v.round.roster = v.round.roster.filter((x) => x.memberId !== e.memberId);
      if (v.room.status !== "lobby") v.departed = [...v.departed.filter((c) => c !== e.cardId), e.cardId];
      break;
    case "seat.reassigned":
      setSeat(v, e.from, null);
      setSeat(v, e.to, e.cardId);
      for (const x of v.round?.roster ?? []) if (x.memberId === e.from) x.memberId = e.to;
      break;
    case "seat.departure":
      v.departed = v.departed.filter((c) => c !== e.cardId);
      break;
    case "roster.updated":
      v.cards = e.cards;
      break;
    case "scene.advancing":
      Object.assign(v.room, { status: "transition", transition: { jobId: e.jobId, since: at, stalled: false } });
      break;
    case "scene.stalled":
      if (v.room.transition) v.room.transition = { ...v.room.transition, stalled: true };
      break;
    case "scene.failed":
      Object.assign(v.room, { status: "playing", transition: null });
      break;
    case "round.opened":
      v.round = { round: e.round, kind: e.kind, scene: v.room.scene, phase: e.phase ?? (e.kind === "play" ? "acting" : "queued"), roster: e.roster, deadline: e.deadline, pausedRemainingMs: null, dispatchId: null, hold: null };
      Object.assign(v, { streaming: null, errorDetail: null });
      break;
    case "round.deadline":
      if (round && round.round === e.round) Object.assign(round, { deadline: e.deadline, pausedRemainingMs: e.pausedRemainingMs });
      break;
    case "action.submitted": {
      const x = entry(e.memberId);
      if (x) Object.assign(x, { status: "submitted", text: e.text });
      if (mine) v.me.draft = null;
      break;
    }
    case "action.skipped": {
      const x = entry(e.memberId);
      if (x) Object.assign(x, { status: "skipped", text: null });
      break;
    }
    case "action.retracted": {
      const x = entry(e.memberId);
      if (!x) break;
      if (mine && x.text) v.me.draft = x.text; // 原文回成草稿
      Object.assign(x, { status: (mine ? v.me.draft : x.text) ? "drafting" : "pending", text: null });
      break;
    }
    case "round.resolved":
      if (v.round) Object.assign(v.round, { phase: "queued", deadline: null, pausedRemainingMs: null });
      break;
    case "round.cancelled": {
      const x = entry(v.me.memberId);
      if (x?.status === "submitted" && x.text) v.me.draft = x.text; // 換幕：已送出的行動退回成草稿
      v.round = null;
      break;
    }
    case "round.undone": // 房主收回：那回合的段落與玩家行動都撤掉（storyOf 另補一行提示）
      v.story = v.story.filter((p) => p.round !== e.round);
      if (v.story[v.story.length - 1]?.part === 33) v.story = v.story.slice(0, -1); // 連續收回只留一行（新的那行隨後補上）
      break;
    case "story.truncated": // 重開補送：本幕（上一幕摘要之後）保留到 after 為止
      v.story = v.story.slice(0, keptCount(v.story, e.after, v.story.map((p) => p.part).lastIndexOf(32) + 1));
      break;
    case "gen.dispatched":
      if (v.round) v.round.dispatchId = e.dispatchId;
      break;
    case "gen.failed":
      if (v.round) Object.assign(v.round, { phase: "held", hold: e.error });
      v.streaming = null;
      break;
  }
}

function onVolatile(v: RoomView, x: any) {
  const round = v.round?.round === x.round ? v.round : null; // 指名回合的即時訊息只套到當前回合
  switch (x.type) {
    case "delta": {
      const s = v.streaming;
      const same = s && s.round === x.round && s.part === x.part;
      v.streaming = { round: x.round, part: x.part, speaker: x.speaker ?? (same ? s.speaker : { kind: "gm" }), text: (same ? s.text : "") + x.text };
      if (round) Object.assign(round, { phase: "generating", hold: null });
      break;
    }
    case "resumed": // 客戶端自己的：遠端重試／切換被執行端接受，伺服器已把 held 轉回 generating（沒有事件）
      if (round?.phase === "held") Object.assign(round, { phase: "generating", hold: null });
      break;
    case "returned": // 只給本人：房主收回的行動原文（null＝丟掉了）
      v.me.returned = x.text;
      break;
    case "called": // 只給房主：上一則 GM 旁白點到的 NPC
      if (round) round.called = x.cardId;
      break;
    case "presence":
      for (const m of v.members) if (m.memberId === x.memberId) m.connected = x.connected;
      break;
    case "roster_status": {
      if (!round) break;
      const e = round.roster.find((r) => r.memberId === x.memberId);
      const cardId = v.members.find((m) => m.memberId === x.memberId)?.cardId;
      if (e) e.status = x.status;
      else if (cardId) round.roster.push({ memberId: x.memberId, cardId, status: x.status, text: null }); // 空名單停住時入座
      break;
    }
    case "executor": {
      const { type: _type, routes, ...executor } = x;
      v.executor = executor;
      if (routes) v.routes = routes;
      break;
    }
    case "quota":
      v.quota = x.coarse;
      break;
    case "quota_detail":
      v.quotaDetail = x.report;
      break;
    case "error_detail":
      v.errorDetail = { round: x.round, error: x.error };
      break;
  }
}

export function applyFrame(view: RoomView | null, f: Frame): RoomView | null {
  if (f.t === "welcome" && f.p.snapshot) return fromSnapshot(f.p.snapshot);
  if (!view) return null;
  const v = copy(view);
  if (f.t === "welcome" && f.p.live) {
    // 補事件的重連：即時值另外帶（§4.3），事件隨後逐筆補
    const live = f.p.live;
    onVolatile(v, { type: "executor", ...live.executor });
    for (const p of live.presence) onVolatile(v, { type: "presence", ...p });
    for (const r of live.rosterStatus) onVolatile(v, { type: "roster_status", ...r });
    Object.assign(v, { quota: live.quota, streaming: live.streaming });
    if (live.errorDetail !== undefined) v.errorDetail = live.errorDetail; // 只有出額度者會帶
  } else if (f.t === "ev") {
    if (f.p.seq <= view.seq) return view; // 重連補事件的重複
    v.seq = f.p.seq;
    onEvent(v, f.p.e, f.p.at);
    const parts = storyOfFrames([f]);
    v.story = mergeStory(v.story, parts);
    if (parts.some((p) => p.round === v.streaming?.round && p.part === v.streaming.part)) v.streaming = null; // 定稿整段取代串流
  } else if (f.t === "vol") onVolatile(v, f.p);
  else return view;
  return v;
}
