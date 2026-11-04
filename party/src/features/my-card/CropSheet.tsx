// 裁切窗（room-phone-gallery §3.5）：照桌面版「裁切角色圖」，2:3 長方形、可拖可捏、另有縮放滑桿；預設置中的最大 2:3。
// 只回裁切框（原圖比例），誰要切圖由呼叫端決定：上傳在手機切，圖庫圖交給執行端拿原圖切。
import { useState } from "react";
import Cropper from "react-easy-crop";
import "react-easy-crop/react-easy-crop.css";
import { t } from "../../i18n";
import type { CropBox } from "./card-upload";

export function CropSheet({
  src,
  onConfirm,
  onCancel,
}: {
  src: string;
  onConfirm: (box: CropBox) => Promise<void>;
  onCancel: () => void;
}) {
  const [pending, setPending] = useState(false); // 套用後草稿寫好前：兩顆鈕都停用
  const [crop, setCrop] = useState({ x: 0, y: 0 });
  const [zoom, setZoom] = useState(1);
  const [box, setBox] = useState<CropBox | null>(null);
  return (
    <div
      className="leave-confirm"
      role="dialog"
      aria-labelledby="crop-title"
      data-testid="crop-sheet"
      onKeyDown={(e) => e.key === "Escape" && !pending && onCancel()}
    >
      <div className="leave-box crop-sheet">
        <h3 id="crop-title">{t("cropImageTitle")}</h3>
        {/* 動作鈕置頂 */}
        <div className="panel-actions">
          <button
            type="button"
            disabled={!box || pending}
            onClick={() => box && (setPending(true), void onConfirm(box))}
          >
            {t("cropConfirm")}
          </button>
          <button type="button" className="ghost" autoFocus disabled={pending} onClick={onCancel}>
            {t("cropCancel")}
          </button>
        </div>
        {/* 窗本身就是 2:3、圖蓋滿：框固定是窗內最大的 2:3，沒有灰底，手指好操作；裁切用原圖，放大不影響畫質 */}
        <div className="crop-area">
          <Cropper
            image={src}
            objectFit="cover"
            crop={crop}
            zoom={zoom}
            maxZoom={4}
            aspect={2 / 3}
            onCropChange={setCrop}
            onZoomChange={setZoom}
            onCropComplete={(area) =>
              setBox({
                x: area.x / 100,
                y: area.y / 100,
                w: area.width / 100,
                h: area.height / 100,
              })
            }
          />
        </div>
        <label className="crop-zoom">
          {t("zoomLabel")}
          <input
            type="range"
            min={1}
            max={4}
            step={0.05}
            value={zoom}
            onChange={(e) => setZoom(Number(e.currentTarget.value))}
          />
        </label>
      </div>
    </div>
  );
}
