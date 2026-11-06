// 聊天顯示名。暱稱與訊息都是不可信文字，呼叫端只放進 React 文字節點。
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";

/** 訊息列：自己是「你」、房主是「房主」、人已不在名單是「已離開」；`null` 是公開的「大家」 */
export function chatWho(view: RoomView, id: string | null): string {
  if (id === null) return t("chatToAll");
  if (id === view.me.memberId) return t("chatYou");
  if (id === view.room.hostId) return t("chatHost");
  return view.members.find((m) => m.memberId === id)?.nickname ?? t("chatLeft");
}

/** 提示列用暱稱（房主也寫名字）；人不在名單才寫「已離開」 */
export function chatSpeaker(view: RoomView, id: string): string {
  return view.members.find((m) => m.memberId === id)?.nickname ?? t("chatLeft");
}

/** 對象下拉：房主在暱稱後標「房主」，其餘只用暱稱 */
export function chatOption(view: RoomView, id: string, nickname: string): string {
  return id === view.room.hostId ? `${nickname}${t("chatHostTag")}` : nickname;
}
