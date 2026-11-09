// 美術一覽的立繪分頁：每位登場角色一列（定稿縮圖、定稿與各表情的狀態點）。點縮圖開燈箱檢視，「編輯」＝角色編輯頁那份
// 立繪分頁（SpriteTab，表情列置頂）；重播桌只能檢視、只畫已有的表情。狀態來自 `vn_sprites_status` 與立繪事件。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { SpriteSlotStatus, SpriteView } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { FACES, faceKeys, statusKey } from "../SpriteExpressionRow";
import { SpriteTab } from "../SpriteTab";
import { onSpriteStatus } from "../useSprites";
import type { Viewing } from "./ArtViewer";

/** 自動定稿事件的 expression 欄（後端 `prepare::BASE_EXPRESSION`） */
const BASE = "base";

type Dot = "done" | "queued" | "running" | "failed" | "missing";

function dot(status: SpriteSlotStatus | undefined, has: boolean): Dot {
  if (status?.state === "queued" || status?.state === "running" || status?.state === "failed") return status.state;
  return has ? "done" : "missing";
}

export function SpriteGallery({
  worldId,
  paused = false,
  images,
  readOnly,
  onView,
}: {
  worldId: string;
  paused?: boolean;
  images: Record<string, string>;
  readOnly: boolean;
  onView: (viewing: Viewing) => void;
}) {
  const [cast, setCast] = useState<CharacterMeta[]>([]);
  const [sprites, setSprites] = useState<Record<string, SpriteView>>({});
  const [statuses, setStatuses] = useState<Record<string, SpriteSlotStatus>>({});
  const [open, setOpen] = useState<CharacterMeta | null>(null);

  useEffect(() => {
    let live = true;
    const key = (status: SpriteSlotStatus) => `${status.characterId}:${statusKey(status.variant, status.expression)}`;
    const readSprites = () =>
      invoke<Record<string, SpriteView>>("vn_sprites_read_all", { worldId })
        .then((all) => live && setSprites(all ?? {}))
        .catch(() => {});
    invoke<CharacterMeta[]>("list_characters", { worldId })
      .then((list) => live && setCast(list.filter((card) => !card.archived && !card.stage)))
      .catch(() => {});
    void readSprites();
    invoke<SpriteSlotStatus[]>("vn_sprites_status", { worldId })
      .then((all) => live && setStatuses(Object.fromEntries((all ?? []).map((status) => [key(status), status]))))
      .catch(() => {});
    const stop = onSpriteStatus(worldId, (status) => {
      setStatuses((current) => ({ ...current, [key(status)]: status }));
      // 完成＝多一張臉層或定稿裝好；表情排入＝定稿剛換
      if (status.state === "done" || status.state === "queued") void readSprites();
    });
    return () => {
      live = false;
      stop();
    };
  }, [worldId]);

  if (open)
    return (
      <div className="vn-gallery-sheet">
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => setOpen(null)}>
          {t("vnGalleryBack")}
        </button>
        <h3>{open.name}</h3>
        <SpriteTab worldId={worldId} characterId={open.id} name={open.name} expressionsFirst source={false} />
      </div>
    );

  return (
    <ul className="vn-gallery-cast">
      {cast.map((card) => {
        const sprite = sprites[card.id];
        const status = (expression: string) => statuses[`${card.id}:${statusKey(null, expression)}`];
        const base = status(BASE);
        const keys = sprite ? faceKeys(sprite) : [...FACES];
        const thumb = sprite?.body ?? images[card.id];
        const dots = ["neutral", ...keys].map((expression) => ({
          expression,
          state:
            expression === "neutral"
              ? dot(sprite ? undefined : base, Boolean(sprite))
              : dot(status(expression), Boolean(sprite?.faces[expression])),
          label:
            sprite?.expressions.find((entry) => entry.key === expression)?.label ??
            t(`vnExpression_${expression}` as "vnExpression_neutral"),
        }));
        return (
          <li key={card.id} className="vn-gallery-row">
            <button
              type="button"
              className="vn-gallery-thumb"
              aria-label={card.name}
              disabled={!sprite && !thumb}
              onClick={() =>
                sprite
                  ? onView({ kind: "sprite", sprite, label: card.name })
                  : onView({ kind: "background", src: thumb!, tint: null, label: card.name })
              }
            >
              {thumb && <img src={thumb} alt="" />}
            </button>
            <span className="vn-gallery-name">
              {card.name}
              {!sprite && (
                <span className="field-note">
                  {t(
                    base?.state === "queued"
                      ? paused ? "vnArtWaitQuota" : "vnGalleryBaseQueued"
                      : base?.state === "running"
                        ? "vnGalleryBaseRunning"
                        : base?.state === "failed"
                          ? "vnGalleryBaseFailed"
                          : "vnGalleryNoBase",
                  )}
                </span>
              )}
            </span>
            <span className="vn-gallery-dots">
              {dots
                .filter(({ state }) => !readOnly || state === "done")
                .map(({ expression, state, label }) => (
                  <span
                    key={expression}
                    className={`vn-gallery-dot vn-gallery-dot-${state}`}
                    title={state === "done" ? label : `${label} · ${t(paused && state === "queued" ? "vnArtWaitQuota" : `vnSpriteState_${state}`)}`}
                  />
                ))}
            </span>
            {!readOnly && (
              <button type="button" className="btn btn-sm" onClick={() => setOpen(card)}>
                {t("vnGalleryEdit")}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
