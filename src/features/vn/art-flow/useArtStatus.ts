import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type { ArtStatus } from "../../../shared/contracts/backend-contracts";
import { onSpriteStatus } from "../useSprites";
import { BACKGROUND_STATUS_EVENT } from "../useBackgrounds";

/** 同一份唯讀狀態供美術進度及角色編輯頁顯示。 */
export function useArtStatus(worldId: string) {
  const [status, setStatus] = useState<ArtStatus | null>(null);
  const read = useCallback(() => invoke<ArtStatus>("vn_art_status", { worldId }).then(setStatus), [worldId]);
  useEffect(() => {
    let live = true;
    const refresh = () => { if (live) void read().catch(() => {}); };
    refresh();
    const off = onSpriteStatus(worldId, refresh);
    const flow = listen("vn-art-flow-status", refresh);
    const bg = listen<{ worldId: string }>(BACKGROUND_STATUS_EVENT, ({ payload }) => { if (payload.worldId === worldId) refresh(); });
    return () => { live = false; off(); void flow.then((stop) => stop()); void bg.then((stop) => stop()); };
  }, [read, worldId]);
  return { status, read };
}
