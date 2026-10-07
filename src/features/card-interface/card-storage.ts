// 卡片殼的 localStorage 快照：型別、上限與信任邊界的清理。不帶沙盒內建庫，宿主各處（含網頁版）只要這幾樣時引這支。

/** 卡片殼寫在沙盒 localStorage 裡的東西（設定分頁的主題、字級等）；宿主原樣存、原樣回填。 */
export type CardStorage = Record<string, string>;

// 卡片殼能往宿主存的上限。殼只該存設定這種小東西，第三方 JS 不能無限往宿主存檔寫。
export const CARD_STORAGE_LIMIT = 64 * 1024;

/**
 * 把來路不明的值（沙盒 postMessage 過來的、宿主存檔讀回來的）收成乾淨的 CardStorage；
 * 型別不對或整份超過上限回 null，呼叫端當作沒有這份快照。
 */
export function sanitizeCardStorage(value: unknown): CardStorage | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const entries = Object.entries(value).filter(([, item]) => typeof item === "string") as [string, string][];
  const clean = Object.fromEntries(entries);
  // 上限照桌檔契約（card_storage 整份 JSON ≤ 64 KiB）量 UTF-8 位元組
  return new TextEncoder().encode(JSON.stringify(clean)).length > CARD_STORAGE_LIMIT ? null : clean;
}
