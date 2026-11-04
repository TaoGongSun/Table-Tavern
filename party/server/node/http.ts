// Node 殼的 HTTP 進入點：照 workers/index.ts 的路由（建房限流＋重產碼、房間路徑、匯出、成員 WS 驗來源），
// 再加 party/dist 靜態檔（區網自用沒有反向代理時，手機也要從這台載客戶端）。路由約 30 行與 Workers 版重複，
// 刻意不抽象：Request 與 IncomingMessage 兩套型別，抽了比重複長。
import { readFile, realpath, stat } from "node:fs/promises";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { Duplex } from "node:stream";
import { extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { WebSocketServer } from "ws";
import { CROCKFORD8, crockford } from "../core/secrets";
import { receiveUpload } from "../core/upload";
import { guard, Rooms } from "./rooms";

export interface ServerConfig {
  publicOrigin: string;
  trustProxy: boolean;
  idleDays: number;
  staticDir: string;
}

const ROOM_PATH = /^\/api\/rooms\/([0-9A-Za-z]{8})(?:\/(ws|export|cards)(?:\/([0-9A-Z]{26}))?)?$/;
const CREATE_PER_HOUR = 10; // 協定 §6.1：同 IP 每小時 10 次（Workers 只能近似每分鐘 1 次）
const HOUR = 3_600_000;
const MIME: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".webp": "image/webp",
  ".woff2": "font/woff2",
  ".webmanifest": "application/manifest+json",
  ".txt": "text/plain; charset=utf-8",
};

const json = (res: ServerResponse, status: number, body: unknown) => {
  res.writeHead(status, { "content-type": "application/json" }).end(JSON.stringify(body));
};

