// 隔離 Worker 的入口（mvu-zod-semantics 規格 4 節）：由 mvu-zod-engine-plugin.ts 打成 classic IIFE 文字（內含 wasm 與
// 桌面同一支 bundle），主頁交給不透明來源的 iframe（public/mvu-zod.html）建 blob Worker。開機時建 QuickJS 宿主
// （quickjs-host.ts）、載入 bundle 才回 ready；卡片 schema 原文要等派工（在期限內）才當字串送進 QuickJS。卡片碰不到
// 這支 Worker 的任何全域。任何失敗後不再接工（主頁會回收整支）。對外只走 iframe 轉交的 MessagePort。
import { bundle, wasm } from "virtual:mvu-quickjs-assets";
import type { ZodBootReply, ZodReply, ZodRequest } from "./protocol";
import { createQuickJsHost, decodeWasm, EngineError, type QuickJsHost } from "./quickjs-host";

/** 開機與單次派工的期限（主頁另有自己的期限並會 terminate；這裡讓 QuickJS 先中斷） */
const BOOT_MS = 10_000;
const DEADLINE_MS = 5_000;

const scope = globalThis as unknown as {
  addEventListener: (type: "message", listener: (event: MessageEvent) => void) => void;
  postMessage: (message: unknown) => void;
};
let port: MessagePort | null = null;
let host: QuickJsHost | null = null;
let dead = false;
const send = (message: ZodBootReply | ZodReply) => port?.postMessage(message);

function handle(request: ZodRequest): void {
  const reply = (kind: EngineError["kind"], detail: string) => send({ id: request.id, ok: false, kind, detail: detail.slice(0, 600) });
  if (dead || !host) return reply("crashed", "引擎已失效");
  if (typeof request.schema !== "string") return reply("bad-request", "schema");
  const deadline = performance.now() + DEADLINE_MS;
  try {
    host.load(request.schema, deadline);
    if (request.op === "parse") {
      send({ id: request.id, ok: true, json: host.call("parse", [request.message, request.old, request.macros], deadline) });
    } else if (request.op === "initialize") {
      send({ id: request.id, ok: true, json: host.call("initialize", [request.stat], deadline) });
    } else {
      reply("bad-request", "op");
    }
  } catch (error) {
    dead = true;
    // 附上當時的 wasm 線性記憶體大小（上限 128 MB；e2e 拿來確認封頂）
    const memory = `（線性記憶體 ${Math.round(host.memoryBytes / 2 ** 20)} MB）`;
    if (error instanceof EngineError) reply(error.kind, `${error.message.slice(0, 500)}${memory}`);
    else reply("crashed", `${String(error instanceof Error ? error.message : error).slice(0, 500)}${memory}`);
  }
}

// iframe 開機後轉交 MessagePort（只收第一次）；先告訴 iframe 已啟動，它就能收回引擎的 blob 網址
scope.addEventListener("message", (event: MessageEvent) => {
  if (port !== null || !(event.ports?.[0] instanceof MessagePort)) return;
  port = event.ports[0];
  port.onmessage = (message: MessageEvent) => handle(message.data as ZodRequest);
  createQuickJsHost({ wasm: decodeWasm(wasm), bundle }, performance.now() + BOOT_MS).then(
    (created) => {
      host = created;
      send({ type: "ready" });
    },
    (error: unknown) => {
      dead = true;
      send({ type: "boot-failed", detail: String(error instanceof Error ? error.message : error).slice(0, 500) });
    },
  );
});
scope.postMessage("up");
