// 設定頁判斷 OpenRouter key 是否免費層（免費層打不了生圖，就不顯示生圖模型選單）。
// key 用草稿值；base 由後端讀已存檔 config，前端只拿存檔原值當快取比對的身分。
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type KeyTier = "free" | "paid" | "unknown";

interface CachedTier {
  key: string;
  /** 發起查詢當下存檔 config 的 base_url 原值 */
  savedBaseRaw: string;
  /** 後端實際打的 base（Rust 正規化後） */
  base: string;
  tier: KeyTier;
}

// 本次 app 執行期間最後一次的查詢結果；重開設定頁時拿來當初始值，免得每次都閃一下
let lastResult: CachedTier | null = null;

export function resetKeyTierCacheForTest() {
  lastResult = null;
}

const DEBOUNCE_MS = 500;

function cachedTier(key: string, savedBaseRaw: string): KeyTier | null {
  return lastResult && lastResult.key === key && lastResult.savedBaseRaw === savedBaseRaw
    ? lastResult.tier
    : null;
}

/** 回傳 null＝查詢中或不適用（CLI、沒填 key），呼叫端一律當不確定。 */
export function useKeyTier(apiKey: string, transport: string, savedBaseRaw: string): KeyTier | null {
  const key = transport === "api" ? apiKey.trim() : "";
  const [tier, setTier] = useState<KeyTier | null>(() => (key ? cachedTier(key, savedBaseRaw) : null));
  const lastQueriedKey = useRef<string | null>(null);

  useEffect(() => {
    if (!key) {
      setTier(null);
      return;
    }
    // 換了查詢身分立刻停用舊結果，不等 debounce
    setTier(cachedTier(key, savedBaseRaw));
    // 開頁與存檔 base 變動立即查；只有改草稿 key 才等玩家打完
    const delay = lastQueriedKey.current === null || lastQueriedKey.current === key ? 0 : DEBOUNCE_MS;
    lastQueriedKey.current = key;
    let stale = false;
    const timer = setTimeout(() => {
      invoke<{ tier: KeyTier; base: string }>("openrouter_key_tier", { apiKey: key })
        .then((result) => {
          if (stale) return;
          lastResult = { key, savedBaseRaw, base: result.base, tier: result.tier };
          setTier(result.tier);
        })
        .catch(() => {
          if (stale) return;
          if (cachedTier(key, savedBaseRaw) !== null) lastResult = null;
          setTier("unknown");
        });
    }, delay);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [key, savedBaseRaw]);

  return tier;
}
