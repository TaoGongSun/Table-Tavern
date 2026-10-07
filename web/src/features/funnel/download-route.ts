// 下載頁的網址是 `#download`：頁首常駐連結、導流面板都連到這裡，複製網址分享也直接打開下載頁。
// 用網址片段開關（不換頁），玩到一半看下載頁，關掉就回到原本那桌。
import { useCallback, useEffect, useState } from "react";

export const DOWNLOAD_HASH = "#download";

export function useDownloadPage(): { open: boolean; close: () => void } {
  const [open, setOpen] = useState(() => window.location.hash === DOWNLOAD_HASH);
  useEffect(() => {
    const sync = () => setOpen(window.location.hash === DOWNLOAD_HASH);
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  const close = useCallback(() => {
    window.history.replaceState(window.history.state, "", `${window.location.pathname}${window.location.search}`);
    setOpen(false);
  }, []);
  return { open, close };
}
