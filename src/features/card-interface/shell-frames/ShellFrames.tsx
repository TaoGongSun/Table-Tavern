// 卡片介面殼的雙緩衝（.ai/plans/shell-update-flash.md）：新殼先掛在舊殼下面載，殼內墊片 postMessage
// ready 才翻面；首開與換桌／還原／切卡直接繪。兩支 iframe 是同一父層下依時間排的 keyed siblings，
// 翻面只改角色、不搬元素（搬動 iframe 會重載）；絕不改既有元素的 srcdoc。
import { useCallback, useEffect, useLayoutEffect, useReducer, useRef } from "react";
import {
  EMPTY_FRAMES,
  FLIP_TIMEOUT_MS,
  framesReducer,
  type FrameSlot,
  type ShellRegistry,
} from "./shell-frames-state";
import { hostNow, parseReady, recordShellTiming, shellTimingEnabled } from "./shell-ready";

interface ShellFramesProps {
  /** 目前該顯示的殼文件；null＝不掛 */
  doc: string | null;
  shellKey: string;
  /** 顯示對象（桌、桌世代、殼所屬的卡）：與正在顯示的不同就硬切 */
  identity: string;
  registry: ShellRegistry;
  title: string;
  /** 某支殼回報 ready（推最新快照給它） */
  onReady: (key: string) => void;
}

export function ShellFrames({ doc, shellKey, identity, registry, title, onReady }: ShellFramesProps) {
  const [state, dispatch] = useReducer(framesReducer, EMPTY_FRAMES);
  const elements = useRef(new Map<string, HTMLIFrameElement>());
  // 早到的 ready：送來時那支 iframe 還沒登記，先記下來源，登記時重核
  const early = useRef<{ token: string; source: MessageEventSource | null } | null>(null);
  const stateRef = useRef(state);
  stateRef.current = state;
  const lockRef = useRef<HTMLDivElement | null>(null);

  useLayoutEffect(() => {
    dispatch({ type: "shell", slot: doc === null ? null : { key: shellKey, doc, identity }, now: hostNow() });
  }, [doc, shellKey, identity]);

  const flip = useCallback(
    (key: string, reason: string) => {
      if (stateRef.current.pending?.key !== key) return;
      recordShellTiming({ type: reason === "timeout" ? "forced" : "flip", key });
      dispatch(reason === "timeout" ? { type: "timeout" } : { type: "ready", key });
    },
    [],
  );

  // 每支 iframe 一個固定的 ref 回呼（每次 render 換新回呼會讓 React 反覆解除再登記）
  const refs = useRef(new Map<string, (element: HTMLIFrameElement | null) => void>());
  const register = (key: string) => {
    let callback = refs.current.get(key);
    if (callback === undefined) {
      callback = (element) => {
        if (element === null) {
          elements.current.delete(key);
          refs.current.delete(key);
          return;
        }
        elements.current.set(key, element);
        const held = early.current;
        if (held !== null && stateRef.current.pending?.key === key && held.token === key && held.source === element.contentWindow) {
          early.current = null;
          flip(key, "early");
        }
      };
      refs.current.set(key, callback);
    }
    return callback;
  };

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const ready = parseReady(event.data);
      if (ready === null) return;
      const { front, pending } = stateRef.current;
      const slot = [front, pending].find(
        (candidate): candidate is FrameSlot =>
          candidate !== null && elements.current.get(candidate.key)?.contentWindow === event.source,
      );
      if (slot === undefined) {
        if (pending !== null && ready.token === pending.key) early.current = { token: ready.token, source: event.source };
        return;
      }
      if (ready.token !== slot.key) return;
      recordShellTiming({ type: "ready", key: slot.key, role: slot === pending ? "pending" : "front", reason: ready.reason, t: ready.t });
      if (shellTimingEnabled() && event.source !== null) {
        (event.source as Window).postMessage({ source: "table-tavern-host", kind: "ping", token: slot.key, t0: hostNow() }, "*");
      }
      onReady(slot.key);
      if (slot === pending) flip(slot.key, "ready");
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [flip, onReady]);

  // 量測用：殼回報的時鐘校準與首次內容繪製
  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const data = event.data as Record<string, unknown> | null;
      if (!shellTimingEnabled() || data?.source !== "table-tavern-card") return;
      if (data.kind === "pong") recordShellTiming({ type: "pong", key: data.token, t0: data.t0, t1: data.t1 });
      else if (data.kind === "paint") recordShellTiming({ type: "fcp", key: data.token, fcp: data.fcp });
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  // 鎖定期總上限：從鎖定起點算，換 pending 不重算
  const { lockedAt, pending, front } = state;
  useEffect(() => {
    if (lockedAt === null || pending === null) return;
    const key = pending.key;
    const timer = window.setTimeout(() => flip(key, "timeout"), Math.max(0, lockedAt + FLIP_TIMEOUT_MS - hostNow()));
    return () => window.clearTimeout(timer);
  }, [lockedAt, pending, flip]);

  // 交給宿主訊息監聽的現況；鎖定時焦點若在舊殼裡就移到擋板，Enter／Space 不再送進舊殼
  useLayoutEffect(() => {
    const entry = (slot: FrameSlot | null) =>
      slot === null ? null : { key: slot.key, frame: elements.current.get(slot.key) ?? null };
    registry.front = entry(front);
    registry.pending = entry(pending);
    registry.locked = pending !== null;
    if (pending !== null && front !== null && document.activeElement === elements.current.get(front.key)) {
      lockRef.current?.focus();
    }
  }, [front, pending, registry]);

  useEffect(
    () => () => {
      registry.front = null;
      registry.pending = null;
      registry.locked = false;
    },
    [registry],
  );

  const locked = pending !== null;
  const slots = [front, pending].filter((slot): slot is FrameSlot => slot !== null);
  if (slots.length === 0) return null;
  return (
    <div className="card-interface-frames">
      {slots.map((slot) => (
        <ShellFrame
          key={slot.key}
          slot={slot}
          role={slot === front ? "front" : "pending"}
          locked={locked}
          title={title}
          frameRef={register(slot.key)}
        />
      ))}
      {locked && <div ref={lockRef} className="card-interface-lock" data-shell-lock="" tabIndex={-1} />}
    </div>
  );
}

function ShellFrame({
  slot,
  role,
  locked,
  title,
  frameRef,
}: {
  slot: FrameSlot;
  role: "front" | "pending";
  locked: boolean;
  title: string;
  frameRef: (element: HTMLIFrameElement | null) => void;
}) {
  const { key } = slot;
  useLayoutEffect(() => {
    recordShellTiming({ type: role === "pending" ? "pending" : "mount", key });
    return () => recordShellTiming({ type: "unmount", key });
  }, [key]); // 只記掛載與卸載；翻面（角色改變）由 flip 記
  return (
    <iframe
      ref={frameRef}
      className={`card-interface-frame is-${role}`}
      data-shell-key={key}
      data-shell-role={role}
      sandbox="allow-scripts"
      srcDoc={slot.doc}
      title={title}
      tabIndex={locked ? -1 : undefined}
      inert={locked}
    />
  );
}
