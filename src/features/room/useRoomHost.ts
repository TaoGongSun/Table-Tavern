// 大廳與頂欄用的房間總覽（room-host-desktop-play）：哪張桌的房間正開著、哪些桌開過房（房間碼），以及開房、重開、結束三個動作。
// 換桌鎖（runTableOp）與進桌由 App 傳進來；這裡只管房間那一半。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { RoomStatus } from "./room-status";

export interface RoomHost {
  /** 房間正開著的那張桌；沒有時 null */
  activeWorld: string | null;
  /** 開過房的桌 → 房間碼（伺服器上已清掉的是空字串） */
  rooms: Record<string, string>;
  refresh: () => Promise<void>;
}

export function useRoomHost(): RoomHost {
  const [activeWorld, setActiveWorld] = useState<string | null>(null);
  const [rooms, setRooms] = useState<Record<string, string>>({});
  const refresh = useCallback(async () => {
    try {
      const status = await invoke<RoomStatus>("room_status", { worldId: null });
      setActiveWorld(status.active ? status.worldId : null);
      setRooms(Object.fromEntries(await invoke<[string, string][]>("room_tables")));
    } catch {
      // 讀不到就維持上一份
    }
  }, []);
  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 3000);
    return () => clearInterval(timer);
  }, [refresh]);
  return { activeWorld, rooms, refresh };
}

/** 開房或重開：這張桌有憑證就接回，否則在 `server` 建新房 */
export const enterRoom = (worldId: string, server: string) => invoke<RoomStatus>("room_enter", { worldId, server });
