// AI 代管的畫面判斷（room-seat-delegate §5.1）：鈕什麼時候出現、對象這回合送出了沒、是不是全員代管。
import type { RoomView } from "./room-state";

/** 代管鈕只在開演後出現（大廳還沒東西可代演；關房後唯讀） */
export const canDelegate = (v: RoomView) =>
  v.room.status === "playing" || v.room.status === "transition";

/** 這回合已送出或已跳過：交給代管時會退回草稿、不計入 */
export const sentThisRound = (v: RoomView, memberId: string) => {
  const e =
    v.round?.phase === "acting" ? v.round.roster.find((x) => x.memberId === memberId) : undefined;
  return e?.status === "submitted" || e?.status === "skipped";
};

/** 入座的人全在代管中：行動名單空著、回合不會自己往下走 */
export const allDelegated = (v: RoomView) => {
  const seated = v.members.filter((m) => m.cardId);
  return v.room.status === "playing" && seated.length > 0 && seated.every((m) => m.delegated);
};

/** 入座、沒連線、還沒代管的人（一鍵代管與重開提醒用） */
export const offlineSeated = (v: RoomView) =>
  v.members.filter((m) => m.cardId && !m.connected && !m.delegated);

export const cardName = (v: RoomView, cardId: string | null) =>
  v.cards.find((c) => c.cardId === cardId)?.name ?? "";
