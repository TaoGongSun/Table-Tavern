// 舞台：背景層、三個槽位的立繪、說話者亮其他壓暗、特寫與震動、地點字卡。
// 退路鏈：該變體該表情臉層 → 該變體 neutral → 預設變體＋共用臉層（SpriteFigure）→ 角色卡原圖 → 剪影＋名牌。
import type { SpriteView } from "../../shared/contracts/backend-contracts";
import { SpriteFigure } from "./SpriteFigure";
import { StageBackdrop } from "./StageBackdrop";
import type { BackdropImage } from "./useBackgrounds";
import type { Stage as StageState } from "./vn-cells";

export function Stage({
  stage,
  sprites,
  images,
  backdrop,
  cellKey,
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
}) {
  const closeup = stage.directive === "closeup" ? stage.speaker : null;
  return (
    <div className="vn-stage">
      {stage.backdrop && <StageBackdrop image={backdrop ?? null} />}
      <div
        key={stage.directive === "shake" ? cellKey : undefined}
        className={`vn-cast${stage.directive === "shake" ? " vn-shake" : ""}`}
      >
        {stage.location && (
          <div key={stage.location} className="vn-location">
            {stage.time ? `${stage.location}｜${stage.time}` : stage.location}
          </div>
        )}
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
            <figure key={id} className={`vn-sprite vn-slot-${slot} vn-${state}`}>
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
