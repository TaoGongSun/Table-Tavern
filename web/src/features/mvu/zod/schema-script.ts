// zod 卡的 schema 腳本（mvu-zod-semantics 規格 1.5、4 節）：偵測卡上註冊 schema 的那一支，並把它的 import 切開——
// 只准 StageDog/tavern_resource 的 mvu_zod.js（對到墊片；動態 import 限字串字面量），其餘都算載入失敗。
// 切開的片段在隔離 Worker 裡以墊片的 blob 網址接回（blob 網址只能在那個不透明來源裡建）。
import { parse } from "es-module-lexer/js";

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

/** 卡片資料（`data` 那層）裡的 schema 腳本原文：酒館助手腳本（新版 `tavern_helper`、舊版 `TavernHelper_scripts`，
 *  形狀各版不同，一律依序遞迴找）裡 `enabled` 不為 false、內容含 `registerMvuSchema` 的第一支；停用的腳本（含停用的
 *  上層）不算。沒有回 null（非 zod 桌） */
export function findSchemaScript(cardData: unknown): string | null {
  const extensions = isObject(cardData) ? cardData.extensions : undefined;
  if (!isObject(extensions)) return null;
  const walk = (value: unknown): string | null => {
    if (Array.isArray(value)) {
      for (const item of value) {
        const found = walk(item);
        if (found !== null) return found;
      }
      return null;
    }
    if (!isObject(value) || value.enabled === false) return null;
    if (typeof value.content === "string" && value.content.includes("registerMvuSchema")) return value.content;
    for (const child of Object.values(value)) {
      const found = walk(child);
      if (found !== null) return found;
    }
    return null;
  };
  for (const key of ["tavern_helper", "TavernHelper_scripts"]) {
    const found = walk(extensions[key]);
    if (found !== null) return found;
  }
  return null;
}

/** 卡片能 import 的唯一模組（同 src-tauri/src/mvu_engine/runtime.rs is_mvu_zod_url）：jsdelivr 三個鏡像的
 *  `gh/StageDog/tavern_resource[@版本]/dist/util/mvu_zod.js`。只對應到墊片，從不連網 */
export function isMvuZodUrl(name: string): boolean {
  const match = /^https:\/\/(testingcf|cdn|fastly)\.jsdelivr\.net\/gh\/StageDog\/tavern_resource(@[A-Za-z0-9._-]+)?\/dist\/util\/mvu_zod\.js$/.exec(name);
  return match !== null;
}

export type SplitResult = { ok: true; parts: string[] } | { ok: false; error: string };

/** 切開 schema 腳本：回傳的片段之間各接一個墊片網址（含引號）就是可執行的模組。白名單網址的靜態 import（含
 *  `export … from`）與字串字面量的動態 import 都換成墊片（同桌面 QuickJS loader 放行白名單）。拒收：語法錯、白名單外的
 *  import、非字面量的動態 import（含模板字串；桌面在執行時才比對網址，網頁只能靜態判定）、source／defer phase import */
export function splitSchemaImports(source: string): SplitResult {
  let imports: ReturnType<typeof parse>[0];
  try {
    [imports] = parse(source);
  } catch (error) {
    return { ok: false, error: `schema 腳本無法解析：${String(error)}` };
  }
  const parts: string[] = [];
  let at = 0;
  for (const entry of imports) {
    if (entry.t === 3) continue; // import.meta：只有 url、resolve
    if (entry.t !== 1 && entry.t !== 2) return { ok: false, error: "schema 腳本不准用 source／defer 載入模組" };
    const name = entry.n ?? "";
    // 靜態 import 的範圍不含引號；動態 import 的範圍是整個參數運算式，要剛好是一個字串字面量
    const start = entry.t === 1 ? entry.s - 1 : entry.s;
    const end = entry.t === 1 ? entry.e + 1 : entry.e;
    const quote = source[start];
    const literal = (quote === '"' || quote === "'") && source[end - 1] === quote && end - start >= 2 && entry.n !== undefined;
    if (!literal) return { ok: false, error: entry.t === 2 ? "schema 腳本的動態 import 只准字串字面量" : "schema 腳本的 import 寫法看不懂" };
    if (!isMvuZodUrl(name)) return { ok: false, error: `schema 腳本不准載入這個模組：${name.slice(0, 200)}` };
    parts.push(source.slice(at, start));
    at = end;
  }
  parts.push(source.slice(at));
  return { ok: true, parts };
}
