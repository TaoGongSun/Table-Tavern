import { useEffect, useState } from "react";
import { FALLBACK_RELEASE, fetchLatestRelease, type ReleaseInfo } from "./releases";

/** 一次頁面載入只抓一次（GitHub 未帶金鑰每 IP 每小時 60 次）；抓到前先用 releases 頁。 */
export function useLatestRelease(): ReleaseInfo {
  const [release, setRelease] = useState<ReleaseInfo>(FALLBACK_RELEASE);
  useEffect(() => {
    let live = true;
    void fetchLatestRelease((...args) => fetch(...args)).then((info) => live && setRelease(info));
    return () => {
      live = false;
    };
  }, []);
  return release;
}
