// 下載頁的網址是 `#download`（可再接 `/new`、`/oneLine`、`/room`、`/vn`）、多人房間說明是 `#guide/room`。
// 頁首常駐連結、導流面板、桌面版與手機都連到這裡，複製網址分享也直接打開。用網址片段開關（不換頁），
// 玩到一半看一眼，關掉就回到原本那桌。`#download/...` 仍算下載頁開著，點四扇門不會把頁關掉。
import { useCallback, useEffect, useState } from "react";

export const DOWNLOAD_HASH = "#download";
export const ROOM_GUIDE_HASH = "#guide/room";

export const DOWNLOAD_TABLES = ["new", "oneLine", "room", "vn"] as const;
export type DownloadTable = (typeof DOWNLOAD_TABLES)[number];

export function isDownloadTable(value: string): value is DownloadTable {
  return (DOWNLOAD_TABLES as readonly string[]).includes(value);
}

/** `#download/room` 這種可分享的門。不認識的尾巴仍留在下載頁，面板回到預設的第一種。 */
export function downloadHash(table: DownloadTable): string {
  return `${DOWNLOAD_HASH}/${table}`;
}

export function parseDownloadTable(hash: string): DownloadTable {
  if (!hash.startsWith(`${DOWNLOAD_HASH}/`)) return "new";
  const slug = hash.slice(DOWNLOAD_HASH.length + 1);
  return isDownloadTable(slug) ? slug : "new";
}

function matchesHash(value: string, hash: string, prefix: boolean): boolean {
  return prefix ? value === hash || value.startsWith(`${hash}/`) : value === hash;
}

function useHashPage(hash: string, prefix = false): { open: boolean; close: () => void } {
  const [open, setOpen] = useState(() => matchesHash(window.location.hash, hash, prefix));
  useEffect(() => {
    const sync = () => setOpen(matchesHash(window.location.hash, hash, prefix));
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, [hash, prefix]);
  const close = useCallback(() => {
    window.history.replaceState(window.history.state, "", `${window.location.pathname}${window.location.search}`);
    setOpen(false);
  }, []);
  return { open, close };
}

export function useDownloadTable(): DownloadTable {
  const [table, setTable] = useState(() => parseDownloadTable(window.location.hash));
  useEffect(() => {
    const sync = () => setTable(parseDownloadTable(window.location.hash));
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  return table;
}

export function useDownloadPage(): { open: boolean; close: () => void; table: DownloadTable } {
  return { ...useHashPage(DOWNLOAD_HASH, true), table: useDownloadTable() };
}

export const useRoomGuidePage = () => useHashPage(ROOM_GUIDE_HASH);
