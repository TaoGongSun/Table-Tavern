// 重連快照（協定 §4.3）：整份換掉本地狀態用。草稿只給本人、額度細節只給出額度者，裝置碼與雜湊一律不出。
import { carryOf, execViewFor, isOnline } from "./round";
import type { Tx } from "./tx";
import type {
  ActionItem,
  ExecutorError,
  ExecutorView,
  Live,
  PublicCard,
  QuotaReport,
  RoomState,
  RouteOffer,
  RoundView,
  ChatMsg,
  Speaker,
  StoredEvent,
  StoryPart,
} from "./types";

export interface Snapshot {
  seq: number;
  room: Pick<
    RoomState,
    | "code"
    | "status"
    | "settings"
    | "hostId"
    | "payerId"
    | "scene"
    | "sceneSettings"
    | "createdAt"
    | "idleDays"
  > & {
    transition: { jobId: string; since: number; stalled: boolean } | null;
  };
  me: {
    memberId: string;
    nickname: string;
    cardId: string | null;
    isHost: boolean;
    isPayer: boolean;
    draft: string | null;
    returned: string | null;
    delegated: boolean;
  } | null;
  members: Array<{
    memberId: string;
    nickname: string;
    cardId: string | null;
    connected: boolean;
    delegated: boolean;
  }>;
  cards: PublicCard[];
  executor: ExecutorView;
  round: RoundView | null;
  story: StoryPart[];
  streaming: { round: number; part: number; speaker: Speaker; text: string } | null;
  quota: Pick<QuotaReport, "state" | "resetsAt"> | null;
  /** 這位看得到的桌邊聊天（公開，或自己是收發方的私訊）。執行端不是成員，一律空陣列 */
  chat: ChatMsg[];
  quotaDetail?: QuotaReport | null;
  routes?: RouteOffer[] | null; // 以下兩欄只給出額度者
  errorDetail?: { round: number; error: ExecutorError } | null;
}

/** 房主收回過的回合（§3.2 round.undone）：那回合的 GM／NPC／系統段與玩家行動段都不算數；匯出也用 */
export const undoneRounds = (events: StoredEvent[]) =>
  new Set(events.filter((e) => e.type === "round.undone").map((e) => e.payload.round as number));

export type KeepThrough = { round: number; part: number } | null;
/** 重開補送撤段（§3.3 story.truncated）：本幕（`from` 起）保留到 `after` 那段為止，之後撤掉；`null` 或指不到
 *  ＝本幕全撤（往回翻頁時那段可能在更早的一頁）。快照、客戶端、匯出共用 */
export function keptCount(
  keys: Array<{ round: number; part: number }>,
  after: KeepThrough,
  from: number,
): number {
  const i = after
    ? keys.findIndex((p, k) => k >= from && p.round === after.round && p.part === after.part)
    : -1;
  return i >= 0 ? i + 1 : Math.min(from, keys.length);
}

/** 段號約定：玩家行動用負數（-1 起，依批次順序），幕摘要用 32、收回提示用 33（GM 段號 0–31 之外），不與生成段撞號 */
export function storyOf(events: StoredEvent[]): StoryPart[] {
  let story: StoryPart[] = [];
  let from = 0; // 本幕第一段在 story 裡的位置（上一幕摘要之後）
  const undone = undoneRounds(events);
  for (const { type, at, payload: p } of events) {
    if (type === "scene.started") from = story.length;
    if (type === "story.truncated")
      story = story.slice(0, keptCount(story, p.after as KeepThrough, from));
    if (type === "round.undone")
      story.push({
        round: p.round as number,
        part: 33,
        speaker: { kind: "system", marker: "round_undone" },
        text: "",
        interrupted: false,
        at,
      });
    if (undone.has(p.round as number) && (type === "story.part" || type === "round.resolved"))
      continue;
    if (type === "story.part") story.push(p as unknown as StoryPart);
    if (type === "scene.summary") {
      story.push({
        round: p.round as number,
        part: 32,
        speaker: { kind: "system", marker: "scene_summary" },
        text: p.text as string,
        interrupted: false,
        at,
      });
      from = story.length; // 快照不帶 scene.started（它在 sceneStartSeq 本身），以摘要為界
    }
    if (type !== "round.resolved") continue;
    (p.batch as ActionItem[]).forEach((a, i) => {
      if (a.kind === "act")
        story.push({
          round: p.round as number,
          part: -1 - i,
          speaker: { kind: "player", memberId: a.memberId, cardId: a.cardId, name: a.name },
          text: a.text!,
          interrupted: false,
          at,
        });
      else if (a.watchReason === "skipped")
        story.push({
          round: p.round as number,
          part: -1 - i,
          speaker: { kind: "player", memberId: a.memberId, cardId: a.cardId, name: a.name },
          text: "",
          interrupted: false,
          at,
          handed: true,
        });
    });
  }
  return story.filter((p, i) => !(isUndoneNote(p) && story[i + 1] && isUndoneNote(story[i + 1]))); // 連續收回只留一行提示
}
const isUndoneNote = (p: StoryPart) => p.part === 33;

