// 房間連線的 React 外殼：開連線、套訊框、發指令（回覆交給呼叫的元件顯示在欄位旁）、草稿節流、往回翻頁。
import { useCallback, useEffect, useRef, useState } from "react";
import type { ErrorCode, Frame } from "../../../server/core/types";
import { RoomSocket, type Reply, type SocketLike, type SocketOptions } from "../connection/room-socket";
import { errorText } from "./error-text";
import { applyFrame, mergeStory, storyOfFrames, type RoomView } from "./room-state";

export interface RoomEnv {
  href: string;
  storage: Storage | null;
  open: (url: string) => SocketLike;
  fetch: typeof fetch;
  userAgent: string;
  replaceHref: (href: string) => void;
  retryDelays?: number[];
  timing?: SocketOptions["timing"];
}
export interface JoinParams {
  code: string;
  deviceId: string;
  joinSecret: string | null;
  pairCode: string | null;
  nickname: string;
}
export type Act = (t: string, p?: unknown) => Promise<Reply>;

const DRAFT_MS = 600; // 伺服器每 500 ms 收一則

/** 按鈕用：發指令、把錯誤留給元件顯示在旁邊（§8「欄位下紅字」）；成功回 result */
export function useRun(act: Act) {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (t: string, p: unknown = {}) => {
    const reply = await act(t, p);
    setError(reply.ok ? null : errorText(reply.code, reply.retryAfterMs));
    return reply.ok ? (reply.result ?? true) : null;
  }, [act]);
  return { run, error, setError };
}

export function wsUrl(href: string, code: string) {
  const url = new URL(href);
  return `${url.protocol === "https:" ? "wss" : "ws"}://${url.host}/api/rooms/${code}/ws?role=member`;
}

export function useRoom(env: RoomEnv, join: JoinParams, onJoined: () => void) {
  const [view, setView] = useState<RoomView | null>(null);
  const [status, setStatus] = useState<"connecting" | "online" | "offline">("connecting");
  const [fatal, setFatal] = useState<ErrorCode | null>(null);
  const [notice, setNotice] = useState<ErrorCode | null>(null); // 沒有請求對應的錯誤（例：加入時的配對碼）
  const viewRef = useRef<RoomView | null>(null);
  const socket = useRef<RoomSocket | null>(null);
  const draft = useRef({ last: 0, timer: null as ReturnType<typeof setTimeout> | null, text: "", gen: 0 });

  useEffect(() => {
    let joined = false;
    const s = new RoomSocket({
      url: wsUrl(env.href, join.code),
      open: env.open,
      retryDelays: env.retryDelays,
      timing: env.timing,
      lastSeq: () => viewRef.current?.seq ?? 0,
      hello: (lastSeq) => ({
        protocol: 1, role: "member", deviceId: join.deviceId, lastSeq, client: { name: "party", version: "1" },
        ...(join.joinSecret ? { joinSecret: join.joinSecret, nickname: join.nickname } : {}),
        ...(join.pairCode && !joined ? { pairCode: join.pairCode } : {}),
      }),
      onFrame: (f: Frame) => {
        if (f.t === "welcome" && !joined) {
          joined = true;
          onJoined();
        }
        viewRef.current = applyFrame(viewRef.current, f);
        setView(viewRef.current);
      },
      onNotice: (code) => setNotice(code),
      onStatus: (next, why) => {
        setStatus(next);
        if (why) setFatal(why);
      },
    });
    socket.current = s;
    s.connect();
    return () => s.stop();
    // 一次加入一條連線；參數變了（換房）才重開
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [join.code, join.deviceId]);

  const act: Act = useCallback(async (t, p = {}) => {
    const reply = (await socket.current?.request(t, p)) ?? { ok: false as const, code: "E_OFFLINE" as const };
    if (!reply.ok && reply.code === "E_STATE_CONFLICT") socket.current?.resync(); // 以伺服器畫面為準，不另跳窗
    return reply;
  }, []);

  /** 草稿：最多每 600 ms 送一則、永遠送最新的字；被限流就照伺服器說的時間延後（靜默） */
  const sendDraft = useCallback((text: string) => {
    const d = draft.current;
    d.text = text;
    if (d.timer) return;
    const gen = d.gen;
    const fire = async () => {
      d.timer = null;
      d.last = Date.now();
      const sent = d.text;
      const reply = await act("action.draft", { text: sent });
      if (gen !== d.gen) return; // 期間送出或跳過了：別再補送舊字
      if (!reply.ok && reply.code === "E_RATE_LIMITED") d.timer = setTimeout(fire, reply.retryAfterMs ?? DRAFT_MS);
      else if (d.text !== sent) sendDraft(d.text);
    };
    const wait = d.last + DRAFT_MS - Date.now();
    if (wait <= 0) void fire();
    else d.timer = setTimeout(fire, wait);
  }, [act]);

  /** 送出、跳過前呼叫：取消待送的草稿，免得晚到的草稿把剛送出的行動又存回伺服器草稿 */
  const cancelDraft = useCallback(() => {
    const d = draft.current;
    if (d.timer) clearTimeout(d.timer);
    Object.assign(d, { timer: null, text: "", gen: d.gen + 1 });
  }, []);

  const loadOlder = useCallback(async (beforeSeq: number) => {
    const reply = await act("history.page", { beforeSeq, limit: 200 });
    if (!reply.ok || !viewRef.current) return 0;
    viewRef.current = { ...viewRef.current, story: mergeStory(viewRef.current.story, storyOfFrames(reply.result.events), true) };
    setView(viewRef.current);
    return reply.result.before as number;
  }, [act]);

  const resumed = useCallback((round: number) => {
    viewRef.current = applyFrame(viewRef.current, { t: "vol", p: { type: "resumed", round } });
    setView(viewRef.current);
  }, []);

  return { view, status, fatal, notice, clearNotice: () => setNotice(null), act, sendDraft, cancelDraft, loadOlder, resumed, leave: () => socket.current?.stop() };
}
