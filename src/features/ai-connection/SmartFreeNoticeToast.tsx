import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { t } from "../../i18n";
import {
  createEventDeduper,
  FAILOVER_EVENT,
  type FailoverPayload,
  failoverNotice,
  noticeText,
  SWITCHED_EVENT,
  type SmartFreeNotice,
  type SwitchedPayload,
  switchedNotice,
} from "./smart-free-events";

const AUTO_DISMISS_MS = 8000;

// 非聊天輪（換幕摘要、翻譯、重構、開桌、生圖）的免費模型換模提示：輕量、非阻塞、自動消失。
// 聊天輪（turnId 非 null）的事件由聊天室自己顯示，這裡一律忽略。
export function SmartFreeNoticeToast() {
  const [notice, setNotice] = useState<(SmartFreeNotice & { id: string }) | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const accept = createEventDeduper();
    const show = (payload: { eventId: string; turnId: string | null }, next: SmartFreeNotice) => {
      if (disposed || payload.turnId !== null || !accept(payload.eventId)) return;
      clearTimeout(timer.current);
      setNotice({ ...next, id: payload.eventId });
      timer.current = setTimeout(() => setNotice(null), AUTO_DISMISS_MS);
    };
    const track = (promise: Promise<() => void>) =>
      void promise.then((stop) => {
        if (disposed) stop();
        else stops.push(stop);
      });
    track(listen<FailoverPayload>(FAILOVER_EVENT, (event) => show(event.payload, failoverNotice(event.payload))));
    track(listen<SwitchedPayload>(SWITCHED_EVENT, (event) => show(event.payload, switchedNotice(event.payload))));
    return () => {
      disposed = true;
      clearTimeout(timer.current);
      stops.forEach((stop) => stop());
    };
  }, []);

  if (!notice) return null;
  return (
    <div className="smart-free-notice-toast" role="status">
      <span>{noticeText(notice)}</span>
      <button type="button" className="ghost" onClick={() => setNotice(null)}>
        {t("smartFreeNewDismiss")}
      </button>
    </div>
  );
}