/** 這位成員看得到的聊天。viewer 不是成員（執行端 welcome 也走這裡）連公開訊息都不給 */
export function visibleChat(s: RoomState, viewer: string): ChatMsg[] {
  if (!s.members.some((m) => m.id === viewer)) return [];
  return (s.chat ?? []).filter((m) => m.to === null || m.from === viewer || m.to === viewer);
}

/** 即時值（不落事件日誌）：補事件的重連路徑也要帶，否則手機一直顯示過時的在線與草稿狀態 */
export function liveValues(s: RoomState, live: Live, viewer: string) {
  const st = live.stream;
  const payer = viewer === s.payerId;
  return {
    executor: execViewFor(s, viewer),
    ...(payer ? { errorDetail: s.lastError ?? null } : {}),
    presence: s.members
      .filter((m) => !m.banned)
      .map((m) => ({ memberId: m.id, connected: isOnline(s, m.id) })),
    rosterStatus: s.round
      ? s.round.roster.map((e) => ({
          round: s.round!.round,
          memberId: e.memberId,
          status: e.status,
        }))
      : [],
    quota: s.exec.quota && { state: s.exec.quota.state, resetsAt: s.exec.quota.resetsAt },
    streaming: st && { round: st.round, part: st.part, speaker: st.speaker, text: st.text },
    chat: visibleChat(s, viewer),
  };
}

export function buildSnapshot(
  s: RoomState,
  events: StoredEvent[],
  viewer: string,
  live: Live,
): Snapshot {
  const me = s.members.find((m) => m.id === viewer);
  const r = s.round;
  const t = s.transition;
  const { executor, quota, streaming } = liveValues(s, live, viewer);
  const payer = me !== undefined && s.payerId === me.id;
  return {
    seq: s.seq,
    room: {
      code: s.code,
      status: s.status,
      settings: s.settings,
      hostId: s.hostId,
      payerId: s.payerId,
      scene: s.scene,
      sceneSettings: s.sceneSettings,
      createdAt: s.createdAt,
      idleDays: s.idleDays,
      transition: t && { jobId: t.jobId, since: t.since, stalled: t.stalled },
    },
    me: me
      ? {
          memberId: me.id,
          nickname: me.nickname,
          cardId: me.cardId,
          isHost: s.hostId === me.id,
          isPayer: s.payerId === me.id,
          draft: s.drafts[me.id] ?? null,
          returned: s.returned?.[me.id] ?? null,
          delegated: !!me.delegated,
        }
      : null,
    members: s.members
      .filter((m) => !m.banned)
      .map((m) => ({
        memberId: m.id,
        nickname: m.nickname,
        cardId: m.cardId,
        connected: isOnline(s, m.id),
        delegated: !!m.delegated,
      })),
    cards: s.cards,
    executor,
    round: r && {
      round: r.round,
      kind: r.kind,
      scene: r.scene,
      phase: r.phase,
      roster: r.roster,
      deadline: r.deadline,
      pausedRemainingMs: r.pausedRemainingMs,
      dispatchId: r.dispatchId,
      hold: r.hold,
      called: viewer === s.hostId ? (r.called ?? null) : null,
      ...carryOf(s, r.kind),
    },
    story: storyOf(events),
    streaming,
    quota,
    chat: visibleChat(s, viewer),
    ...(payer
      ? {
          quotaDetail: s.exec.quota,
          routes: s.exec.caps?.routes ?? null,
          errorDetail: s.lastError ?? null,
        }
      : {}),
  };
}

/** 整份快照給一條連線（`welcome`）：重連與第一次加入，或身分變了要重給（新出額度者的專屬欄位） */
export async function sendSnapshot(
  tx: Tx,
  conn: string,
  who: string,
  extra: Record<string, unknown> = {},
) {
  const s = tx.s;
  const summaries = await Promise.all(
    s.summarySeqs.map((seq) => tx.ctx.store.eventsAfter(seq - 1, 1)),
  );
  const scene = [
    ...(await tx.ctx.store.eventsAfter(s.sceneStartSeq, Number.MAX_SAFE_INTEGER)),
    ...tx.events,
  ];
  const snapshot = buildSnapshot(
    s,
    [...summaries.flat(), ...scene.filter((e) => e.seq > s.sceneStartSeq)],
    who,
    tx.ctx.live,
  );
  tx.send(conn, "welcome", { ...(who !== "exec" ? { memberId: who } : {}), snapshot, ...extra });
}

/** 某位成員所有連線重給快照 */
export async function resnapshot(tx: Tx, memberId: string) {
  for (const [id, c] of Object.entries(tx.s.conns))
    if (c.who === memberId) await sendSnapshot(tx, id, memberId);
}
