// 網頁版的 QuickJS 宿主（mvu-zod-semantics 規格 4 節；桌面對應 src-tauri/src/mvu_engine/runtime.rs）：wasm 版 QuickJS
// 跑桌面同一支 bundle（quickjs-entry.ts），卡片 schema 腳本原文只當字串送進 QuickJS 執行，拿不到任何瀏覽器能力。
// 宿主只橋接模組 loader（白名單網址回墊片、其餘失敗，從不連網）與中斷處理器，不交任何宿主函式或物件給卡片。
// 防線：每支各自一份有 maximum 的 WebAssembly.Memory（真正的硬上限）；QuickJS 的 setMemoryLimit 只是內層，wasm 版
// 擋不住字串／ArrayBuffer 配置，不算防線。任何失敗後這支宿主就壞了、不再接工，也不 dispose（觸限後釋放可能中止），
// 由呼叫端回收整支 Worker。純邏輯不碰 DOM：隔離 Worker（worker-entry.ts）與 Node 測試（test-runner.ts）共用。
import { newQuickJSWASMModuleFromVariant, newVariant, type QuickJSContext, type QuickJSHandle } from "quickjs-emscripten-core";
import variant from "@jitl/quickjs-ng-wasmfile-release-sync";
import { REPLY_CHARS, type ZodWorkerFailure } from "./protocol";
import { isMvuZodUrl } from "./schema-script";

/** wasm 線性記憶體上限：整支引擎（QuickJS 堆積、配置器碎片、C 堆疊）的總佔用；正常卡約 19 MB */
export const MEMORY_MAX_BYTES = 128 * 1024 * 1024;
const MEMORY_INITIAL_BYTES = 16 * 1024 * 1024;
const WASM_PAGE = 64 * 1024;
/** 內層（同桌面 Limits）：不是防線，只讓多數配置以 QuickJS 的 out of memory 先失敗 */
const INNER_MEMORY_BYTES = 64 * 1024 * 1024;
const STACK_BYTES = 1024 * 1024;
/** 錯誤文字上限（字元），讀之前先量長度 */
const DETAIL_CHARS = 500;
const NAME_CHARS = 64;

const SHIM = "tt:mvu_zod";
const BLOCKED = "tt:blocked";
const SHIM_GLOBAL = "__ttRegisterMvuSchema";
/** 卡片 import 的 mvu_zod 換成這個墊片（同 runtime.rs SHIM_SOURCE） */
const SHIM_SOURCE = `const register = globalThis.${SHIM_GLOBAL};\ndelete globalThis.${SHIM_GLOBAL};\nexport function registerMvuSchema(input) { return register(input); }\n`;
const BOOT_SOURCE = `(() => { const api = TTMvu.boot(); globalThis.TTMvu = undefined; return api; })()`;

export class EngineError extends Error {
  constructor(
    readonly kind: ZodWorkerFailure,
    detail: string,
  ) {
    super(detail);
  }
}

export interface QuickJsHostOptions {
  /** wasm 位元組（release-sync 版） */
  wasm: ArrayBuffer;
  /** 桌面同一支 bundle 的原始碼（scripts/build-mvu-bundle.mjs） */
  bundle: string;
  now?: () => number;
  memoryMaxBytes?: number;
}

export interface QuickJsHost {
  /** 載入卡片 schema 腳本（只一次）；`deadline` 是 now() 的絕對時刻 */
  load: (schema: string, deadline: number) => void;
  /** 呼叫 bundle 介面（parse／initialize），回傳 JSON 文字 */
  call: (op: "parse" | "initialize", args: string[], deadline: number) => string;
  readonly loaded: boolean;
  /** 目前 wasm 線性記憶體大小（位元組） */
  readonly memoryBytes: number;
}

