// 舞台用的立繪：讀這桌所有定稿與臉層，聽背景補表情的狀態事件重讀該角色；實際上台時才報需要的角色，後端保存首次上台名單。
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

// 上次讀到的立繪（只留一桌）：切聊天視圖再回來會重掛，舞台只在換格時收新立繪，
// 從空的起算就得等玩家點下一格才看得到人，所以重掛先拿這份
let lastRead: { worldId: string; sprites: Record<string, SpriteView> } | null = null;
const cached = (worldId: string) => (lastRead?.worldId === worldId ? lastRead.sprites : {});

export function useSprites(worldId: string, visible: readonly (string | null)[] = [], active = false): Record<string, SpriteView> {
  const [sprites, setSprites] = useState<Record<string, SpriteView>>(() => cached(worldId));
  useEffect(() => {
    let alive = true;
    const keep = (next: Record<string, SpriteView>) => {
      lastRead = { worldId, sprites: next };
      return next;
    };
    setSprites(cached(worldId));
    invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId })
      .then((all) => alive && setSprites(keep(all ?? {})))
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
            return keep(next);
          });
        })
        .catch(() => {});
    });
    return () => {
      alive = false;
      stop();
    };
  }, [worldId]);
  const appeared = JSON.stringify(visible.filter(Boolean));
  useEffect(() => {
    if (active) void invoke("vn_sprites_fill", { worldId, appeared: JSON.parse(appeared) }).catch(() => {});
  }, [worldId, appeared, active]);
  return sprites;
}
