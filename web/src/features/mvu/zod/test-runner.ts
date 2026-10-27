// 測試專用：不經 iframe／Worker，在同一支程式跑正式版同一個 QuickJS 宿主（quickjs-host.ts：同一份 wasm、同一支
// bundle、同樣的 Memory 上限與 loader）的 ZodRunner。iframe、CSP 與主頁期限只在真瀏覽器測：web/e2e/zod-isolation.mjs。
import { bundle, wasm } from "virtual:mvu-quickjs-assets";
import { ZodRunError, type ZodFailureKind, type ZodRunner } from "./frame-host";
import { createQuickJsHost, decodeWasm, EngineError, type QuickJsHost, type QuickJsHostOptions } from "./quickjs-host";
import { checkDetailed, checkInitialized } from "./validate";

const DEADLINE_MS = 5_000;

/** 正式版同一份資產建宿主（測試可改上限） */
export function createTestHost(overrides: Partial<QuickJsHostOptions> = {}): Promise<QuickJsHost> {
  return createQuickJsHost({ wasm: decodeWasm(wasm), bundle, ...overrides }, performance.now() + 10 * DEADLINE_MS);
}

const toRunError = (error: unknown) => {
  if (!(error instanceof EngineError)) return error;
  const kind: ZodFailureKind = ["schema-load", "deadline", "memory", "stack"].includes(error.kind) ? (error.kind as ZodFailureKind) : "script";
  return new ZodRunError(kind, error.message);
};

/** 出錯就丟掉這支宿主、下一次重建（同主頁回收整支 Worker） */
export function createTestZodRunner(schema: string): ZodRunner & { calls: { parse: number; initialize: number } } {
  let host: Promise<QuickJsHost> | null = null;
  const calls = { parse: 0, initialize: 0 };
  const run = async (op: "parse" | "initialize", args: string[]) => {
    host ??= createTestHost();
    try {
      const ready = await host;
      const deadline = performance.now() + DEADLINE_MS;
      ready.load(schema, deadline);
      return ready.call(op, args, deadline);
    } catch (error) {
      host = null;
      throw toRunError(error);
    }
  };
  return {
    calls,
    async parse(message, old, macros) {
      calls.parse += 1;
      return checkDetailed(await run("parse", [message, JSON.stringify(old), JSON.stringify(macros)]));
    },
    async initialize(stat) {
      calls.initialize += 1;
      return checkInitialized(await run("initialize", [JSON.stringify(stat)]));
    },
    dispose() {},
  };
}
