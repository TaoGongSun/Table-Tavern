// 房間畫面（party 元件）的 WebSocket 換成經 Rust 轉手的一條管子（room-host-desktop-play §4.2）：桌面版 CSP 不放
// 房間伺服器，網址也由 Rust 用執行端憑證組，這裡只把 open／msg／close 事件接回 SocketLike 的介面。
import { Channel, invoke } from "@tauri-apps/api/core";
import type { SocketLike } from "@party/features/connection/room-socket";

type Event = { k: "open" } | { k: "msg"; d: string } | { k: "close"; code: number };

export function ipcSocket(): SocketLike {
  let id: number | null = null;
  let closing: number | null = null;
  const socket: SocketLike = {
    readyState: 0,
    onopen: null,
    onmessage: null,
    onclose: null,
    send(text) {
      if (id !== null) void invoke("room_member_send", { id, text }).catch(() => {});
    },
    close(code = 1000) {
      if (socket.readyState >= 2) return;
      socket.readyState = 2;
      if (id === null) closing = code;
      else void invoke("room_member_close", { id, code }).catch(() => {});
    },
  };
  const channel = new Channel<Event>();
  channel.onmessage = (event) => {
    if (event.k === "open") {
      if (socket.readyState !== 0) return;
      socket.readyState = 1;
      socket.onopen?.({});
    } else if (event.k === "msg") socket.onmessage?.({ data: event.d });
    else {
      socket.readyState = 3;
      socket.onclose?.({ code: event.code });
    }
  };
  invoke<number>("room_member_open", { onEvent: channel }).then(
    (opened) => {
      id = opened;
      if (closing !== null) void invoke("room_member_close", { id, code: closing }).catch(() => {});
    },
    () => {
      socket.readyState = 3;
      socket.onclose?.({ code: 1006 });
    },
  );
  return socket;
}
