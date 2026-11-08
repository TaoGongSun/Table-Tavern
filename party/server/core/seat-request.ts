// 換裝置認領座位：玩家按「這是我的？」、房主確認。暱稱不是憑證，有效條件只看裝置狀態；都不進事件日誌，只給房主與認領人。
import { isOnline } from "./round";
import type { Tx } from "./tx";
import type { Live, Member, RoomState, SeatRequestView } from "./types";

/** 送出、確認、集中掃描共用。`occupantId`＝認領當時的佔位者，換人就失效 */
export function seatRequestValid(
  s: RoomState,
  live: Live,
  m: Member,
  cardId: string,
  occupantId?: string,
) {
  if ((s.status !== "lobby" && s.status !== "playing") || m.banned || m.cardId) return false;
  // 建卡、匯入的結果會替他入座，接手後就坐不下了
  if (
    s.jobs.some(
      (j) => "memberId" in j.job && j.job.memberId === m.id && j.job.kind.startsWith("card_"),
    )
  )
    return false;
  if (Object.values(live.uploads).some((u) => u.memberId === m.id && u.kind === "card"))
    return false;
  const occupantOf = s.cards.find((c) => c.cardId === cardId)?.claimedBy;
  const o = s.members.find((x) => x.id === occupantOf);
  return (
    !!o &&
    !o.banned &&
    o.id !== m.id &&
    o.id !== s.hostId &&
    o.id !== s.payerId &&
    !isOnline(s, o.id) &&
    (occupantId === undefined || o.id === occupantId)
  );
}

/** 只給房主：欄位逐一挑，不把成員狀態整包送出 */
export function seatRequestsFor(s: RoomState): SeatRequestView[] {
  return s.members.flatMap((m) => {
    const r = m.seatRequest;
    if (!r) return [];
    const offlineSince = s.members.find((x) => x.id === r.occupantId)?.offlineSince ?? null;
    return [
      {
        requestId: r.requestId,
        memberId: m.id,
        cardId: r.cardId,
        occupantId: r.occupantId,
        offlineSince,
      },
    ];
  });
}

/** 每筆輸入收尾：清掉不再成立的認領；有變動才通知認領人與房主（房主換人時，新房主拿完整清單、舊房主清空） */
export function settleSeatRequests(tx: Tx, before: RoomState | null) {
  const live = tx.ctx.live;
  for (const m of tx.s.members) {
    const r = m.seatRequest;
    if (r && !seatRequestValid(tx.s, live, m, r.cardId, r.occupantId))
      delete tx.w.members.find((x) => x.id === m.id)!.seatRequest;
  }
  const s = tx.s;
  const was = (id: string) => before?.members.find((x) => x.id === id)?.seatRequest;
  for (const m of s.members)
    if (m.seatRequest?.requestId !== was(m.id)?.requestId)
      tx.toMember(m.id, "vol", { type: "seat_request", cardId: m.seatRequest?.cardId ?? null });
  const list = seatRequestsFor(s);
  const old = before ? seatRequestsFor(before) : [];
  if (s.hostId !== (before?.hostId ?? null)) {
    if (before?.hostId) tx.toMember(before.hostId, "vol", { type: "seat_requests", list: [] });
    tx.toMember(s.hostId, "vol", { type: "seat_requests", list });
  } else if (JSON.stringify(list) !== JSON.stringify(old))
    tx.toMember(s.hostId, "vol", { type: "seat_requests", list });
}
