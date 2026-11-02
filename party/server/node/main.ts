// 自架房間伺服器（Node ≥ 22.13；選項丙的 Node 薄殼）。規則全在 core/，行為對齊 workers/。
//
//   cd party && npm ci && npm run build && npm run build:node
//   PUBLIC_ORIGIN=https://party.example.com TRUST_PROXY=1 npm run start:node
//
// 環境變數：PORT（8787）、HOST（127.0.0.1；區網直連自用改 0.0.0.0）、PUBLIC_ORIGIN（邀請連結與成員 WS 來源比對，
// 不從 Host 標頭推）、TRUST_PROXY（1＝建房限流讀 X-Forwarded-For 最右一段）、ROOM_IDLE_DAYS（30，1–365）、
// DB_PATH（./data/rooms.sqlite，所有房間一個檔，備份它就好）、STATIC_DIR（party/dist）。
//
// 對外一律放在反向代理後，TLS 由代理處理（桌面版只接受 https，本機與私網 http 例外）。Caddy 範例：
//   party.example.com {
//     reverse_proxy 127.0.0.1:8787
//   }
// Caddy 會補寫 X-Forwarded-For 並轉送 WebSocket；nginx 要自己加 `proxy_set_header Upgrade/Connection` 與
// `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for`。TRUST_PROXY=1 的前提：Node 只聽 127.0.0.1
// （或被防火牆擋住），外部無法繞過代理直連，否則來源 IP 可被偽造。
//
// 授權 AGPL-3.0：拿改過的伺服器給別人用，就要向這些使用者提供你改過的原始碼。
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createPartyServer } from "./http";
import { Rooms } from "./rooms";
import { RoomDb } from "./sqlite-store";

const env = process.env;
const port = Number(env.PORT) || 8787;
const host = env.HOST || "127.0.0.1";
const publicOrigin = env.PUBLIC_ORIGIN ? new URL(env.PUBLIC_ORIGIN).origin : `http://localhost:${port}`;
const dbPath = env.DB_PATH || "./data/rooms.sqlite";
mkdirSync(dirname(dbPath), { recursive: true });

const db = new RoomDb(dbPath);
const rooms = new Rooms(db);
const server = createPartyServer(rooms, {
  publicOrigin,
  trustProxy: env.TRUST_PROXY === "1",
  idleDays: Math.min(365, Math.max(1, Math.trunc(Number(env.ROOM_IDLE_DAYS)) || 30)),
  staticDir: env.STATIC_DIR || fileURLToPath(new URL("../dist", import.meta.url)), // 打包後在 dist-node/，客戶端在旁邊的 dist/
});
rooms.restore();
server.listen(port, host, () => {
  console.log(`[party] listening on http://${host}:${port}, public origin ${publicOrigin}`);
  if (!env.PUBLIC_ORIGIN) console.warn("[party] PUBLIC_ORIGIN 未設：邀請連結與成員來源比對都用 localhost，對外部署前請設定");
  if (env.TRUST_PROXY !== "1") console.warn("[party] TRUST_PROXY 未設：放在反向代理後時所有人共用同一個建房限流額度");
});

let stopping = false;
async function stop() {
  if (stopping) return;
  stopping = true;
  server.close();
  await rooms.shutdown();
  db.db.close();
  process.exit(0);
}
process.on("SIGTERM", stop);
process.on("SIGINT", stop);
