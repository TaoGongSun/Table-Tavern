// 立繪準備進度（標題畫面小字「立繪準備中 2/3」）：登場角色裡有定稿的幾位、自動定稿失敗的幾位；
// 聽立繪狀態事件（定稿換、表情補完、自動定稿失敗）重算。美術進度只由資產與佇列推導，跟設定階段旗標無關
// （plans/vn-setup-wizard.md §2 ③）。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { SpriteSlotStatus, SpriteView } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { onSpriteStatus } from "../useSprites";

export interface ArtProgressState {
  ready: number;
  total: number;
  /** 自動定稿失敗、還沒有定稿的角色數 */
  failed: number;
}

export function useArtProgress(worldId: string) {
  const [progress, setProgress] = useState<ArtProgressState | null>(null);
  useEffect(() => {
    let live = true;
    const read = () =>
      Promise.all([
        invoke<CharacterMeta[]>("list_characters", { worldId }),
        invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId }),
        invoke<SpriteSlotStatus[]>("vn_sprites_status", { worldId }),
      ])
        .then(([cast, sprites, statuses]) => {
          const shown = cast.filter((card) => !card.archived && !card.stage);
          const failed = new Set(
            (statuses ?? []).filter((s) => s.expression === "base" && s.state === "failed").map((s) => s.characterId),
          );
          if (live)
            setProgress({
              ready: shown.filter((card) => sprites?.[card.id]).length,
              total: shown.length,
              failed: shown.filter((card) => !sprites?.[card.id] && failed.has(card.id)).length,
            });
        })
        .catch(() => {});
    void read();
    const stop = onSpriteStatus(worldId, (status) => {
      // 定稿：排隊／完成／失敗都重算；臉層排入＝剛換過定稿（開演時補表情）
      if (status.expression === "base" ? status.state !== "running" : status.state === "queued") void read();
    });
    return () => {
      live = false;
      stop();
    };
  }, [worldId]);
  /** 重試：再排一次自動定稿（失敗的角色會重生） */
  const retry = useCallback(() => {
    invoke("vn_sprites_prepare", { worldId, adopt: [] }).catch(() => {});
  }, [worldId]);
  return { progress, retry };
}
