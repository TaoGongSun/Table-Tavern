// MVU `parseMessage` 的值解析引擎（計畫 8.9，D1）：跑在宿主專用 Web Worker 裡（card-mvu-eval.worker.ts），
// 純函式、不碰 DOM 與 app 狀態。行為對照 MagVarUpdate `parseCommandValue`（值解析六段）與 `parseString`
// （JSONPatch 區塊），只當規格書讀、不抄碼。
import {
  all,
  create,
  isAccessorNode,
  isAssignmentNode,
  isComplex,
  isConstantNode,
  isFunctionAssignmentNode,
  isFunctionNode,
  isMatrix,
  isSymbolNode,
  type MathJsInstance,
  type MathNode,
} from "mathjs";
import JSON5 from "json5";
import YAML from "yaml";
import { jsonrepair } from "jsonrepair";

/** 單一數學式字數上限（8.9）；超過就不求值，往下走 YAML／字串 */
export const MAX_EXPRESSION_CHARS = 1000;

// 值上限（與 8.8、card-mvu-write.ts 同一組數字）
const MAX_DEPTH = 32;
const MAX_STRING_BYTES = 64 * 1024;
const MAX_CHILDREN = 10_000;
const MAX_NODES = 200_000;

/** 命令值用 `yaml` 套件預設解析（同上游 `YAML.parse`）；JSONPatch 區塊另開合併鍵（同上游 `parseDocument(…, { merge: true }).toJS()`，
 *  文件有錯誤時不丟錯，照上游往下用解出的內容） */
function yamlLoad(text: string): unknown {
  return YAML.parse(text);
}
function yamlPatchLoad(text: string): unknown {
  return YAML.parseDocument(text, { merge: true }).toJS();
}

/** 去掉頭尾的反斜線、引號、反引號與空白；字串含換行時（`.` 不吃換行）照原樣 */
export function trimQuotesAndBackslashes(text: string): string {
  return text.replace(/^[\\"'` ]*(.*?)[\\"'` ]*$/, "$1");
}

// ---- 受限 mathjs ----

/** 只複製允許的成員、凍結成獨立命名空間：算式裡 `Math.floor`、`math.pow` 照常能寫，卻改不到宿主物件 */
function readOnlyNamespace(source: Record<string, unknown>, names: readonly string[]): Readonly<Record<string, unknown>> {
  const namespace: Record<string, unknown> = {};
  for (const name of names) {
    const value = source[name];
    namespace[name] = typeof value === "function" ? (value as (...args: unknown[]) => unknown).bind(source) : value;
  }
  return Object.freeze(namespace);
}

const MATH_NAMES = [
  "abs", "acos", "acosh", "asin", "asinh", "atan", "atan2", "atanh", "ceil", "cos", "cosh", "cube", "e", "exp",
  "expm1", "floor", "gcd", "hypot", "lcm", "log", "log10", "log1p", "log2", "max", "mean", "median", "min", "mod",
  "nthRoot", "pi", "pow", "prod", "round", "sign", "sin", "sinh", "sqrt", "square", "std", "sum", "tan", "tanh",
  "tau", "variance",
] as const;

let shared: MathJsInstance | undefined;
let jsMath: Readonly<Record<string, unknown>> | undefined;
let mathNamespace: Readonly<Record<string, unknown>> | undefined;
let reusableFunctions: Set<string> | undefined;

/** 共用實例與命名空間（Worker 開機時先建好，第一筆求值才不吃建置時間） */
export function prepareMath(): void {
  if (shared !== undefined) return;
  shared = create(all);
  jsMath = readOnlyNamespace(Math as unknown as Record<string, unknown>, Object.getOwnPropertyNames(Math));
  mathNamespace = readOnlyNamespace(shared as unknown as Record<string, unknown>, MATH_NAMES);
  reusableFunctions = new Set([
    ...Object.keys(mathNamespace).filter((name) => typeof mathNamespace![name] === "function"),
    "complex", "det", "matrix", "number", "unit",
  ]);
}

function isFacadeAccessor(node: MathNode): boolean {
  if (!isAccessorNode(node)) return false;
  const property = node.index.dimensions[0];
  if (!isSymbolNode(node.object) || node.index.dimensions.length !== 1 || !isConstantNode(property) || typeof property.value !== "string") {
    return false;
  }
  const namespace = node.object.name === "Math" ? jsMath : node.object.name === "math" ? mathNamespace : undefined;
  return namespace !== undefined && Object.prototype.hasOwnProperty.call(namespace, property.value);
}

/** 只有「無賦值、無間接呼叫」的純數學式才共用實例；整棵語法樹檢查，求導後 evaluate、`import`、單位修改等一律走一次性新實例 */
function canShareInstance(expression: MathNode): boolean {
  return (
    expression.filter((node) => {
      if (isAssignmentNode(node) || isFunctionAssignmentNode(node)) return true;
      if (isAccessorNode(node)) return !isFacadeAccessor(node);
      if (isFunctionNode(node)) {
        return isSymbolNode(node.fn) ? !reusableFunctions!.has(node.fn.name) : !isFacadeAccessor(node.fn);
      }
      return false;
    }).length === 0
  );
}

/** 數學式求值；回 `{ value }` 或 null（不是可用的算式，往下走 YAML／字串） */
function evaluateMath(trimmed: string): { value: unknown } | null {
  if (trimmed.length > MAX_EXPRESSION_CHARS) return null;
  prepareMath();
  try {
    const scope = { Math: jsMath, math: mathNamespace };
    const expression = shared!.parse(trimmed);
    const result: unknown = canShareInstance(expression)
      ? expression.compile().evaluate(scope)
      : create(all).evaluate(trimmed, scope);
    // 複數與矩陣轉成字串表示
    if (isComplex(result) || isMatrix(result)) return { value: String(result) };
    // 單一單字（符號名）求值是 undefined：不是算式
    if (result === undefined && !/^[a-zA-Z_]+$/.test(trimmed)) return { value: trimmed };
    // 浮點誤差用 toPrecision 修掉；布林、單位等沒有 toPrecision 的結果會丟錯，走 catch 往下
    if (result !== undefined) return { value: parseFloat((result as number).toPrecision(12)) };
  } catch {
    /* 不是有效算式 */
  }
  return null;
}

/**
 * 命令值解析六段：字面量（true／false／null／undefined）→ JSON → JSON5（只收物件與陣列）→
 * 單引號 YAML → 受限數學式 → YAML → 去頭尾引號的字串。
 */
export function parseCommandValue(text: string): unknown {
  const trimmed = text.trim();
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (trimmed === "null") return null;
  if (trimmed === "undefined") return undefined;
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
      try {
        const relaxed: unknown = JSON5.parse(trimmed);
        if (relaxed !== null && typeof relaxed === "object") return relaxed;
      } catch {
        /* 可能是未加引號的字串或算式 */
      }
    }
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
    try {
      return yamlLoad(trimmed);
    } catch {
      /* 仍可能是含字串運算元的算式 */
    }
  }
  const math = evaluateMath(trimmed);
  if (math !== null) return math.value;
  try {
    return yamlLoad(trimmed);
  } catch {
    /* 普通字串 */
  }
  return trimQuotesAndBackslashes(text);
}

