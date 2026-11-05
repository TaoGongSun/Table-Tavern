// 舞台：背景層、三個槽位的立繪（VN 卡給了身高就照比例縮、下限 0.7、下緣對齊；獨自一人站中間）、說話者亮其他壓暗、
// 特寫（以臉為錨放大）與震動、地點字卡（換地點出現 4 秒後淡出）。
// 退路鏈：該變體該表情臉層 → 該變體 neutral → 預設變體＋共用臉層（SpriteFigure）→ 角色卡原圖 → 剪影＋名牌。
import { useEffect, useState } from "react";
import type { SpriteView } from "../../shared/contracts/backend-contracts";
import { SpriteFigure } from "./SpriteFigure";
import { StageBackdrop } from "./StageBackdrop";
import type { BackdropImage } from "./useBackgrounds";
import type { Stage as StageState } from "./vn-cells";

/** 身高比例下限：名單裡有特別高的角色時，一般人不至於縮成小人 */
export const MIN_SCALE = 0.7;
const CENTER_SLOT = 2;
const LOCATION_MS = 4000;

/** 畫面上的位置：獨自一人在台上站中央槽，兩人以上照分配到的槽（換位走 left 過渡） */
export const placeOf = (slots: (string | null)[], slot: number) =>
  slots.filter(Boolean).length === 1 ? CENTER_SLOT : slot;

/** 地點字卡：出現 4 秒後淡出（減少動態時直接消失）；播放器被蓋住時先不計時 */
function LocationCard({ location, hold }: { location: string; hold: boolean }) {
  const [shown, setShown] = useState(true);
  useEffect(() => {
    if (hold || !shown) return;
    const id = setTimeout(() => setShown(false), LOCATION_MS);
    return () => clearTimeout(id);
  }, [hold, shown]);
  return <div className={`vn-location${shown ? "" : " vn-location-out"}`}>{location}</div>;
}

export function Stage({
  stage,
  sprites,
  images,
  backdrop,
  cellKey,
  hold = false,
}: {
  stage: StageState;
  /** 角色 id → 立繪（定稿＋臉層） */
  sprites: Record<string, SpriteView>;
  /** 角色 id → 角色卡原圖：沒立繪時的佔位 */
  images: Record<string, string>;
  /** 目前背景鍵的圖；undefined＝還沒讀過（同沒有） */
  backdrop?: BackdropImage;
  /** 目前格的鍵：震動以它為 key，每格最多震一次 */
  cellKey: string;
  /** 標題、面板、字卡蓋著播放器：地點字卡先不開始計時 */
  hold?: boolean;
}) {
  const closeup = stage.directive === "closeup" ? stage.speaker : null;
  return (
    <div className="vn-stage">
      {stage.backdrop && <StageBackdrop image={backdrop ?? null} />}
      <div
        key={stage.directive === "shake" ? cellKey : undefined}
        className={`vn-cast${stage.directive === "shake" ? " vn-shake" : ""}`}
      >
        {stage.location && <LocationCard key={stage.location} location={stage.location} hold={hold} />}
        {stage.slots.map((id, slot) => {
          if (!id) return null;
          const state = closeup
            ? closeup === id
              ? "closeup"
              : "faded"
            : stage.speaker === null
              ? "idle"
              : stage.speaker === id
                ? "speaking"
                : "dim";
          return (
            <figure
              key={id}
              className={`vn-sprite vn-slot-${placeOf(stage.slots, slot)} vn-${state}`}
              style={sprites[id]?.scale ? { ["--vn-scale" as string]: Math.max(MIN_SCALE, sprites[id].scale) } : undefined}
            >
              {sprites[id] ? (
                <SpriteFigure
                  sprite={sprites[id]}
                  expression={stage.expressions[id] ?? "neutral"}
                  variant={stage.variants[id]}
                  alt={stage.names[id]}
                />
              ) : images[id] ? (
                <img src={images[id]} alt={stage.names[id]} />
              ) : (
                <div className="vn-silhouette" aria-label={stage.names[id]} />
              )}
            </figure>
          );
        })}
      </div>
    </div>
  );
}
