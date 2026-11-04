// 下載頁的網址是 `#download`、多人房間說明是 `#guide/room`：頁首常駐連結、導流面板、桌面版與手機都連到這裡，
// 複製網址分享也直接打開。用網址片段開關（不換頁），玩到一半看一眼，關掉就回到原本那桌。
import { useCallback, useEffect, useState } from "react";

export const DOWNLOAD_HASH = "#download";
export const ROOM_GUIDE_HASH = "#guide/room";

function useHashPage(hash: string): { open: boolean; close: () => void } {
  const [open, setOpen] = useState(() => window.location.hash === hash);
  useEffect(() => {
    const sync = () => setOpen(window.location.hash === hash);
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, [hash]);
  const close = useCallback(() => {
    window.history.replaceState(window.history.state, "", `${window.location.pathname}${window.location.search}`);
    setOpen(false);
  }, []);
  return { open, close };
}

export const useDownloadPage = () => useHashPage(DOWNLOAD_HASH);
export const useRoomGuidePage = () => useHashPage(ROOM_GUIDE_HASH);
