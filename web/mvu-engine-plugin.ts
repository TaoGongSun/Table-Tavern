// `virtual:mvu-engine`：把桌面版沙盒裡的 MVU parseMessage（card-mvu-parse-source.ts，照 MagVarUpdate
// updateVariables 重寫、與後端有對照測試的那一份）在建置時包成一般模組給宿主用。宿主頁的 CSP 不准 eval，
// 這樣不必在執行時求值字串，也不另寫一份 MVU。片段用到的閉包變數（emit、事件名、lodash、送值解析的對象、
// token、巨集代換值）由 createMvuEngine 的參數給。
import type { Plugin } from "vite";
import { buildMvuParseSource } from "../src/features/card-interface/mvu/card-mvu-parse-source";

const ID = "virtual:mvu-engine";
const RESOLVED = `\0${ID}`;

function engineModule(): string {
  return [
    "export function createMvuEngine(env) {",
    "  var emit = env.emit;",
    "  var EVENTS = env.events;",
    "  var lodash = env.lodash;",
    "  var parentRef = env.parentRef;",
    "  var TOKEN = env.token;",
    "  var store = env.store;",
    "  var hasOwn = Object.prototype.hasOwnProperty;",
    "  function isPlainObject(value) {",
    '    return value !== null && typeof value === "object" && !Array.isArray(value);',
    "  }",
    buildMvuParseSource(),
    "  return {",
    "    parseMessage: parseMessage,",
    "    resolveEval: resolveEval,",
    "    generateSchema: generateSchema,",
    "    cleanUpMetadata: cleanUpMetadata,",
    "  };",
    "}",
  ].join("\n");
}

export function mvuEnginePlugin(): Plugin {
  return {
    name: "tt-mvu-engine",
    resolveId: (source) => (source === ID ? RESOLVED : null),
    load: (id) => (id === RESOLVED ? engineModule() : null),
  };
}
