// 兩層立繪：身體層＋臉層，臉層以臉框／所用身體圖尺寸的百分比絕對定位。
// 退路鏈：該變體該表情臉層 → 該變體 neutral（不疊臉層）→ 預設變體身體＋共用臉層。
import type { CSSProperties } from "react";
import type { FaceBox, SpriteView } from "../../shared/contracts/backend-contracts";

export const percentBox = (face: FaceBox, width: number, height: number): CSSProperties => ({
  left: `${(face.x / width) * 100}%`,
  top: `${(face.y / height) * 100}%`,
  width: `${(face.w / width) * 100}%`,
  height: `${(face.h / height) * 100}%`,
});

/** 劇本的 `@變體` 比對 label 或 key（不分大小寫）；對不到、沒寫或沒有身體圖＝預設變體 */
export function spriteLayers(sprite: SpriteView, variant: string | undefined, expression: string) {
  const wanted = variant?.trim().toLowerCase() || sprite.meta.default;
  const entry = sprite.meta.variants.find(
    (item) => item.key === wanted || item.label.trim().toLowerCase() === wanted,
  );
  const layers = entry && sprite.variant_layers[entry.key];
  if (!entry || !layers) {
    return { body: sprite.body, width: sprite.width, height: sprite.height, face: sprite.faces[expression], box: sprite.meta.face };
  }
  // 獨立表情組用自己的框與臉層；共用組的身體已縮成定稿尺寸，直接疊定稿的臉層
  return entry.face
    ? { ...layers, face: layers.faces[expression], box: entry.face }
    : { ...layers, face: sprite.faces[expression], box: sprite.meta.face };
}

export function SpriteFigure({
  sprite,
  expression,
  variant,
  alt,
  className = "vn-layers",
  tint = null,
}: {
  sprite: SpriteView;
  expression: string;
  variant?: string;
  alt: string;
  className?: string;
  /** 夜晚的色調：以身體圖當遮罩疊色，只染到人物、不染透明處 */
  tint?: { color: string; opacity: number } | null;
}) {
  const { body, width, height, face, box } = spriteLayers(sprite, variant, expression);
  return (
    <div
      className={className}
      style={{
        ["--w" as string]: width,
        ["--h" as string]: height,
        // 臉框中心（比例）：特寫以它為錨放大
        ["--fx" as string]: (box.x + box.w / 2) / width,
        ["--fy" as string]: (box.y + box.h / 2) / height,
      }}
    >
      <img src={body} alt={alt} />
      {face && <img className="vn-face" src={face} alt="" style={percentBox(box, width, height)} />}
      {tint && (
        <div
          className="vn-sprite-tint"
          style={{
            backgroundColor: tint.color,
            opacity: tint.opacity,
            maskImage: `url("${body}")`,
            WebkitMaskImage: `url("${body}")`,
          }}
        />
      )}
    </div>
  );
}
