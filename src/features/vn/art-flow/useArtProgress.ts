// 標題重進只恢復已按開始的美術；失敗等玩家重試，事件只重算進度。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { ArtStatus, SpriteSlotStatus, SpriteView } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { onSpriteStatus } from "../useSprites";

export interface ArtProgressState {
  started: boolean;
  ready: number;
  total: number;
  /** 自動定稿失敗、還沒有定稿的角色數 */
  failed: number;
}

export function useArtProgress(worldId: string) {
  const [progress, setProgress] = useState<ArtProgressState | null>(null);
  useEffect(() => {
    let live = true;
    let resumed = false;
    const read = () =>
      Promise.all([
        invoke<CharacterMeta[]>("list_characters", { worldId }),
        invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId }),
        invoke<SpriteSlotStatus[]>("vn_sprites_status", { worldId }),
        invoke<ArtStatus>("vn_art_status", { worldId }),
      ])
        .then(([cast, sprites, statuses, art]) => {
          const shown = cast.filter((card) => !card.archived && !card.stage);
          const bases = (statuses ?? []).filter((s) => s.expression === "base");
          const failed = new Set(bases.filter((s) => s.state === "failed").map((s) => s.characterId));
          const ready = shown.filter((card) => sprites?.[card.id]).length;
          const pending = bases.some((s) => s.state === "queued" || s.state === "running");
          if (live && !resumed) {
            resumed = true;
            if (art?.art.started && ready < shown.length && !pending && failed.size === 0) invoke("vn_sprites_prepare", { worldId, adopt: [] }).catch(() => {});
          }
          if (live)
            setProgress({
              started: art?.art.started ?? false,
              ready,
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
