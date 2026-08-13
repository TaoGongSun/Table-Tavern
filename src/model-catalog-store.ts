import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  applyCachedCatalogs,
  CATALOG_SOURCES,
  mergeCatalog,
  parseOpenRouterModels,
  type ModelCatalogs,
  type ModelOption,
} from "./model-catalog";

// 模型清單接線：合併規則與 OpenRouter 解析在 model-catalog.ts，這裡只管抓取、落地與訂閱
let catalogStore: ModelCatalogs = {};
const catalogListeners = new Set<() => void>();
let catalogPrefetched = false;

function publishCatalogs(next: ModelCatalogs) {
  catalogStore = next;
  catalogListeners.forEach((listener) => listener());
}

/// 訂閱模組級清單：抓取在背景完成，回來時所有掛著的畫面一起換上新的。
export function useModelCatalogs(): ModelCatalogs {
  const [snapshot, setSnapshot] = useState(catalogStore);
  useEffect(() => {
    const listener = () => setSnapshot(catalogStore);
    catalogListeners.add(listener);
    listener(); // 掛上前抓完的那幾支要補回來
    return () => {
      catalogListeners.delete(listener);
    };
  }, []);
  return snapshot;
}

async function fetchCatalog(id: string): Promise<ModelOption[]> {
  if (id === "api") {
    // OpenRouter 公開清單（免 key）；拿不到就退化成純手動輸入
    return parseOpenRouterModels(await (await fetch("https://openrouter.ai/api/v1/models")).json());
  }
  return invoke<ModelOption[]>("list_cli_models", { cli: id });
}

/// 抓一支並落地；抓到空的由 mergeCatalog 留住上次的結果。
/// catalogStore 一律在 await 之後才讀：五家並行預熱，拿抓取前的舊值去算會互相覆蓋。
export async function refreshCatalog(id: string): Promise<void> {
  try {
    const fetched = await fetchCatalog(id);
    const merged = mergeCatalog(catalogStore, id, fetched);
    if (merged === catalogStore) return;
    publishCatalogs(merged);
    await invoke("write_model_catalog", { catalog: catalogStore });
  } catch {
    /* 抓不到就沿用快取 */
  }
}

/// 開 app 時跑一次：先把上次存的清單擺上（玩家點進設定即刻有東西可選），
/// 再五家並行重抓，回來一支換一支。
export async function prefetchModelCatalogs(): Promise<void> {
  if (catalogPrefetched) return;
  catalogPrefetched = true;
  try {
    const cached = await invoke<ModelCatalogs>("read_model_catalog");
    publishCatalogs(applyCachedCatalogs(catalogStore, cached));
  } catch {
    /* 沒有快取檔就等抓取回來 */
  }
  await Promise.all(CATALOG_SOURCES.map((id) => refreshCatalog(id)));
}
