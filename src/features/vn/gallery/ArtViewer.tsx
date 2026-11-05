// 美術一覽的燈箱：點縮圖全幅檢視。背景整張（沒專屬圖的時段疊該時段遮罩），立繪疊臉層、可切換已生好的表情。
// 點暗幕或關閉鈕收起；重播桌也能看。
import { useState } from "react";
import { t } from "../../../i18n";
import type { SpriteView } from "../../../shared/contracts/backend-contracts";
import { SpriteFigure } from "../SpriteFigure";
import { faceKeys } from "../SpriteExpressionRow";

export type Viewing =
  | { kind: "background"; src: string; tint: { color: string; opacity: number } | null; label: string }
  | { kind: "sprite"; sprite: SpriteView; label: string };

const expressionLabel = (sprite: SpriteView, key: string) =>
  sprite.expressions.find((entry) => entry.key === key)?.label ?? t(`vnExpression_${key}` as "vnExpression_neutral");

export function ArtViewer({ viewing, onClose }: { viewing: Viewing; onClose: () => void }) {
  const [expression, setExpression] = useState("neutral");
  const sprite = viewing.kind === "sprite" ? viewing.sprite : null;
  const expressions = sprite ? ["neutral", ...faceKeys(sprite).filter((key) => sprite.faces[key])] : [];
  return (
    <div className="vn-gallery-viewer" role="dialog" aria-label={viewing.label} onClick={onClose}>
      <div className="vn-gallery-viewer-stage" onClick={(event) => event.stopPropagation()}>
        {viewing.kind === "background" ? (
          <div className="vn-gallery-viewer-backdrop">
            <img src={viewing.src} alt={viewing.label} />
            {viewing.tint && (
              <div className="vn-gallery-tint" style={{ backgroundColor: viewing.tint.color, opacity: viewing.tint.opacity }} aria-hidden />
            )}
          </div>
        ) : (
          <SpriteFigure sprite={viewing.sprite} expression={expression} alt={viewing.label} className="vn-layers vn-gallery-viewer-sprite" />
        )}
      </div>
      <div className="vn-gallery-viewer-bar" onClick={(event) => event.stopPropagation()}>
        <strong>{viewing.label}</strong>
        {sprite &&
          expressions.map((key) => (
            <button
              key={key}
              type="button"
              className="btn btn-sm"
              aria-pressed={key === expression}
              onClick={() => setExpression(key)}
            >
              {expressionLabel(sprite, key)}
            </button>
          ))}
        <button type="button" className="btn btn-sm" onClick={onClose}>
          {t("vnChapterClose")}
        </button>
      </div>
    </div>
  );
}
