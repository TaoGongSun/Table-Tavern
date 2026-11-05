// 立繪準備進度（標題畫面小字「立繪準備中 2/3」）：登場角色裡有定稿的幾位，聽臉層狀態事件（定稿換、表情補完）重算。
// 美術進度只由資產推導，跟設定階段旗標無關（plans/vn-setup-wizard.md §2 ③）。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { SpriteView } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { onSpriteStatus } from "../useSprites";

export function useArtProgress(worldId: string): { ready: number; total: number } | null {
  const [progress, setProgress] = useState<{ ready: number; total: number } | null>(null);
  useEffect(() => {
    let live = true;
    const read = () =>
      Promise.all([
        invoke<CharacterMeta[]>("list_characters", { worldId }),
        invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId }),
      ])
        .then(([cast, sprites]) => {
          const shown = cast.filter((card) => !card.archived && !card.stage);
          if (live) setProgress({ ready: shown.filter((card) => sprites?.[card.id]).length, total: shown.length });
        })
        .catch(() => {});
    void read();
    const stop = onSpriteStatus(worldId, (status) => {
      if (status.state === "queued" || status.state === "done") void read();
    });
    return () => {
      live = false;
      stop();
    };
  }, [worldId]);
  return progress;
}
