// Worker 進入點（協定 §4.1、§12.1）：建房（限流＋產房間碼）、把房間路徑轉給該房的 DO。不持有任何金鑰。
import { CROCKFORD8, crockford } from "../core/secrets";
import type { Env } from "./room-do";

export { RoomDO } from "./room-do";

const json = (body: unknown, status: number) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
const ROOM_PATH = /^\/api\/rooms\/([0-9A-Za-z]{8})(?:\/(ws|export|cards)(?:\/([0-9A-Z]{26}))?)?$/;

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    if (url.pathname === "/api/rooms" && req.method === "POST") {
      // 核心不碰 IP：建房限流由 Rate Limiting binding 擋（binding 的週期只有 10／60 秒，見 wrangler.toml）
      const ip = req.headers.get("CF-Connecting-IP") ?? "unknown";
      if (env.CREATE_LIMITER && !(await env.CREATE_LIMITER.limit({ key: ip })).success) return json({ code: "E_RATE_LIMITED", retryAfterMs: 60_000 }, 429);
      const body = await req.text();
      if (body.length > 16 * 1024) return json({ code: "E_BAD_FRAME" }, 400);
      for (let i = 0; i < 5; i++) {
        // 亂數產碼 → idFromName → DO 內 room 表為空才建，否則（409）重產
        const code = crockford(crypto.getRandomValues(new Uint8Array(8)), 8);
        const headers = { "content-type": "application/json", "x-room-code": code, "x-origin": url.origin };
        const res = await env.ROOM.get(env.ROOM.idFromName(code)).fetch("https://room/create", { method: "POST", body, headers });
        if (res.status !== 409) return res;
      }
      return json({ code: "E_INTERNAL" }, 503);
    }
    const m = url.pathname.match(ROOM_PATH);
    const code = m?.[1].toUpperCase() ?? "";
    if (!m || !CROCKFORD8.test(code)) return json({ code: "E_ROOM_NOT_FOUND" }, 404);
    if (m[3] !== undefined && m[2] !== "cards") return json({ code: "E_ROOM_NOT_FOUND" }, 404);
    if (m[2] === "cards" && req.method !== (m[3] === undefined ? "POST" : "GET")) return json({ code: "E_BAD_FRAME" }, 405); // 上傳／執行端取檔
    if (m[2] === "export" && req.method !== "GET") return json({ code: "E_BAD_FRAME" }, 405);
    if (m[2] === undefined && req.method !== "DELETE") return json({ code: "E_BAD_FRAME" }, 405); // 結束房間（執行端憑證）
    // 成員連線只收 party/ 自己的來源；執行端（桌面版）不看來源、只看憑證
    const origin = req.headers.get("Origin");
    if (m[2] === "ws" && url.searchParams.get("role") !== "executor" && origin !== null && origin !== url.origin) return json({ code: "E_FORBIDDEN" }, 403);
    return env.ROOM.get(env.ROOM.idFromName(code)).fetch(req);
  },
} satisfies ExportedHandler<Env>;
