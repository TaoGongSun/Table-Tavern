// 聊天未讀。這次掛載的第一份畫面把已在的訊息當成看過（重新整理不整包算未讀）；
// 之後只有「聊天頁開著而且頁面可見」才把最後一則標成看過。熄螢幕期間到的不推進。
import { useEffect, useState } from "react";
import type { ChatMsg } from "../../../server/core/types";

export function useUnread(chat: ChatMsg[], me: string, onChat: boolean) {
  const [seen, setSeen] = useState(() => chat.at(-1)?.id ?? 0);
  const [visible, setVisible] = useState(() => globalThis.document?.visibilityState !== "hidden");
  useEffect(() => {
    const onVis = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);
  useEffect(() => {
    if (onChat && visible) setSeen(chat.at(-1)?.id ?? 0);
  }, [onChat, visible, chat]);
  const unread = chat.filter((m) => m.id > seen && m.from !== me);
  return { count: unread.length, latest: unread.at(-1) ?? null };
}
