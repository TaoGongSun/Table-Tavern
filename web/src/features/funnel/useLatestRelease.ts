import { useEffect, useState } from "react";
import { fetchLatestRelease, LOADING_RELEASE, type ReleaseInfo } from "./releases";

/** 一次頁面載入只抓一次（GitHub 未帶金鑰每 IP 每小時 60 次）；查到之前是 loading（連結先指 releases 頁）。 */
export function useLatestRelease(): ReleaseInfo {
  const [release, setRelease] = useState<ReleaseInfo>(LOADING_RELEASE);
  useEffect(() => {
    let live = true;
    void fetchLatestRelease((...args) => fetch(...args)).then((info) => live && setRelease(info));
    return () => {
      live = false;
    };
  }, []);
  return release;
}
