// 故事頁（以及其他非聊天頁）的一行未讀提示：最後一則未讀的「名字：內文」，點了切到聊天。
import type { ChatMsg } from "../../../server/core/types";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { chatSpeaker } from "./chat-label";
import "./chat.css";

export function ChatNotice({
  view,
  msg,
  onOpen,
}: {
  view: RoomView;
  msg: ChatMsg;
  onOpen: () => void;
}) {
  return (
    <button type="button" className="banner chat-notice" data-testid="chat-notice" onClick={onOpen}>
      {t("chatNotice", { name: chatSpeaker(view, msg.from), text: msg.text })}
      {msg.to !== null && t("chatPrivate")}
    </button>
  );
}
