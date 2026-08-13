// 模型清單的純邏輯：合併規則與 OpenRouter 回應解析。零 React／零 invoke 依賴，
// 抓取、落地與訂閱在 App.tsx 接線，這裡單獨測。
//
// 清單一律開 app 預熱一次就放著（見 App.tsx 的 prefetchModelCatalogs）：抓一支要
// 0.9～2.2 秒（agy／grok 走網路查詢），擺在設定頁裡抓等於每次點開都罰站。

export interface ModelOption {
  id: string;
  label: string;
}

/** 五家一起預熱；"api"＝OpenRouter 公開清單，其餘走各自的 CLI */
export const CATALOG_SOURCES = ["api", "claude", "codex", "agy", "grok"] as const;

export type ModelCatalogs = Record<string, ModelOption[]>;

/** OpenRouter 公開清單（免 key）：沒有 id 的項目丟掉，沒有 name 的拿 id 當顯示名 */
export function parseOpenRouterModels(body: unknown): ModelOption[] {
  const data = (body as { data?: unknown })?.data;
  if (!Array.isArray(data)) return [];
  return data.flatMap((entry) => {
    const id = (entry as { id?: unknown })?.id;
    if (typeof id !== "string" || id === "") return [];
    const name = (entry as { name?: unknown })?.name;
    return [{ id, label: typeof name === "string" && name !== "" ? name : id }];
  });
}

/**
 * 併入一支的抓取結果。抓到空的就原封不動退回舊的：CLI 沒登入、斷網、子行程逾時都會
 * 回空清單，這時讓玩家看見上次那份，比把下拉清空好。
 */
export function mergeCatalog(
  store: ModelCatalogs,
  id: string,
  fetched: ModelOption[],
): ModelCatalogs {
  if (fetched.length === 0) return store;
  return { ...store, [id]: fetched };
}

/**
 * 開 app 時把上次存的清單擺上，讓玩家點進設定即刻有東西可選。
 * 已經抓回來的新結果優先——預熱是「先秀舊的、背景更新」，順序反了會把新的蓋回舊的。
 */
export function applyCachedCatalogs(store: ModelCatalogs, cached: ModelCatalogs): ModelCatalogs {
  return { ...cached, ...store };
}
