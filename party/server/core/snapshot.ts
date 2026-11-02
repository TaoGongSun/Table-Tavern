// 重連快照（協定 §4.3）：整份換掉本地狀態用。草稿只給本人、額度細節只給出額度者，裝置碼與雜湊一律不出。
import { execViewFor, isOnline } from "./round";
import type { ActionItem, ExecutorError, ExecutorView, Live, PublicCard, QuotaReport, RoomState, RouteOffer, RoundView, Speaker, StoredEvent, StoryPart } from "./types";

export interface Snapshot {
  seq: number;
  room: Pick<RoomState, "code" | "status" | "settings" | "hostId" | "payerId" | "scene" | "sceneSettings" | "createdAt" | "idleDays"> & {
    transition: { jobId: string; since: number; stalled: boolean } | null;
  };
  me: { memberId: string; nickname: string; cardId: string | null; isHost: boolean; isPayer: boolean; draft: string | null } | null;
  members: Array<{ memberId: string; nickname: string; cardId: string | null; connected: boolean }>;
  cards: PublicCard[];
  executor: ExecutorView;
  round: RoundView | null;
  story: StoryPart[];
  streaming: { round: number; part: number; speaker: Speaker; text: string } | null;
  quota: Pick<QuotaReport, "state" | "resetsAt"> | null;
  quotaDetail?: QuotaReport | null;
  routes?: RouteOffer[] | null; // 以下兩欄只給出額度者
  errorDetail?: { round: number; error: ExecutorError } | null;
}

/** 段號約定：玩家行動用負數（-1 起，依批次順序），幕摘要用 32（GM 段號 0–31 之外），不與生成段撞號 */
export function storyOf(events: StoredEvent[]): StoryPart[] {
  const story: StoryPart[] = [];
  for (const { type, at, payload: p } of events) {
    if (type === "story.part") story.push(p as unknown as StoryPart);
    if (type === "scene.summary") story.push({ round: p.round as number, part: 32, speaker: { kind: "system", marker: "scene_summary" }, text: p.text as string, interrupted: false, at });
    if (type !== "round.resolved") continue;
    (p.batch as ActionItem[]).forEach((a, i) => {
      if (a.kind === "act") story.push({ round: p.round as number, part: -1 - i, speaker: { kind: "player", memberId: a.memberId, cardId: a.cardId, name: a.name }, text: a.text!, interrupted: false, at });
    });
  }
  return story;
}

/** 即時值（不落事件日誌）：補事件的重連路徑也要帶，否則手機一直顯示過時的在線與草稿狀態 */
export function liveValues(s: RoomState, live: Live, viewer: string) {
  const st = live.stream;
  const payer = viewer === s.payerId;
  return {
    executor: execViewFor(s, viewer),
    ...(payer ? { errorDetail: s.lastError ?? null } : {}),
    presence: s.members.filter((m) => !m.banned).map((m) => ({ memberId: m.id, connected: isOnline(s, m.id) })),
    rosterStatus: s.round ? s.round.roster.map((e) => ({ round: s.round!.round, memberId: e.memberId, status: e.status })) : [],
    quota: s.exec.quota && { state: s.exec.quota.state, resetsAt: s.exec.quota.resetsAt },
    streaming: st && { round: st.round, part: st.part, speaker: st.speaker, text: st.text },
  };
}

export function buildSnapshot(s: RoomState, events: StoredEvent[], viewer: string, live: Live): Snapshot {
  const me = s.members.find((m) => m.id === viewer);
  const r = s.round;
  const t = s.transition;
  const { executor, quota, streaming } = liveValues(s, live, viewer);
  const payer = me !== undefined && s.payerId === me.id;
  return {
    seq: s.seq,
    room: { code: s.code, status: s.status, settings: s.settings, hostId: s.hostId, payerId: s.payerId, scene: s.scene, sceneSettings: s.sceneSettings, createdAt: s.createdAt, idleDays: s.idleDays, transition: t && { jobId: t.jobId, since: t.since, stalled: t.stalled } },
    me: me ? { memberId: me.id, nickname: me.nickname, cardId: me.cardId, isHost: s.hostId === me.id, isPayer: s.payerId === me.id, draft: s.drafts[me.id] ?? null } : null,
    members: s.members.filter((m) => !m.banned).map((m) => ({ memberId: m.id, nickname: m.nickname, cardId: m.cardId, connected: isOnline(s, m.id) })),
    cards: s.cards,
    executor,
    round: r && { round: r.round, kind: r.kind, scene: r.scene, phase: r.phase, roster: r.roster, deadline: r.deadline, pausedRemainingMs: r.pausedRemainingMs, dispatchId: r.dispatchId, hold: r.hold },
    story: storyOf(events),
    streaming,
    quota,
    ...(payer ? { quotaDetail: s.exec.quota, routes: s.exec.caps?.routes ?? null, errorDetail: s.lastError ?? null } : {}),
  };
}