export function createPartyServer(rooms: Rooms, cfg: ServerConfig) {
  const created = new Map<string, number[]>();
  const wss = new WebSocketServer({ noServer: true, maxPayload: 1024 * 1024 }); // 平台上限同 Workers；成員 16 KiB 由核心擋

  const clientIp = (req: IncomingMessage) => {
    // 只有 Node 被可信代理擋在前面（外部連不到 Node）時，最右一段才是代理補寫的真實來源
    const fwd = cfg.trustProxy
      ? String(req.headers["x-forwarded-for"] ?? "")
          .split(",")
          .pop()
          ?.trim()
      : "";
    return fwd || req.socket.remoteAddress || "unknown";
  };
  const limited = (ip: string, now: number) => {
    const hits = (created.get(ip) ?? []).filter((t) => t > now - HOUR);
    created.set(ip, hits);
    if (hits.length >= CREATE_PER_HOUR) return hits[0] + HOUR - now;
    hits.push(now);
    return 0;
  };
  const pruneLimiter = () => {
    for (const [ip, hits] of created)
      if (!hits.some((t) => t > rooms.now() - HOUR)) created.delete(ip);
  };

  async function readBody(req: IncomingMessage, max: number): Promise<string | null> {
    let size = 0;
    const chunks: Buffer[] = [];
    for await (const c of req as AsyncIterable<Buffer>) {
      size += c.length;
      if (size > max) return null;
      chunks.push(c);
    }
    return Buffer.concat(chunks).toString("utf8");
  }

  async function handle(req: IncomingMessage, res: ServerResponse) {
    const url = new URL(req.url ?? "/", cfg.publicOrigin);
    if (url.pathname === "/api/rooms" && req.method === "POST") {
      const wait = limited(clientIp(req), rooms.now());
      if (wait) return json(res, 429, { code: "E_RATE_LIMITED", retryAfterMs: wait });
      const body = await readBody(req, 16 * 1024);
      if (body === null) {
        json(res, 400, { code: "E_BAD_FRAME" });
        return void req.destroy();
      }
      const parsed = (() => {
        try {
          return JSON.parse(body) as unknown;
        } catch {
          return null;
        }
      })();
      for (let i = 0; i < 5; i++) {
        const code = crockford(crypto.getRandomValues(new Uint8Array(8)), 8);
        const out = await rooms.with(code, (r) =>
          r.run({
            kind: "http",
            route: "create",
            body: parsed,
            code,
            origin: cfg.publicOrigin,
            idleDays: cfg.idleDays,
          }),
        );
        if (out?.http?.status !== 409)
          return json(res, out?.http?.status ?? 500, out?.http?.body ?? { code: "E_INTERNAL" });
      }
      return json(res, 503, { code: "E_INTERNAL" });
    }
    if (url.pathname.startsWith("/api/")) {
      const m = url.pathname.match(ROOM_PATH);
      const code = m?.[1].toUpperCase() ?? "";
      if (!m || !CROCKFORD8.test(code)) return json(res, 404, { code: "E_ROOM_NOT_FOUND" });
      if (m[2] === "ws") return json(res, 426, { code: "E_BAD_FRAME" });
      if (m[3] !== undefined && m[2] !== "cards")
        return json(res, 404, { code: "E_ROOM_NOT_FOUND" });
      const bearer = String(req.headers.authorization ?? "").replace(/^Bearer\s+/i, "");
      if (m[2] === "cards" && m[3] !== undefined) {
        if (req.method !== "GET") return json(res, 405, { code: "E_BAD_FRAME" });
        const out = await rooms.with(code, (r) =>
          r.run({ kind: "http", route: "card_fetch", executorToken: bearer, uploadId: m[3] }),
        );
        if (!out?.http?.bytes)
          return json(res, out?.http?.status ?? 500, out?.http?.body ?? { code: "E_INTERNAL" });
        return void res
          .writeHead(200, { "content-type": "application/octet-stream" })
          .end(out.http.bytes);
      }
      if (m[2] === "cards") {
        if (req.method !== "POST") return json(res, 405, { code: "E_BAD_FRAME" });
        const origin = req.headers.origin ?? null;
        const reply = await rooms.with(code, (r) =>
          receiveUpload(r.runner, {
            kind: url.searchParams.get("kind"),
            contentLength: req.headers["content-length"] ?? null,
            origin,
            selfOrigin: cfg.publicOrigin,
            deviceId: bearer,
            body: req,
          }),
        );
        if (!req.complete) res.once("finish", () => req.destroy()); // 宣告超大或沒宣告長度的本體不讀，回完就斷
        return json(res, reply.status, reply.body);
      }
      if (m[2] === undefined) {
        if (req.method !== "DELETE") return json(res, 405, { code: "E_BAD_FRAME" });
        const out = await rooms.with(code, (r) =>
          r.run({ kind: "http", route: "close", executorToken: bearer }),
        );
        return json(res, out?.http?.status ?? 500, out?.http?.body ?? { code: "E_INTERNAL" });
      }
      if (req.method !== "GET") return json(res, 405, { code: "E_BAD_FRAME" });
      const out = await rooms.with(code, (r) =>
        r.run({ kind: "http", route: "export", deviceId: bearer }),
      );
      return json(res, out?.http?.status ?? 500, out?.http?.body ?? { code: "E_INTERNAL" });
    }
    if (req.method !== "GET" && req.method !== "HEAD")
      return json(res, 405, { code: "E_BAD_FRAME" });
    return serveStatic(url.pathname, req, res);
  }

  async function serveStatic(pathname: string, req: IncomingMessage, res: ServerResponse) {
    let decoded: string;
    try {
      decoded = decodeURIComponent(pathname);
    } catch {
      return json(res, 400, { code: "E_BAD_FRAME" });
    }
    // 按路徑分段判斷（第一段是 .. 或跑到別的磁碟就是穿越）；字面路徑與 realpath 後的實際目標都要在根裡，擋符號連結
    const root = await realpath(cfg.staticDir);
    const inside = (p: string) => {
      const rel = relative(root, p);
      return rel.split(sep)[0] !== ".." && !isAbsolute(rel);
    };
    const file = resolve(root, "." + decoded);
    if (decoded.includes("\0") || !inside(file)) return json(res, 404, { code: "E_NOT_FOUND" });
    const real = await realpath(file).catch(() => null);
    if (real && !inside(real)) return json(res, 404, { code: "E_NOT_FOUND" });
    const isFile = real !== null && real !== root && (await stat(real).catch(() => null))?.isFile();
    const index = await realpath(join(root, "index.html")).catch(() => null);
    const target = isFile ? real : index && inside(index) ? index : null; // /r/<房間碼> 這類路徑回單頁應用
    const body = target && (await readFile(target).catch(() => null));
    if (!body) return json(res, 404, { code: "E_NOT_FOUND" });
    const rel = relative(root, target);
    const immutable = isFile && rel.split(sep)[0] === "assets";
    res.writeHead(200, {
      "content-type": MIME[extname(target)] ?? "application/octet-stream",
      "cache-control": immutable ? "public, max-age=31536000, immutable" : "no-cache",
    });
    res.end(req.method === "HEAD" ? undefined : body);
  }

  function upgrade(req: IncomingMessage, socket: Duplex, head: Buffer) {
    const reject = (status: number, code: string) => {
      const body = JSON.stringify({ code });
      socket.end(
        `HTTP/1.1 ${status} ${code}\r\ncontent-type: application/json\r\ncontent-length: ${body.length}\r\nconnection: close\r\n\r\n${body}`,
      );
    };
    const url = new URL(req.url ?? "/", cfg.publicOrigin);
    const m = url.pathname.match(ROOM_PATH);
    const code = m?.[1].toUpperCase() ?? "";
    if (!m || m[2] !== "ws" || !CROCKFORD8.test(code)) return reject(404, "E_ROOM_NOT_FOUND");
    const role = url.searchParams.get("role") === "executor" ? "executor" : "member";
    // 成員連線只收 party/ 自己的來源；執行端（桌面版）不看來源、只看憑證
    const origin = req.headers.origin;
    if (role === "member" && origin !== undefined && origin !== cfg.publicOrigin)
      return reject(403, "E_FORBIDDEN");
    wss.handleUpgrade(req, socket, head, (ws) => guard(rooms.attach(code, ws, role)));
  }

  const server = createServer((req, res) => {
    handle(req, res).catch((e) => {
      console.error("[party] http failed:", e);
      if (!res.headersSent) json(res, 500, { code: "E_INTERNAL" });
      else res.destroy();
    });
  });
  server.on("upgrade", (req, socket, head) => {
    socket.on("error", () => socket.destroy());
    try {
      upgrade(req, socket, head);
    } catch (e) {
      console.error("[party] upgrade failed:", e);
      socket.destroy();
    }
  });
  const sweeper = setInterval(() => {
    rooms.sweepAll();
    pruneLimiter();
  }, 15_000);
  sweeper.unref();
  server.on("close", () => clearInterval(sweeper));
  return server;
}