/** 建宿主：自己的有上限 Memory（建不出來就失敗，不退回預設 Memory）、QuickJS runtime、載入 bundle 並開機 */
export async function createQuickJsHost(options: QuickJsHostOptions, bootDeadline: number): Promise<QuickJsHost> {
  const now = options.now ?? (() => performance.now());
  const maximum = options.memoryMaxBytes ?? MEMORY_MAX_BYTES;
  const memory = new WebAssembly.Memory({ initial: Math.min(MEMORY_INITIAL_BYTES, maximum) / WASM_PAGE, maximum: maximum / WASM_PAGE });
  const module = await newQuickJSWASMModuleFromVariant(newVariant(variant, { wasmBinary: options.wasm, wasmMemory: memory }));
  const runtime = module.newRuntime();
  runtime.setMemoryLimit(INNER_MEMORY_BYTES);
  runtime.setMaxStackSize(STACK_BYTES);
  let deadline = bootDeadline;
  let fired = false;
  runtime.setInterruptHandler(() => {
    if (now() < deadline) return false;
    fired = true;
    return true;
  });
  runtime.setModuleLoader(
    (name) => (name === SHIM ? SHIM_SOURCE : { error: new Error("schema 腳本不准載入這個模組") }),
    (_base, name) => (isMvuZodUrl(name) ? SHIM : BLOCKED),
  );
  const ctx = runtime.newContext();
  const inspect = inspector(ctx);
  let broken = false;
  let loaded = false;

  /** 把失敗記下來：之後這支不再接工 */
  const fail = (error: EngineError): never => {
    broken = true;
    throw error;
  };
  const describe = (handle: QuickJSHandle) => {
    const failure = classify(inspect, handle, fired);
    handle.dispose();
    return fail(failure);
  };
  /** 跑完待辦工作（Promise 回呼、動態 import）；期限到了宿主這端也停 */
  const drain = () => {
    while (runtime.hasPendingJob()) {
      if (now() >= deadline) {
        fired = true;
        fail(new EngineError("deadline", "整次處理超過期限"));
      }
      const result = runtime.executePendingJobs(256);
      if (result.error) describe(result.error);
    }
  };
  /** 結算一個值：Promise 等到結算，其餘照原值 */
  const settle = (handle: QuickJSHandle): QuickJSHandle => {
    drain();
    const state = ctx.getPromiseState(handle);
    if (state.type === "fulfilled" && state.notAPromise) return handle;
    handle.dispose();
    if (state.type === "rejected") return describe(state.error);
    if (state.type === "pending") return fail(new EngineError("script", "Promise 永遠不會結算"));
    return state.value;
  };
  const guarded = <T>(run: () => T): T => {
    if (broken) throw new EngineError("crashed", "引擎已失效");
    try {
      const value = run();
      // 中斷只要觸發過就整次失敗，即使最後回了成功
      if (fired) fail(new EngineError("deadline", "整次處理超過期限"));
      return value;
    } catch (error) {
      broken = true;
      // 中斷觸發過（例如讀錯誤屬性時卡片的 getter 跑到期限）一律歸期限
      if (fired) throw error instanceof EngineError && error.kind === "deadline" ? error : new EngineError("deadline", "整次處理超過期限");
      if (error instanceof EngineError) throw error;
      // wasm trap、emscripten 中止、宿主膠水的例外；遞迴太深時可能是瀏覽器自己的 wasm 呼叫堆疊先爆（Worker 堆疊
      // 比 QuickJS 的上限小），一樣歸堆疊
      const detail = clip(String(error instanceof Error ? error.message : error));
      throw new EngineError(error instanceof RangeError && detail.includes("call stack") ? "stack" : "crashed", detail);
    }
  };
  const evaluate = (code: string, filename: string, type: "global" | "module") => {
    const result = ctx.evalCode(code, filename, { type });
    return result.error ? describe(result.error) : result.value;
  };

  const api = guarded(() => {
    const failed = (error: unknown) => (error instanceof EngineError ? error : new EngineError("crashed", String(error)));
    try {
      evaluate(options.bundle, "mvu-engine.js", "global").dispose();
      return settle(evaluate(BOOT_SOURCE, "boot.js", "global"));
    } catch (error) {
      const failure = failed(error);
      throw new EngineError(failure.kind, `引擎開機失敗：${failure.message}`);
    }
  });

  const host: QuickJsHost = {
    get loaded() {
      return loaded;
    },
    get memoryBytes() {
      return memory.buffer.byteLength;
    },
    load(schema, at) {
      if (loaded) return;
      deadline = at;
      guarded(() => {
        try {
          const register = ctx.getProp(api, "register");
          ctx.setProp(ctx.global, SHIM_GLOBAL, register);
          register.dispose();
          settle(evaluate(schema, "card-schema", "module")).dispose();
          // 卡片沒 import 墊片時全域還留著：一律清掉
          ctx.evalCode(`delete globalThis.${SHIM_GLOBAL}`).dispose();
          const status = JSON.parse(callString("status", [])) as { registered?: unknown };
          if (status.registered !== true) fail(new EngineError("schema-load", "卡片 schema 腳本沒有呼叫 registerMvuSchema"));
        } catch (error) {
          // 同 runtime.rs build：載入途中的失敗都歸載入失敗，只有期限歸期限
          if (!(error instanceof EngineError) || error.kind === "deadline" || error.kind === "schema-load") throw error;
          const reason = error.kind === "memory" ? `記憶體超過上限（${error.message}）` : error.kind === "stack" ? `堆疊超過上限（${error.message}）` : error.message;
          throw new EngineError("schema-load", `卡片 schema 腳本執行失敗：${reason}`);
        }
      });
      loaded = true;
    },
    call(op, args, at) {
      if (!loaded) throw new EngineError("bad-request", "卡片 schema 還沒載入");
      deadline = at;
      return guarded(() => callString(op, args));
    },
  };
  return host;

  /** 呼叫介面函式並取回字串：先量長度，超過上限就失敗，不整份搬出 */
  function callString(name: string, args: string[]): string {
    const fn = ctx.getProp(api, name);
    const handles = args.map((arg) => ctx.newString(arg));
    const result = ctx.callFunction(fn, api, ...handles);
    fn.dispose();
    for (const handle of handles) handle.dispose();
    const value = settle(result.error ? describe(result.error) : result.value);
    try {
      if (ctx.typeof(value) !== "string") return fail(new EngineError("script", "引擎回傳不是字串"));
      const length = inspect.length(value);
      if (length > REPLY_CHARS) return fail(new EngineError("too-large", `引擎回傳超過上限（${length} 字）`));
      return ctx.getString(value);
    } finally {
      value.dispose();
    }
  }
}

