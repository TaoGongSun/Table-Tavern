// 卡片變數表的上限（與後端 src-tauri/src/data/message_vars/json.rs 同一組數字與先後）：卡片寫入與網頁存檔
// 共用。整張大小兩種量法：卡片寫入（有原文）原文位元組或 JSON.stringify 緊湊寫法任一不超過就收（舊版只量
// 原文，這樣只放寬不收窄）；網頁存檔的表只量緊湊寫法（後端照 JS 的數字寫法算，原文的空白不算）。

export const VARS_TABLE_LIMITS = {
  /** 整張表（緊湊寫法）上限 */
  tableBytes: 2 * 1024 * 1024,
  /** 原文先擋的上限（解析前，防超大字串）：整張上限的四倍 */
  rawBytes: 8 * 1024 * 1024,
  depth: 32,
  stringBytes: 64 * 1024,
  children: 10_000,
  nodes: 200_000,
  keyChars: 256,
} as const;

const encoder = new TextEncoder();
const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

/** 字串裡有孤立的代理字元（後端的 JSON 解析不收，`\ud800` 這種跳脫一律拒絕）。 */
export const hasLoneSurrogate = (text: string) => LONE_SURROGATE.test(text);

export type TableProblem =
  | "invalid-json"
  | "not-object"
  | "too-large"
  | "too-deep"
  | "string-too-long"
  | "non-finite"
  | "too-many-children"
  | "too-many-nodes"
  | "empty-key"
  | "key-too-long";

/**
 * 驗一張已解析的表；不符回原因代碼，符合回 null。先後：頂層物件 → 整張大小 → 逐節點。`rawBytes`＝原文
 * 位元組（有原文時原文不超過上限就不再量緊湊寫法）。
 */
export function tableProblem(value: unknown, rawBytes?: number): TableProblem | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return "not-object";
  const rawFits = rawBytes !== undefined && rawBytes <= VARS_TABLE_LIMITS.tableBytes;
  if (!rawFits && encoder.encode(JSON.stringify(value)).length > VARS_TABLE_LIMITS.tableBytes) return "too-large";
  let nodes = 0;
  const walk = (node: unknown, depth: number): TableProblem | null => {
    nodes += 1;
    if (nodes > VARS_TABLE_LIMITS.nodes) return "too-many-nodes";
    if (depth > VARS_TABLE_LIMITS.depth) return "too-deep";
    if (typeof node === "string") {
      if (hasLoneSurrogate(node)) return "invalid-json";
      return encoder.encode(node).length > VARS_TABLE_LIMITS.stringBytes ? "string-too-long" : null;
    }
    if (typeof node === "number") return Number.isFinite(node) ? null : "non-finite";
    if (Array.isArray(node)) {
      if (node.length > VARS_TABLE_LIMITS.children) return "too-many-children";
      for (const item of node) {
        const problem = walk(item, depth + 1);
        if (problem) return problem;
      }
      return null;
    }
    if (node !== null && typeof node === "object") {
      const entries = Object.entries(node);
      if (entries.length > VARS_TABLE_LIMITS.children) return "too-many-children";
      for (const [key, child] of entries) {
        if (key === "") return "empty-key";
        if (hasLoneSurrogate(key)) return "invalid-json";
        if ([...key].length > VARS_TABLE_LIMITS.keyChars) return "key-too-long";
        const problem = walk(child, depth + 1);
        if (problem) return problem;
      }
    }
    return null;
  };
  return walk(value, 1);
}

/** 驗一段要寫入的表原文：原文先擋、解析、再照 `tableProblem`。 */
export function tableTextProblem(text: string): TableProblem | null {
  const rawBytes = encoder.encode(text).length;
  if (rawBytes > VARS_TABLE_LIMITS.rawBytes) return "too-large";
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return "invalid-json";
  }
  return tableProblem(value, rawBytes);
}
