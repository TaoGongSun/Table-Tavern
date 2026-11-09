// 角色編輯頁的立繪分頁（只有 VN 桌）：定稿＋臉框（拖拉移動、拉角縮放）、五格表情（疊在定稿上預覽）、
// 每格狀態與重骰／上傳替換；依定稿重生全部表情、換定稿、變體列（SpriteVariants），刪除立繪放最下方。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import type { SpriteSlotStatus } from "../../shared/contracts/backend-contracts";
import { SpriteBasePicker } from "./SpriteBasePicker";
import { ExpressionRow, FaceBoxEditor, faceKeys, statusKey } from "./SpriteExpressionRow";
import { SpriteVariants } from "./SpriteVariants";
import { onSpriteStatus } from "./useSprites";
import { useArtStatus } from "./art-flow/useArtStatus";
import { useSpriteSlot } from "./useSpriteSlot";

/** expressionsFirst＝表情列置頂（美術一覽：玩家是看到缺的表情才點進來）；角色編輯頁維持定稿在上 */
export function SpriteTab({
  worldId,
  characterId,
  name,
  expressionsFirst = false,
  source = true,
}: {
  worldId: string;
  characterId: string;
  name: string;
  expressionsFirst?: boolean;
  source?: boolean;
}) {
  const { status: art } = useArtStatus(worldId);
  const paused = Boolean(art?.pause);
  const slot = useSpriteSlot(worldId, characterId, true);
  const [statuses, setStatuses] = useState<Record<string, SpriteSlotStatus>>({});
  const reload = useRef(slot.reload);
  reload.current = slot.reload;

  useEffect(() => {
    invoke<SpriteSlotStatus[]>("vn_sprites_status", { worldId })
      .then((all) =>
        setStatuses(
          Object.fromEntries(
            (all ?? []).filter((s) => s.characterId === characterId).map((s) => [statusKey(s.variant, s.expression), s]),
          ),
        ),
      )
      .catch(() => {});
    return onSpriteStatus(worldId, (status) => {
      if (status.characterId !== characterId) return;
      setStatuses((current) => ({ ...current, [statusKey(status.variant, status.expression)]: status }));
      if (status.state === "done") void reload.current();
    });
  }, [worldId, characterId]);

  const sprite = slot.state?.sprite ?? null;
  const ids = { worldId, characterId };
  const regenerate = (variant: string | null, expressions: readonly string[]) =>
    slot.run("work", async () => {
      for (const expression of expressions) await invoke("vn_sprite_reroll", { ...ids, variant, expression });
    });

  if (!sprite) {
    return (
      <div className="vn-sprite-tab">
        <p className="field-note">{t("vnSpriteTabIntro")}</p>
        <SpriteBasePicker slot={slot} name={name} paused={paused} source={source} />
      </div>
    );
  }
  const expressionRow = (
    <ExpressionRow
      sprite={sprite}
      paused={paused}
      statuses={statuses}
      disabled={slot.busy !== null}
      onReroll={(expression) => void regenerate(null, [expression])}
      onUpload={(file, expression) => void slot.upload(file, expression)}
    />
  );
  return (
    <div className="vn-sprite-tab">
      <div className="vn-sprite-tab-actions">
        <button type="button" className="btn btn-sm" disabled={slot.busy !== null} onClick={() => void regenerate(null, faceKeys(sprite))}>
          {t("vnSpriteRegenerateAll")}
        </button>
      </div>
      {expressionsFirst && expressionRow}
      {sprite.meta.face_fallback && <p className="field-warn">{t("vnSpriteFaceFallback")}</p>}
      <div className="vn-sprite-tab-top">
        <FaceBoxEditor
          sprite={sprite}
          onApply={(face) => slot.run("work", () => invoke("vn_sprite_set_face", { ...ids, variant: null, face }))}
        />
        <div className="vn-sprite-tab-base">
          <h3>{t("vnSpriteChangeBase")}</h3>
          <p className="field-note">{t("vnSpriteChangeBaseNote")}</p>
          <SpriteBasePicker slot={slot} name={name} paused={paused} source={source} />
        </div>
      </div>
      {!expressionsFirst && expressionRow}
      <SpriteVariants
        sprite={sprite}
        slot={slot}
        paused={paused}
        statuses={statuses}
        onReroll={(variant, expressions) => void regenerate(variant, expressions)}
      />
      <div className="vn-sprite-tab-actions">
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          disabled={slot.busy !== null}
          onClick={async () => {
            if (await confirm(t("vnSpriteDeleteConfirm"), { kind: "warning" }))
              void slot.run("work", () => invoke("vn_sprite_delete", ids));
          }}
        >
          {t("vnSpriteDelete")}
        </button>
      </div>
    </div>
  );
}