/** 建置時內嵌的 wasm（base64）→ 位元組 */
export function decodeWasm(base64: string): ArrayBuffer {
  const text = atob(base64);
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i);
  return bytes.buffer;
}

function clip(text: string): string {
  return text.length > DETAIL_CHARS ? text.slice(0, DETAIL_CHARS) : text;
}

interface Inspector {
  ctx: QuickJSContext;
  /** 字串長度（字元）；量不到回 Infinity */
  length: (handle: QuickJSHandle) => number;
  isNull: (handle: QuickJSHandle) => boolean;
}

/** 開機時（卡片還沒執行）在 QuickJS 裡建的兩支小函式，只有宿主拿得到；字串原始值的 length 卡片改不掉 */
function inspector(ctx: QuickJSContext): Inspector {
  const created = ctx.evalCode("({ length: (s) => s.length, isNull: (v) => v === null })", "inspect.js");
  if (created.error) {
    created.error.dispose();
    throw new EngineError("crashed", "宿主小工具建立失敗");
  }
  const helpers = created.value;
  const lengthFn = ctx.getProp(helpers, "length");
  const isNullFn = ctx.getProp(helpers, "isNull");
  helpers.dispose();
  const apply = <T>(fn: QuickJSHandle, handle: QuickJSHandle, read: (value: QuickJSHandle) => T, fallback: T): T => {
    const result = ctx.callFunction(fn, ctx.undefined, handle);
    if (result.error) {
      result.error.dispose();
      return fallback;
    }
    try {
      return read(result.value);
    } finally {
      result.value.dispose();
    }
  };
  return {
    ctx,
    length: (handle) => apply(lengthFn, handle, (value) => (ctx.typeof(value) === "number" ? ctx.getNumber(value) : Infinity), Infinity),
    isNull: (handle) => apply(isNullFn, handle, (value) => ctx.typeof(value) === "boolean" && ctx.getNumber(value) === 1, false),
  };
}

/** 讀 QuickJS 裡的字串：先量長度，超過就不讀 */
function readString(inspect: Inspector, handle: QuickJSHandle, max: number): string | null {
  if (inspect.ctx.typeof(handle) !== "string") return null;
  const length = inspect.length(handle);
  return length <= max ? inspect.ctx.getString(handle) : `（${length} 字，過長略去）`;
}

function readProp(inspect: Inspector, handle: QuickJSHandle, key: string, max: number): string | null {
  try {
    const value = inspect.ctx.getProp(handle, key);
    try {
      return readString(inspect, value, max);
    } finally {
      value.dispose();
    }
  } catch {
    return null;
  }
}

/** 錯誤分類（同 runtime.rs classify）：中斷觸發過一律算期限；null／undefined（記憶體耗盡到建不出錯誤物件，或卡片自己
 *  丟的）算記憶體；記憶體、堆疊照訊息分；核心的 parseFailure 帶 code。錯誤文字只讀有上限的長度 */
function classify(inspect: Inspector, handle: QuickJSHandle, fired: boolean): EngineError {
  if (fired) return new EngineError("deadline", "整次處理超過期限");
  const type = inspect.ctx.typeof(handle);
  if (type === "undefined" || (type === "object" && inspect.isNull(handle))) return new EngineError("memory", "丟出 null（記憶體耗盡）");
  if (type !== "object") return new EngineError("script", `丟出非錯誤值：${readString(inspect, handle, DETAIL_CHARS) ?? type}`);
  const message = readProp(inspect, handle, "message", DETAIL_CHARS) ?? "";
  const name = readProp(inspect, handle, "name", NAME_CHARS);
  const code = readProp(inspect, handle, "code", NAME_CHARS);
  const detail = clip(name ? `${name}: ${message}` : message);
  if (message.includes("out of memory") || message.includes("string too long")) return new EngineError("memory", detail);
  if (message.includes("call stack")) return new EngineError("stack", detail);
  if (code === "deadline") return new EngineError("deadline", detail);
  return new EngineError("script", code ? `${detail}（${code}）` : detail);
}
