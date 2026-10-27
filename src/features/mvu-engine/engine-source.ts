// MVU 共用核心（計畫 8.9；mvu-zod-semantics 規格 1 節）：`updateVariables`／`parseMessage` 照 MVU（MagVarUpdate
// `src/function/update_variables.ts`、`schema.ts`，釘版本見計畫 8 節）的行為重寫，只當規格書讀、不抄碼。
// 一份原始碼給三個宿主：卡片 iframe（card-mvu-shim-source.ts 嵌入）、網頁宿主（web/mvu-engine-plugin.ts 包成模組）、
// 桌面 QuickJS。宿主差異全走 `createMvuCore(env)` 的參數（型別見 host-contract.ts），片段本身不碰 DOM、postMessage、計時器。
//
// 片段是純 ES5＋async 的原始碼字串，用 String.raw 寫：反斜線照原樣進宿主，所以不能出現反引號與
// 「錢字號加左大括號」；要比對反引號寫 \x60。片段之間靠函式宣告互相呼叫，依下列順序串接。
import { SOURCE_APPLY } from "./source-apply";
import { SOURCE_COMMANDS } from "./source-commands";
import { SOURCE_LIMITS } from "./source-limits";
import { SOURCE_PATH } from "./source-path";
import { SOURCE_SCHEMA } from "./source-schema";
import { SOURCE_UPDATE } from "./source-update";
import { SOURCE_ZOD } from "./source-zod";

/** `function createMvuCore(env) { … }` 的原始碼：回值見 host-contract.ts 的 MvuCore */
export function buildMvuEngineSource(): string {
  return [
    "function createMvuCore(env) {",
    "  var host = env.host;",
    "  var emit = env.emit;",
    "  var EVENTS = env.events;",
    "  var lodash = env.lodash;",
    "  var hasOwn = Object.prototype.hasOwnProperty;",
    "  function isPlainObject(value) {",
    '    return value !== null && typeof value === "object" && !Array.isArray(value);',
    "  }",
    SOURCE_LIMITS,
    SOURCE_SCHEMA,
    SOURCE_PATH,
    SOURCE_COMMANDS,
    SOURCE_APPLY,
    SOURCE_ZOD,
    SOURCE_UPDATE,
    "  return {",
    "    parseMessage: parseMessage,",
    "    parseMessageDetailed: parseMessageDetailed,",
    "    generateSchema: generateSchema,",
    "    cleanUpMetadata: cleanUpMetadata,",
    "    registerMvuSchema: registerMvuSchema,",
    "    zodStatus: zodStatus,",
    "    zodInitialize: zodInitialize,",
    "    limits: PARSE_LIMITS",
    "  };",
    "}",
  ].join("\n");
}
