// `virtual:mvu-engine`：把 MVU 共用核心（src/features/mvu-engine/engine-source.ts，照 MagVarUpdate updateVariables
// 重寫、與後端有對照測試的那一份）在建置時包成一般模組給宿主用。宿主頁的 CSP 不准 eval，這樣不必在執行時
// 求值字串，也不另寫一份 MVU。宿主差異（值解析、計時器、巨集、事件）由 createMvuCore 的參數給。
import type { Plugin } from "vite";
import { buildMvuEngineSource } from "../src/features/mvu-engine/engine-source";

const ID = "virtual:mvu-engine";
const RESOLVED = `\0${ID}`;

export function mvuEnginePlugin(): Plugin {
  return {
    name: "tt-mvu-engine",
    resolveId: (source) => (source === ID ? RESOLVED : null),
    load: (id) => (id === RESOLVED ? `export ${buildMvuEngineSource()}` : null),
  };
}
