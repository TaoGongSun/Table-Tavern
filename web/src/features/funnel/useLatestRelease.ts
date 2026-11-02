import { useEffect, useRef, useState } from "react";
import { fetchLatestRelease, LOADING_RELEASE, type ReleaseInfo } from "./releases";

/**
 * 一次頁面載入只抓一次（GitHub 未帶金鑰每 IP 每小時 60 次）；查到之前是 loading（連結先指 releases 頁）。
 * `enabled=false`（中國供應商模式：下載鈕藏起來，GitHub 在中國也不穩）就不發請求，切回來才抓。
 */
export function useLatestRelease(enabled = true): ReleaseInfo {
  const [release, setRelease] = useState<ReleaseInfo>(LOADING_RELEASE);
  const fetched = useRef(false);
  useEffect(() => {
    if (!enabled || fetched.current) return;
    // 只抓一次（StrictMode 重跑 effect 也不重抓），結果一定收下
    fetched.current = true;
    void fetchLatestRelease((...args) => fetch(...args)).then(setRelease);
  }, [enabled]);
  return release;
}
