// 免費模型換模提示（failover／switched 事件）的訂閱：只收本次檢視期間自己送出的 turn 的事件，
// 錨在那輪開始時的逐字稿長度；換幕、離桌（世代變）清空。世代與 turn 錨點由聊天 controller 持有、原 ref 傳進來。
import { MutableRefObject, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  createEventDeduper,
  FAILOVER_EVENT,
  type FailoverPayload,
  failoverNotice,
  SWITCHED_EVENT,
  type SmartFreeNotice,
  type SwitchedPayload,
  switchedNotice,
} from "../../ai-connection/smart-free-events";

/** 聊天室裡的一行非持久系統提示（不進逐字稿）；at＝出現時逐字稿有幾則，畫面據此插在那個位置 */
export interface ChatNotice {
  id: string;
  at: number;
  notice: SmartFreeNotice;
}

/** 要在 render 期世代遞增之後呼叫：effect 抓的 gen 與 visibleNotices 的過濾都讀 generation.current */
export function useSmartFreeNotices(
  worldId: string,
  scene: number,
  generation: MutableRefObject<number>,
  ownTurns: MutableRefObject<Map<string, number>>,
): ChatNotice[] {
  const [notices, setNotices] = useState<(ChatNotice & { gen: number })[]>([]);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const accept = createEventDeduper();
    const gen = generation.current;
    const show = (
      payload: { eventId: string; world: string | null; turnId: string | null },
      notice: SmartFreeNotice,
    ) => {
      if (disposed || generation.current !== gen) return;
      if (payload.turnId === null || payload.world !== worldId) return;
      const anchor = ownTurns.current.get(payload.turnId);
      if (anchor === undefined) return;
      if (!accept(payload.eventId)) return;
      setNotices((previous) => [
        ...previous,
        { id: payload.eventId, at: anchor, notice, gen },
      ]);
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
      stops.forEach((stop) => stop());
      setNotices([]);
    };
  }, [worldId, scene]);

  return useMemo(
    () => notices.filter((entry) => entry.gen === generation.current),
    [notices, worldId, scene],
  );
}
