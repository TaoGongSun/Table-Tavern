// 舞台用的立繪：讀這桌所有定稿與臉層，聽背景補表情的狀態事件重讀該角色；掛載時排入缺的臉層（進桌續生）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { SpriteSlotStatus, SpriteState, SpriteView } from "../../shared/contracts/backend-contracts";

export const SPRITE_STATUS_EVENT = "vn-sprite-status";

/** 聽這桌的臉層狀態事件；回傳取消函式 */
export function onSpriteStatus(worldId: string, handle: (status: SpriteSlotStatus) => void) {
  const off = listen<SpriteSlotStatus>(SPRITE_STATUS_EVENT, (event) => {
    if (event.payload.worldId === worldId) handle(event.payload);
  });
  return () => void off.then((stop) => stop());
}

export function useSprites(worldId: string): Record<string, SpriteView> {
  const [sprites, setSprites] = useState<Record<string, SpriteView>>({});
  useEffect(() => {
    let alive = true;
    setSprites({});
    invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId })
      .then((all) => alive && setSprites(all ?? {}))
      .catch(() => {});
    const stop = onSpriteStatus(worldId, (status) => {
      // 排入＝定稿可能剛換（精靈 ③ 挑定後開演）；完成＝多一張臉層
      if (status.state !== "done" && status.state !== "queued") return;
      invoke<SpriteState>("vn_sprite_read", { worldId, characterId: status.characterId })
        .then(({ sprite }) => {
          if (!alive) return;
          setSprites((current) => {
            const next = { ...current };
            if (sprite) next[status.characterId] = sprite;
            else delete next[status.characterId];
            return next;
          });
        })
        .catch(() => {});
    });
    invoke("vn_sprites_fill", { worldId }).catch(() => {});
    return () => {
      alive = false;
      stop();
    };
  }, [worldId]);
  return sprites;
}
