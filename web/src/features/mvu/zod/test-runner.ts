// 測試專用：不經 iframe／Worker，在同一支程式跑同一份 zod 引擎（engine-core.ts）的 ZodRunner。schema 腳本照
// splitSchemaImports 切開後，以參數代替墊片 import 執行（隔離與期限只在真瀏覽器測：web/e2e/zod-isolation.mjs）。
import { createZodEngine, cardGlobals } from "./engine-core";
import { ZodRunError, type ZodRunner } from "./frame-host";
import { splitSchemaImports } from "./schema-script";
import { checkDetailed, checkInitialized } from "./validate";

const SHIM = "tt:shim";

/** 已切開的 schema 腳本 → 註冊（測試改寫：拿掉墊片 import 與 export，其餘照原文執行） */
export function loadSchemaForTest(register: (input: unknown) => void, schema: string): void {
  const split = splitSchemaImports(schema);
  if (!split.ok) throw new ZodRunError("schema-load", split.error);
  const source = split.parts.join(JSON.stringify(SHIM));
  const pattern = /import\s*\{\s*registerMvuSchema(?:\s+as\s+(\w+))?\s*\}\s*from\s*"tt:shim";?/;
  const alias = pattern.exec(source)?.[1] ?? "registerMvuSchema";
  const body = source.replace(pattern, "").replace(/export const/g, "const");
  const globals = cardGlobals();
  new Function("z", "_", "$", "console", alias, body)(globals.z, globals._, globals.$, globals.console, register);
}

export function createTestZodRunner(schema: string): ZodRunner & { calls: { parse: number; initialize: number } } {
  let engine: ReturnType<typeof createZodEngine> | null = null;
  const calls = { parse: 0, initialize: 0 };
  const ensure = () => {
    if (engine) return engine;
    const fresh = createZodEngine();
    try {
      loadSchemaForTest(fresh.register, schema);
    } catch (error) {
      throw error instanceof ZodRunError ? error : new ZodRunError("schema-load", String(error));
    }
    engine = fresh;
    return fresh;
  };
  return {
    calls,
    async parse(message, old, macros) {
      calls.parse += 1;
      const result = await ensure().parse(message, JSON.parse(JSON.stringify(old)), macros);
      return checkDetailed(JSON.stringify(result));
    },
    async initialize(stat) {
      calls.initialize += 1;
      return checkInitialized(JSON.stringify(ensure().initialize(JSON.parse(JSON.stringify(stat)))));
    },
    dispose() {},
  };
}
