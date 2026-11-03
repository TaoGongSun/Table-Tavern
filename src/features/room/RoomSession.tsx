// 電腦的房間成員連線本體（room-host-desktop-play）：掛在 app 最外層、不畫東西，只把最新狀態交給 useDesktopRoom。
import { useEffect, useRef } from "react";
import { useRoomSession } from "@party/App";
import { ipcSocket } from "./ipc-socket";
import type { DesktopRoomCtl } from "./useDesktopRoom";

const ENV_BASE = {
  storage: null,
  open: () => ipcSocket(),
  fetch: (() => Promise.reject(new Error("no fetch"))) as typeof fetch,
  userAgent: navigator.userAgent,
  replaceHref: () => {},
};

function Session({ ctl }: { ctl: DesktopRoomCtl }) {
  const join = ctl.live!;
  const env = { ...ENV_BASE, href: `${join.origin}/r/${join.invite.code}` };
  const room = useRoomSession(env, join.invite, join.device, ctl.nickname);
  const latest = useRef(room);
  latest.current = room;
  const { setRoom } = ctl;
  useEffect(() => setRoom(latest.current), [setRoom, room.view, room.status, room.fatal, room.notice]);
  useEffect(() => () => setRoom(null), [setRoom]);
  return null;
}

export function RoomSession({ ctl }: { ctl: DesktopRoomCtl }) {
  return ctl.live ? <Session key={ctl.live.key} ctl={ctl} /> : null;
}
