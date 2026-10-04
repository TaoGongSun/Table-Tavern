// 測試通道用的本機假 OpenRouter：智慧免費（穩定免費）的試打、派送、/key、模型清單都能逐請求腳本化。
// 用法：node scripts/harness-fake-openrouter.mjs <script.json> [--port N]
// 起來後印出網址，再 `node scripts/harness.mjs openrouter-origin <網址>` 讓 app 改打這裡（只有 harness 包有這個入口）。
//
// script.json：
// {
//   "routes": {
//     "POST /chat/completions": [ <回應>, ... ],   // 依序取用，用完重複最後一個
//     "GET /key": [ { "json": { "data": { "free_model_daily_requests": { "limit": 50, "remaining": 40 } } } } ],
//     "GET /models/*/endpoints": [ ... ]           // * 比對單一路徑段以外的任意字元（含 /）
//   },
//   "fallback": { "status": 404, "json": { "error": { "code": 404, "message": "no route" } } }
// }
// <回應>：{ "status": 200, "headers": {}, "json": {...} | "body": "原文" | "sse": [payload, ...], "delayMs": 0 }
//   sse 的每個 payload 是物件（序列化成 JSON）或字串（原樣，例如 "[DONE]"）；model 欄可用 "$model" 代入請求的 model。
// GET /__log 回目前收到的請求（method、path、model），供驗收比對。
import fs from "node:fs";
import http from "node:http";

const [scriptPath, ...rest] = process.argv.slice(2);
if (!scriptPath) {
  console.error("用法：node scripts/harness-fake-openrouter.mjs <script.json> [--port N]");
  process.exit(1);
}
const portFlag = rest.indexOf("--port");
const port = portFlag >= 0 ? Number(rest[portFlag + 1]) : 0;
const script = JSON.parse(fs.readFileSync(scriptPath, "utf8"));
const queues = new Map(Object.entries(script.routes ?? {}).map(([key, list]) => [key, [...list]]));
const log = [];

function matchRoute(method, path) {
  const exact = `${method} ${path}`;
  if (queues.has(exact)) return exact;
  for (const key of queues.keys()) {
    const [keyMethod, pattern] = key.split(" ");
    if (keyMethod !== method || !pattern.includes("*")) continue;
    const regex = new RegExp(`^${pattern.split("*").map((part) => part.replace(/[.+?^${}()|[\]\\]/g, "\\$&")).join(".+")}$`);
    if (regex.test(path)) return key;
  }
  return null;
}

function nextResponse(key) {
  const queue = queues.get(key);
  return queue.length > 1 ? queue.shift() : queue[0];
}

function substitute(value, model) {
  return JSON.parse(JSON.stringify(value).replaceAll('"$model"', JSON.stringify(model ?? null)));
}

const server = http.createServer((request, response) => {
  let raw = "";
  request.on("data", (chunk) => (raw += chunk));
  request.on("end", async () => {
    const path = new URL(request.url, "http://local").pathname.replace(/^\/api\/v1/, "");
    if (request.method === "GET" && path === "/__log") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(log));
      return;
    }
    let model = null;
    try {
      model = JSON.parse(raw || "{}").model ?? null;
    } catch {}
    log.push({ method: request.method, path, model });
    const key = matchRoute(request.method, path);
    const reply = key ? nextResponse(key) : (script.fallback ?? { status: 404, json: { error: { code: 404, message: "no route" } } });
    if (reply.delayMs) await new Promise((resolve) => setTimeout(resolve, reply.delayMs));
    const headers = { ...(reply.headers ?? {}) };
    if (reply.sse) {
      response.writeHead(reply.status ?? 200, { "content-type": "text/event-stream", ...headers });
      for (const payload of reply.sse) {
        const text = typeof payload === "string" ? payload : JSON.stringify(substitute(payload, model));
        response.write(`data: ${text}\n\n`);
      }
      response.end();
      return;
    }
    const body = reply.json !== undefined ? JSON.stringify(substitute(reply.json, model)) : (reply.body ?? "");
    if (reply.json !== undefined) headers["content-type"] ??= "application/json";
    response.writeHead(reply.status ?? 200, headers);
    response.end(body);
  });
});

server.listen(port, "127.0.0.1", () => {
  console.log(`http://127.0.0.1:${server.address().port}/api/v1`);
});