/** JSONPatch 區塊的文字解析：YAML → JSON5 → jsonrepair 修殘缺 JSON；以 `[`／`{` 開頭的先走 JSON5，最後才試 YAML。全失敗丟錯 */
export function parsePatchBlock(content: string): unknown {
  const jsonFirst = /^[[{]/s.test(content.trimStart());
  let yamlError: unknown;
  if (!jsonFirst) {
    try {
      return yamlPatchLoad(content);
    } catch (error) {
      yamlError = error;
    }
  }
  try {
    return JSON5.parse(content) as unknown;
  } catch {
    /* 往下 */
  }
  try {
    return JSON.parse(jsonrepair(content)) as unknown;
  } catch {
    /* 往下 */
  }
  if (jsonFirst) {
    try {
      return yamlPatchLoad(content);
    } catch (error) {
      yamlError = error;
    }
  }
  throw new Error(`格式無法解析：${yamlError instanceof Error ? yamlError.message : "未知錯誤"}`);
}

/** 結果值的上限（8.8 的深度、字串、元素數、節點數；循環參照也算超限）；符合回 null，否則回原因代碼 */
export function valueLimitProblem(value: unknown): string | null {
  let nodes = 0;
  const path = new Set<object>();
  const walk = (node: unknown, depth: number): string | null => {
    nodes += 1;
    if (nodes > MAX_NODES) return "too-many-nodes";
    if (depth > MAX_DEPTH) return "too-deep";
    if (typeof node === "string") {
      return node.length * 3 > MAX_STRING_BYTES && new TextEncoder().encode(node).length > MAX_STRING_BYTES ? "string-too-long" : null;
    }
    if (node === null || typeof node !== "object") return null;
    if (path.has(node)) return "circular";
    path.add(node);
    try {
      const children = Array.isArray(node) ? node : Object.values(node);
      if (children.length > MAX_CHILDREN) return "too-many-children";
      for (const child of children) {
        const problem = walk(child, depth + 1);
        if (problem !== null) return problem;
      }
      return null;
    } finally {
      path.delete(node);
    }
  };
  return walk(value, 1);
}

export type EvalOp = "value" | "patch";

/** Worker 與測試共用的入口：回 `{ ok, value }` 或 `{ ok: false, error }`；結果超過值上限算錯 */
export function runEval(op: EvalOp, text: string): { ok: true; value: unknown } | { ok: false; error: string } {
  try {
    const value = op === "patch" ? parsePatchBlock(text) : parseCommandValue(text);
    const problem = valueLimitProblem(value);
    if (problem !== null) return { ok: false, error: `value-limit:${problem}` };
    return { ok: true, value };
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
}
