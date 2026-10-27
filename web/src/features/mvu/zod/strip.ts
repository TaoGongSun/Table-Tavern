// 隔離 Worker 的第二道防線（規格 4 節）：卡片腳本執行前把全域剝到只剩 ECMAScript 內建（白名單），再確認危險能力
// 一個都看不到——還看得到就不載入卡片（失敗即停）。第一道防線是不透明來源（拿不到網站儲存）加 CSP。

/** 留下的全域：ECMAScript 內建，加 TextEncoder／TextDecoder（值解析要用）、計時器、structuredClone */
export const KEEP: ReadonlySet<string> = new Set([
  "Object", "Function", "Array", "Number", "parseFloat", "parseInt", "Infinity", "NaN", "undefined", "Boolean", "String",
  "Symbol", "Date", "Promise", "RegExp", "Error", "AggregateError", "EvalError", "RangeError", "ReferenceError",
  "SyntaxError", "TypeError", "URIError", "globalThis", "JSON", "Math", "Intl", "ArrayBuffer", "Atomics", "Uint8Array",
  "Int8Array", "Uint16Array", "Int16Array", "Uint32Array", "Int32Array", "Float32Array", "Float64Array",
  "Uint8ClampedArray", "BigUint64Array", "BigInt64Array", "DataView", "Map", "BigInt", "Set", "WeakMap", "WeakSet",
  "Proxy", "Reflect", "FinalizationRegistry", "WeakRef", "decodeURI", "decodeURIComponent", "encodeURI",
  "encodeURIComponent", "escape", "unescape", "isFinite", "isNaN", "Iterator", "SharedArrayBuffer", "structuredClone",
  "queueMicrotask", "setTimeout", "clearTimeout", "TextEncoder", "TextDecoder", "constructor",
]);

/** 失敗即停：卡片看得到其中任何一個就不載入 */
export const MUST_BE_GONE = [
  "indexedDB", "caches", "fetch", "XMLHttpRequest", "WebSocket", "WebTransport", "EventSource", "Worker", "SharedWorker",
  "RTCPeerConnection", "RTCDataChannel", "BroadcastChannel", "URL", "Blob", "postMessage", "importScripts", "navigator",
  "addEventListener", "onmessage", "close",
] as const;

/** 沿原型鏈（到 Object.prototype 為止）刪掉白名單外的屬性；回傳刪不掉的名字 */
export function stripGlobals(scope: object): string[] {
  const stuck: string[] = [];
  for (let proto: object | null = scope; proto !== null && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
    for (const name of Object.getOwnPropertyNames(proto)) {
      if (KEEP.has(name)) continue;
      try {
        if (!Reflect.deleteProperty(proto, name)) stuck.push(name);
      } catch {
        stuck.push(name);
      }
    }
    for (const symbol of Object.getOwnPropertySymbols(proto)) {
      if (symbol !== Symbol.toStringTag) Reflect.deleteProperty(proto, symbol);
    }
  }
  return stuck;
}

/** 還看得到的危險能力（應為空） */
export function leftoverCapabilities(scope: object): string[] {
  const view = scope as Record<string, unknown>;
  return MUST_BE_GONE.filter((name) => {
    try {
      return view[name] !== undefined;
    } catch {
      return true;
    }
  });
}
