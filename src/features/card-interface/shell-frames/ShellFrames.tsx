// 卡片介面殼的雙緩衝（.ai/plans/shell-update-flash.md）：新殼先掛在舊殼下面載，殼內墊片 postMessage
// ready 才翻面；首開與換桌／還原／切卡直接繪。新殼逾時或載入失敗時保留載好過的舊殼、只解鎖
// （.ai/plans/shell-keep-old-on-timeout.md）。兩支 iframe 是同一父層下依時間排的 keyed siblings，
// 翻面只改角色、不搬元素（搬動 iframe 會重載）；每支 iframe 只在首次載入時收一次殼文件。
// iframe 載的是 Rust 的 tt-sandbox 外層（src-tauri/src/card_sandbox.rs，自帶寬政策、不吃宿主 CSP），
// 殼文件經 postMessage 送進去，外層放進內層 srcdoc 跑；訊息經外層雙向轉送（.ai/plans/desktop-csp.md）。
import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useLayoutEffect, useReducer, useRef, type SyntheticEvent } from "react";
import {
  EMPTY_FRAMES,
  FLIP_TIMEOUT_MS,
  framesReducer,
  isLocked,
  type FramesAction,
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
  /** 某支殼回報 ready（推最新快照給它；target＝已核對過的那支 iframe 視窗） */
  onReady: (key: string, target: Window) => void;
}

export function ShellFrames({ doc, shellKey, identity, registry, title, onReady }: ShellFramesProps) {
  const [state, dispatch] = useReducer(framesReducer, EMPTY_FRAMES);
  const elements = useRef(new Map<string, HTMLIFrameElement>());
  // 早到的 ready：送來時那支 iframe 還沒登記，先記下來源與是否載入失敗，登記時重核
  const early = useRef<{ token: string; source: MessageEventSource | null; failed: boolean } | null>(null);
  const stateRef = useRef(state);
  stateRef.current = state;
  const lockRef = useRef<HTMLDivElement | null>(null);
  const onReadyRef = useRef(onReady);
  onReadyRef.current = onReady;

  useLayoutEffect(() => {
    dispatch({ type: "shell", slot: doc === null ? null : { key: shellKey, doc, identity }, now: hostNow() });
  }, [doc, shellKey, identity]);

  // pending 回報 ready 或鎖定期逾時：翻面，或（新殼沒載好、舊殼載好過）保留舊殼只解鎖
  const settle = useCallback((key: string, action: FramesAction) => {
    const current = stateRef.current;
    if (current.pending?.key !== key) return;
    const next = framesReducer(current, action);
    const kept = next.front?.key !== key;
    const type = action.type === "timeout" ? (kept ? "timeout-keep" : "forced") : kept ? "failed-keep" : "flip";
    recordShellTiming({ type, key });
    dispatch(action);
  }, []);

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
          // 與一般 ready 同一套：先補推最新快照，再翻面（或保留舊殼）
          if (element.contentWindow !== null) onReadyRef.current(key, element.contentWindow);
          settle(key, { type: "ready", key, failed: held.failed });
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
        if (pending !== null && ready.token === pending.key) early.current = { token: ready.token, source: event.source, failed: ready.failed };
        return;
      }
      if (ready.token !== slot.key) return;
      recordShellTiming({ type: "ready", key: slot.key, role: slot === pending ? "pending" : "front", reason: ready.reason, failed: ready.failed, t: ready.t });
      if (shellTimingEnabled() && event.source !== null) {
        (event.source as Window).postMessage({ source: "table-tavern-host", kind: "ping", token: slot.key, t0: hostNow() }, "*");
      }
      onReadyRef.current(slot.key, event.source as Window);
      // front 的 ready 只記它載好了沒（決定之後新殼沒載好時值不值得留它）
      if (slot === pending) settle(slot.key, { type: "ready", key: slot.key, failed: ready.failed });
      else dispatch({ type: "ready", key: slot.key, failed: ready.failed });
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [settle]);

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

  // 鎖定期總上限：從鎖定起點算，換 pending 不重算；逾時保留舊殼後 lockedAt 為 null，不再計時
  const { lockedAt, pending, front } = state;
  const locked = isLocked(state);
  useEffect(() => {
    if (lockedAt === null || pending === null) return;
    const key = pending.key;
    const timer = window.setTimeout(() => settle(key, { type: "timeout" }), Math.max(0, lockedAt + FLIP_TIMEOUT_MS - hostNow()));
    return () => window.clearTimeout(timer);
  }, [lockedAt, pending, settle]);

  // 交給宿主訊息監聽的現況；鎖定時焦點若在舊殼裡就移到擋板，Enter／Space 不再送進舊殼
  useLayoutEffect(() => {
    const entry = (slot: FrameSlot | null) =>
      slot === null ? null : { key: slot.key, frame: elements.current.get(slot.key) ?? null };
    registry.front = entry(front);
    registry.pending = entry(pending);
    registry.locked = locked;
    if (locked && front !== null && document.activeElement === elements.current.get(front.key)) {
      lockRef.current?.focus();
    }
  }, [front, pending, locked, registry]);

  useEffect(
    () => () => {
      registry.front = null;
      registry.pending = null;
      registry.locked = false;
    },
    [registry],
  );

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
  const { key, doc } = slot;
  const loaded = useRef(false);
  const onLoad = (event: SyntheticEvent<HTMLIFrameElement>) => {
    // 只有第一次 load 是外層載好；之後的 load 不再送
    if (loaded.current) return;
    loaded.current = true;
    event.currentTarget.contentWindow?.postMessage({ source: "table-tavern-host", kind: "load", token: key, html: doc }, "*");
  };
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
      src={convertFileSrc("", "tt-sandbox")}
      onLoad={onLoad}
      title={title}
      tabIndex={role === "pending" || locked ? -1 : undefined}
      inert={role === "pending" || locked}
    />
  );
}
