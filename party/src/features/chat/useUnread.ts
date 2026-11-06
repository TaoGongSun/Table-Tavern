// 聊天未讀。這次掛載的第一份畫面把已在的訊息當成看過（重新整理不整包算未讀）；
// 之後只有「聊天頁開著而且頁面可見」才把最後一則標成看過。熄螢幕期間到的不推進。
import { useEffect, useState } from "react";
import type { ChatMsg } from "../../../server/core/types";

// 根目錄桌面版的 tsconfig lib 沒有 Array.prototype.at，這裡用索引取最後一則
const lastId = (chat: ChatMsg[]) => chat[chat.length - 1]?.id ?? 0;

export function useUnread(chat: ChatMsg[], me: string, onChat: boolean) {
  const [seen, setSeen] = useState(() => lastId(chat));
  const [visible, setVisible] = useState(() => globalThis.document?.visibilityState !== "hidden");
  useEffect(() => {
    const onVis = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);
  useEffect(() => {
    if (onChat && visible) setSeen(lastId(chat));
  }, [onChat, visible, chat]);
  const unread = chat.filter((m) => m.id > seen && m.from !== me);
  return { count: unread.length, latest: unread[unread.length - 1] ?? null };
}
