// zod 卡的 schema 腳本（mvu-zod-semantics 規格 1.5、4 節）：偵測卡上註冊 schema 的那一支，與卡片能 import 的唯一網址
// （QuickJS 宿主的 loader 用：quickjs-host.ts）。

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
