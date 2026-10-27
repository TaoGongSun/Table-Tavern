// 隔離 Worker 的入口（mvu-zod-semantics 規格 4 節）：由 mvu-zod-engine-plugin.ts 打成 classic IIFE 文字，主頁交給
// 不透明來源的 iframe（public/mvu-zod.html）建 blob Worker。開機時先抓住要用的宿主函式，再剝全域、確認危險能力都看不到
// 才回 ready；卡片腳本要等派工（在 5 秒期限內）才 import。對外只走 iframe 轉交的 MessagePort。
import { cardGlobals, createZodEngine, type Macros } from "./engine-core";
import type { ZodBootReply, ZodReply, ZodRequest } from "./protocol";
import { leftoverCapabilities, stripGlobals } from "./strip";

const scope = globalThis as unknown as Record<string, unknown> & typeof globalThis;
// 卡片看不到的閉包：blob 網址、序列化、計時、回覆
const createUrl = URL.createObjectURL.bind(URL);
const revokeUrl = URL.revokeObjectURL.bind(URL);
const BlobCtor = Blob;
const { stringify, parse } = JSON;
const later = setTimeout;
const importModule = (url: string): Promise<unknown> => import(/* @vite-ignore */ url);
const engine = createZodEngine();
const SHIM_KEY = "__ttRegisterMvuSchema";

let port: MessagePort | null = null;
const send = (message: ZodBootReply | ZodReply) => port?.postMessage(message);

/** 卡片模組跑完後，等它非頂層的 import 收尾（例：`import(墊片).then(m => m.registerMvuSchema(S))`）最多這麼多輪 */
const REGISTER_TURNS = 100;

let loaded: Promise<void> | null = null;
async function loadCard(parts: string[]): Promise<void> {
  // 墊片第一次執行時才取走註冊函式；墊片的 blob 網址不在這裡收回（非頂層的動態 import 可能晚點才載墊片），
  // 跟著整支 Worker 回收時一起消失
  scope[SHIM_KEY] = engine.register;
  const shim = createUrl(
    new BlobCtor([`const r = globalThis.${SHIM_KEY};\ndelete globalThis.${SHIM_KEY};\nexport function registerMvuSchema(input) { return r(input); }\n`], {
      type: "text/javascript",
    }),
  );
  const card = createUrl(new BlobCtor([parts.join(stringify(shim))], { type: "text/javascript" }));
  try {
    await importModule(card);
  } finally {
    revokeUrl(card);
  }
  for (let turn = 0; turn < REGISTER_TURNS && !engine.status().registered; turn++) {
    await new Promise((resolve) => later(resolve, 0));
  }
  if (!engine.status().registered) throw new Error("卡片 schema 腳本沒有呼叫 registerMvuSchema");
}

async function handle(request: ZodRequest): Promise<void> {
  const fail = (kind: "schema-load" | "script" | "bad-request", error: unknown) =>
    send({ id: request.id, ok: false, kind, detail: String(error instanceof Error ? error.message : error).slice(0, 500) });
  if (!Array.isArray(request.parts) || !request.parts.every((part) => typeof part === "string")) return fail("bad-request", "parts");
  // 載入失敗就一直失敗（主頁收到後回收整支 Worker，下次重建）
  loaded ??= loadCard(request.parts);
  try {
    await loaded;
  } catch (error) {
    return fail("schema-load", error);
  }
  try {
    if (request.op === "parse") {
      const result = await engine.parse(request.message, parse(request.old) as Record<string, unknown>, parse(request.macros) as Macros);
      send({ id: request.id, ok: true, json: stringify(result) });
    } else if (request.op === "initialize") {
      send({ id: request.id, ok: true, json: stringify(engine.initialize(parse(request.stat))) });
    } else {
      fail("bad-request", "op");
    }
  } catch (error) {
    fail("script", error);
  }
}

// iframe 開機後轉交 MessagePort（只收第一次）；先告訴 iframe 已啟動，它就能收回引擎的 blob 網址
const post = scope.postMessage.bind(scope) as (message: unknown) => void;
const listen = scope.addEventListener.bind(scope) as (type: string, listener: (event: MessageEvent) => void) => void;
listen("message", (event) => {
  if (port !== null || !(event.ports?.[0] instanceof MessagePort)) return;
  port = event.ports[0];
  port.onmessage = (message: MessageEvent) => {
    void handle(message.data as ZodRequest);
  };
  const stuck = stripGlobals(scope);
  const leftover = leftoverCapabilities(scope);
  if (leftover.length > 0) {
    send({ type: "boot-failed", detail: `隔離不完整：${leftover.join(", ")}${stuck.length ? `（刪不掉：${stuck.join(", ")}）` : ""}` });
    return;
  }
  Object.assign(scope, cardGlobals());
  send({ type: "ready" });
});
post("up");
