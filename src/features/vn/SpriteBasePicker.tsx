// 定稿挑選：目前候選大圖＋候選縮圖列；立繪來源、生成（再骰）、挑定、上傳定稿。精靈 ③ 與立繪分頁共用
// （精靈 ③ 的來源下拉整步只放頂部一個，`source={false}`）。
import { useRef } from "react";
import "./vn-sprites.css";
import { t } from "../../i18n";
import { SpriteSourceSelect } from "./art-flow/SpriteSourceSelect";
import type { SpriteSlot } from "./useSpriteSlot";

export function SpriteBasePicker({
  slot,
  name,
  source = true,
  paused = false,
}: {
  slot: SpriteSlot;
  name: string;
  source?: boolean;
  paused?: boolean;
}) {
  const input = useRef<HTMLInputElement>(null);
  const candidates = slot.state?.candidates ?? [];
  const shown = slot.selected ? slot.previews[slot.selected] : slot.state?.sprite?.body;
  const disabled = slot.busy !== null;
  // 選的就是裝上去那張不必再挑（定稿記著是哪張候選；上傳的或舊桌不記＝可以重新挑定）
  const alreadyPicked = Boolean(slot.state?.sprite) && slot.selected === slot.installed;
  return (
    <div className="vn-picker">
      {source && <SpriteSourceSelect />}
      <div className="vn-picker-actions">
        <button type="button" className="btn btn-sm" disabled={disabled || slot.auto !== null} onClick={() => void slot.generate()}>
          {t(candidates.length ? "vnSpriteReroll" : "vnSpriteGenerate")}
        </button>
        <button
          type="button"
          className={slot.selected && !alreadyPicked ? "btn btn-sm btn-primary" : "btn btn-sm"}
          disabled={disabled || !slot.selected || alreadyPicked}
          onClick={() => void slot.pick()}
        >
          {t(alreadyPicked ? "vnSpriteAlreadyBase" : "vnSpritePick")}
        </button>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => input.current?.click()}>
          {t("vnSpriteUploadBase")}
        </button>
        <input
          ref={input}
          type="file"
          accept="image/png,image/webp"
          hidden
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) void slot.upload(file);
          }}
        />
      </div>
      <div className="vn-picker-main">
        {shown ? (
          <img className="vn-picker-image" src={shown} alt={name} />
        ) : (
          <div className="vn-picker-empty">
            {slot.busy || slot.auto ? (
              <span className="vn-pending">
                {slot.busy
                  ? t(`vnSpriteBusy_${slot.busy}`)
                  : t(slot.auto === "queued" ? paused ? "vnArtWaitQuota" : "vnSpriteQueued" : "vnSpriteBusy_generate")}
              </span>
            ) : (
              t("vnSpriteNone")
            )}
          </div>
        )}
        {shown && (slot.busy || slot.auto === "running") && (
          <span className="vn-picker-busy">{t(`vnSpriteBusy_${slot.busy ?? "generate"}`)}</span>
        )}
      </div>
      {/* 只有一張候選但沒選它（例如上傳了定稿）也列出來，點了才改用它 */}
      {(candidates.length > 1 || (candidates.length === 1 && slot.selected !== candidates[0])) && (
        <div className="vn-picker-strip">
          {candidates.map((file) => (
            <button
              key={file}
              type="button"
              className="vn-picker-thumb"
              aria-pressed={file === slot.selected}
              onClick={() => slot.setSelected(file)}
            >
              <img src={slot.previews[file]} alt="" />
            </button>
          ))}
        </div>
      )}
      {slot.error && (
        <p className="field-warn" role="alert">
          {slot.error}
        </p>
      )}
    </div>
  );
}
