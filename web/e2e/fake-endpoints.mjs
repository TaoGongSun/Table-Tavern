// 端對端用的本機假端點：OpenRouter（授權頁、換金鑰、/key、模型清單與排行、上游、串流聊天）與
// GitHub Releases API。全部帶 CORS *，行為由回傳的 state 物件即時切換。不連任何外部服務。
import { createHash } from "node:crypto";
import http from "node:http";

const DAY = 86_400;
const OLD = Math.floor(Date.now() / 1000) - 30 * DAY;

function model(id, slug) {
  return {
    id,
    canonical_slug: slug,
    name: id,
    created: OLD,
    context_length: 65_536,
    pricing: { prompt: "0", completion: "0" },
    architecture: { input_modalities: ["text"], output_modalities: ["text"] },
  };
}

const MODELS = [model("alpha/one:free", "alpha/one"), model("beta/two:free", "beta/two")];

const RELEASE = {
  tag_name: "v0.3.0",
  html_url: "https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.3.0",
  assets: [
    { name: "TableTavern_0.3.0_x64-setup.exe", browser_download_url: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_x64-setup.exe" },
    { name: "TableTavern_0.3.0_aarch64.dmg", browser_download_url: "https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern_0.3.0_aarch64.dmg" },
  ],
};

export const E2E_KEY = "sk-or-v1-e2e-local-test";

const base64Url = (buffer) => buffer.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

export async function startFake() {
  const state = {
    remaining: 40,
    /** normal：分段吐完；numbered：回「第 N 次回覆」（N＝第幾個聊天請求）；hang：送出標頭後不吐任何字；partial-hang：吐一段後卡住 */
    chat: "normal",
    release: "none",
    challenge: null,
    exchanges: [],
    chatRequests: [],
  };
  const sockets = new Set();

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, "http://local");
    const cors = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Headers": "authorization, content-type, x-title, accept",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    };
    const json = (status, body) => {
      res.writeHead(status, { ...cors, "Content-Type": "application/json" });
      res.end(JSON.stringify(body));
    };
    if (req.method === "OPTIONS") {
      res.writeHead(204, cors);
      res.end();
      return;
    }
    const body = await new Promise((resolve) => {
      let data = "";
      req.on("data", (chunk) => (data += chunk));
      req.on("end", () => resolve(data));
    });
    const authorized = req.headers.authorization === `Bearer ${E2E_KEY}`;
    const path = url.pathname;

    if (path === "/auth") {
      state.challenge = url.searchParams.get("code_challenge");
      const callback = new URL(url.searchParams.get("callback_url"));
      callback.searchParams.set("code", "fake-code");
      res.writeHead(302, { Location: callback.toString() });
      res.end();
      return;
    }
    if (path === "/api/v1/auth/keys" && req.method === "POST") {
      const payload = JSON.parse(body || "{}");
      state.exchanges.push(payload);
      const ok =
        payload.code === "fake-code" &&
        payload.code_challenge_method === "S256" &&
        base64Url(createHash("sha256").update(payload.code_verifier ?? "").digest()) === state.challenge;
      json(ok ? 200 : 403, ok ? { key: E2E_KEY } : { error: { code: 403, message: "bad verifier" } });
      return;
    }
    if (path === "/api/v1/key") {
      if (!authorized) return json(401, { error: { code: 401 } });
      return json(200, { data: { free_model_daily_requests: { limit: 50, remaining: state.remaining } } });
    }
    if (path === "/api/v1/models/user") {
      if (!authorized) return json(401, { error: { code: 401 } });
      return json(200, { data: MODELS });
    }
    if (path === "/api/v1/models") {
      if (url.searchParams.get("category") === "roleplay") return json(200, { data: [{ id: "alpha/one", canonical_slug: "alpha/one" }] });
      return json(200, { data: MODELS.map(({ id }) => ({ id })) });
    }
    const endpoints = path.match(/^\/api\/v1\/models\/(.+)\/endpoints$/);
    if (endpoints) {
      const provider = decodeURIComponent(endpoints[1]).startsWith("alpha") ? "ProviderA" : "ProviderB";
      return json(200, { data: { endpoints: [{ provider_name: provider }] } });
    }
    if (path === "/api/v1/chat/completions" && req.method === "POST") {
      if (!authorized) return json(401, { error: { code: 401 } });
      const payload = JSON.parse(body || "{}");
      state.chatRequests.push(payload);
      res.writeHead(200, { ...cors, "Content-Type": "text/event-stream" });
      const send = (delta) => res.write(`data: ${JSON.stringify({ model: payload.model, choices: [{ delta }] })}\n\n`);
      const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      if (state.chat === "hang") return; // 只送標頭，等客戶端取消
      if (state.chat === "partial-hang") {
        send({ content: "說到一半" });
        return;
      }
      const parts = state.chat === "numbered" ? [`第 ${state.chatRequests.length} 次`, "回覆"] : ["雪還在下，", "先喝口湯", "暖暖身子吧。"];
      for (const part of parts) {
        send({ content: part });
        await pause(120);
      }
      res.write(`data: ${JSON.stringify({ choices: [{ delta: {}, finish_reason: "stop" }] })}\n\n`);
      res.end("data: [DONE]\n\n");
      return;
    }
    if (path === "/github/repos/TaoGongSun/Table-Tavern/releases/latest") {
      return state.release === "none" ? json(404, { message: "Not Found" }) : json(200, RELEASE);
    }
    json(404, { error: { code: 404, message: `no route ${req.method} ${path}` } });
  });
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    url: `http://127.0.0.1:${port}`,
    state,
    close: () =>
      new Promise((resolve) => {
        for (const socket of sockets) socket.destroy();
        server.close(resolve);
      }),
  };
}
