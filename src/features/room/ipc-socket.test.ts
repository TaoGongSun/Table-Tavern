// 房間畫面的 WebSocket 轉接：Rust 事件依序變成 SocketLike 的 onopen／onmessage／onclose；開好之前就關也照樣送出關閉
import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: Array<[string, Record<string, unknown>]> = [];
let channel: { onmessage: (event: unknown) => void } | null = null;
let resolveOpen: (id: number) => void = () => {};

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {};
    constructor() {
      channel = this;
    }
  },
  invoke: (command: string, args: Record<string, unknown>) => {
    calls.push([command, args]);
    if (command === "room_member_open") return new Promise<number>((resolve) => (resolveOpen = resolve));
    return Promise.resolve();
  },
}));

const { ipcSocket } = await import("./ipc-socket");
const { pairCodeOf } = await import("./room-status");

beforeEach(() => {
  calls.length = 0;
});

describe("ipcSocket", () => {
  it("open → msg → close 依序轉成 SocketLike 事件，送出帶連線編號", async () => {
    const socket = ipcSocket();
    const seen: string[] = [];
    socket.onopen = () => seen.push("open");
    socket.onmessage = (ev) => seen.push(`msg:${ev.data}`);
    socket.onclose = (ev) => seen.push(`close:${ev.code}`);
    resolveOpen(7);
    await Promise.resolve();
    channel!.onmessage({ k: "open" });
    expect(socket.readyState).toBe(1);
    socket.send("hello");
    channel!.onmessage({ k: "msg", d: "welcome" });
    channel!.onmessage({ k: "close", code: 4410 });
    expect(seen).toEqual(["open", "msg:welcome", "close:4410"]);
    expect(socket.readyState).toBe(3);
    expect(calls).toContainEqual(["room_member_send", { id: 7, text: "hello" }]);
  });

  it("連線編號還沒回來就關：拿到編號後補送關閉", async () => {
    const socket = ipcSocket();
    socket.close(4000);
    resolveOpen(9);
    await Promise.resolve();
    await Promise.resolve();
    expect(calls).toContainEqual(["room_member_close", { id: 9, code: 4000 }]);
  });
});

describe("pairCodeOf", () => {
  it("從附碼連結取出配對碼；沒有就 null", () => {
    expect(pairCodeOf("https://x.example/r/ABCD2345#k=secret&p=WXYZ2345")).toBe("WXYZ2345");
    expect(pairCodeOf("https://x.example/r/ABCD2345#k=secret")).toBeNull();
    expect(pairCodeOf(null)).toBeNull();
  });
});
