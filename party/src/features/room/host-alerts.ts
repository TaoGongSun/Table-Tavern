// 房主提醒（room-status-strip §2）：只比對前後兩份畫面，第一份快照不響。
// 同一拍裡回合可能已經結算完，所以「還在行動」看上一份；回合號變了就不比名單。
import { useCallback, useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { cardName } from "./delegation";
import type { RoomView } from "./room-state";

export interface HostToast {
  id: number;
  text: string;
}

const CHIME_GAP = 1000;
let audio: AudioContext | null = null;
let lastChime = 0;

/** 在使用者點擊裡建立／恢復音訊：自動播放限制只認手勢當下，事件觸發的 resume 常常拿不到。失敗就安靜。 */
export function primeChime(): void {
  try {
    const Ctx = window.AudioContext;
    if (!Ctx) return;
    audio ??= new Ctx();
    if (audio.state !== "running") audio.resume().catch(() => {}); // 外層 try 接不到 Promise 的拒絕
  } catch {
    // 沒有音訊裝置
  }
}

/** 兩聲短音。這裡再擋一秒內的重入；context 沒被手勢解鎖就只會安靜。 */
function chime(): void {
  try {
    const now = Date.now();
    if (now - lastChime < CHIME_GAP) return;
    lastChime = now;
    if (!audio || audio.state !== "running") return;
    const tone = (freq: number, at: number) => {
      const osc = audio!.createOscillator();
      const gain = audio!.createGain();
      osc.frequency.value = freq;
      gain.gain.value = 0.15;
      osc.connect(gain);
      gain.connect(audio!.destination);
      osc.start(audio!.currentTime + at);
      osc.stop(audio!.currentTime + at + 0.08);
    };
    tone(660, 0);
    tone(880, 0.08);
  } catch {
    // 自動播放被擋、或沒有音訊裝置
  }
}

function alertTexts(before: RoomView, next: RoomView): string[] {
  const out: string[] = [];
  const me = next.me.memberId;
  const prevById = new Map(before.members.map((m) => [m.memberId, m]));
  for (const m of next.members) {
    if (m.memberId === me) continue;
    const old = prevById.get(m.memberId);
    if (!old) {
      out.push(
        next.room.status === "lobby" && !m.cardId
          ? t("hostAlertJoinedLobby", { name: m.nickname })
          : t("hostAlertJoined", { name: m.nickname }),
      );
      // 你不在時進房又選完角：兩件事都講
      if (m.cardId)
        out.push(t("hostAlertSeated", { name: m.nickname, card: cardName(next, m.cardId) }));
    } else if (old.cardId == null && m.cardId) {
      out.push(t("hostAlertSeated", { name: m.nickname, card: cardName(next, m.cardId) }));
    }
  }
  const asked = new Set(before.seatRequests.map((r) => r.requestId));
  for (const r of next.seatRequests) {
    const name = next.members.find((m) => m.memberId === r.memberId)?.nickname;
    if (name && !asked.has(r.requestId))
      out.push(t("hostAlertSeatRequest", { name, card: cardName(next, r.cardId) }));
  }
  const br = before.round;
  const nr = next.round;
  if (!br || !nr || br.round !== nr.round || br.phase !== "acting") return out;
  const prevRoster = new Map(br.roster.map((e) => [e.memberId, e]));
  const left = nr.roster.filter((e) => e.status === "pending" || e.status === "drafting").length;
  for (const e of nr.roster) {
    if (e.memberId === me) continue;
    const old = prevRoster.get(e.memberId);
    if (old?.status === e.status) continue;
    if (e.status !== "submitted" && e.status !== "skipped") continue;
    const name = cardName(next, e.cardId);
    if (left === 0) out.push(t("hostAlertAllIn", { name }));
    else if (e.status === "submitted") out.push(t("hostAlertSubmitted", { name, n: left }));
    else out.push(t("hostAlertSkipped", { name, n: left }));
  }
  for (const e of nr.roster) {
    if (e.memberId === me) continue;
    if (e.status === "submitted" || e.status === "skipped") continue;
    const oldM = prevById.get(e.memberId);
    const nowM = next.members.find((m) => m.memberId === e.memberId);
    if (oldM?.connected && nowM && !nowM.connected)
      out.push(t("hostAlertOffline", { name: cardName(next, e.cardId) }));
  }
  return out;
}

export function useHostAlerts(view: RoomView, opts: { sound: boolean }) {
  const [toasts, setToasts] = useState<HostToast[]>([]);
  const prev = useRef<RoomView | null>(null);
  const seq = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const live = useRef(toasts);
  live.current = toasts;
  const { sound } = opts;

  const dismiss = useCallback((id: number) => {
    const handle = timers.current.get(id);
    if (handle) clearTimeout(handle);
    timers.current.delete(id);
    setToasts((list) =>
      list.some((item) => item.id === id) ? list.filter((item) => item.id !== id) : list,
    );
  }, []);

  const push = useCallback(
    (texts: string[]) => {
      if (!texts.length) return;
      const items = texts.map((text) => ({ id: ++seq.current, text }));
      const next = [...live.current, ...items].slice(-3);
      const keep = new Set(next.map((item) => item.id));
      for (const id of [...timers.current.keys()]) {
        if (keep.has(id)) continue;
        const handle = timers.current.get(id);
        if (handle) clearTimeout(handle);
        timers.current.delete(id);
      }
      for (const item of items) {
        if (!keep.has(item.id)) continue;
        timers.current.set(
          item.id,
          setTimeout(() => dismiss(item.id), 4000),
        );
      }
      live.current = next;
      setToasts(next);
      if (sound) chime();
      try {
        navigator.vibrate?.(80);
      } catch {
        // 沒有震動或被瀏覽器擋下
      }
    },
    [dismiss, sound],
  );

  // 房主任何一次點擊都順手解鎖音訊（resume 已 running 時是空操作）。
  // 用 click 不用 pointerdown：觸控的 pointerdown 不算使用者啟用，click 滑鼠與觸控都算
  const isHost = view.room.hostId === view.me.memberId && view.room.status !== "closed";
  useEffect(() => {
    if (!isHost || !sound) return;
    document.addEventListener("click", primeChime, true);
    return () => document.removeEventListener("click", primeChime, true);
  }, [isHost, sound]);

  useEffect(() => {
    const host = view.room.hostId === view.me.memberId && view.room.status !== "closed";
    if (!host) {
      prev.current = null;
      setToasts((list) => (list.length ? [] : list));
      return;
    }
    const before = prev.current;
    prev.current = view;
    if (!before) return;
    push(alertTexts(before, view));
  }, [view, push]);

  useEffect(
    () => () => {
      for (const handle of timers.current.values()) clearTimeout(handle);
      timers.current.clear();
    },
    [],
  );

  return { toasts, dismiss };
}
