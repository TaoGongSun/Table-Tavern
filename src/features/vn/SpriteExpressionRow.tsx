// 立繪分頁與變體列共用的元件：一組表情（定稿的共用組，或某變體的獨立組）五格預覽與重骰／上傳替換、臉框編輯、選檔欄。
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { t } from "../../i18n";
import type { FaceBox, SpriteSlotStatus, SpriteView } from "../../shared/contracts/backend-contracts";
import { SpriteFigure, percentBox, spriteLayers } from "./SpriteFigure";
import { spriteError } from "./useSpriteSlot";

export const FACES = ["happy", "sad", "angry", "surprised"] as const;
const EXPRESSIONS = ["neutral", ...FACES] as const;
const MIN_FACE = 16;

/** 狀態事件的鍵：組（空字串＝共用組）＋表情 */
export const statusKey = (variant: string | null | undefined, expression: string) => `${variant ?? ""}/${expression}`;

export function ExpressionRow({
  sprite,
  set,
  statuses,
  disabled,
  onReroll,
  onUpload,
}: {
  sprite: SpriteView;
  /** 變體鍵＝該變體的獨立組；不給＝共用組 */
  set?: string;
  statuses: Record<string, SpriteSlotStatus>;
  disabled: boolean;
  onReroll: (expression: string) => void;
  onUpload: (file: File, expression: string) => void;
}) {
  return (
    <div className="vn-expr-row">
      {EXPRESSIONS.map((expression) => (
        <ExpressionCell
          key={expression}
          sprite={sprite}
          set={set}
          expression={expression}
          status={statuses[statusKey(set, expression)]}
          disabled={disabled}
          onReroll={() => onReroll(expression)}
          onUpload={(file) => onUpload(file, expression)}
        />
      ))}
    </div>
  );
}

function ExpressionCell({
  sprite,
  set,
  expression,
  status,
  disabled,
  onReroll,
  onUpload,
}: {
  sprite: SpriteView;
  set?: string;
  expression: string;
  status?: SpriteSlotStatus;
  disabled: boolean;
  onReroll: () => void;
  onUpload: (file: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  // 預覽只看臉框外擴的那塊（跟送給模型的裁切同一塊）
  const { box: face, width, face: layer } = spriteLayers(sprite, set, expression);
  const side = Math.round(Math.max(face.w, face.h) * 1.8);
  const left = Math.round(face.x + face.w / 2 - side / 2);
  const top = Math.round(face.y + face.h / 2 - side / 2);
  const neutral = expression === "neutral";
  const state = neutral
    ? "done"
    : status && status.state !== "done"
      ? status.state
      : layer
        ? "done"
        : "missing";
  return (
    <figure className="vn-expr-cell">
      <div className="vn-expr-view">
        <div
          style={{
            position: "absolute",
            width: `${(width / side) * 100}%`,
            left: `${(-left / side) * 100}%`,
            top: `${(-top / side) * 100}%`,
          }}
        >
          <SpriteFigure sprite={sprite} variant={set} expression={expression} alt="" className="vn-expr-figure" />
        </div>
      </div>
      <figcaption>
        {t(`vnExpression_${expression}` as "vnExpression_neutral")}
        {state !== "done" && <span className={`vn-expr-state vn-expr-${state}`}>{t(`vnSpriteState_${state}`)}</span>}
      </figcaption>
      {state === "failed" && status?.reason && <p className="field-warn">{spriteError(status.reason)}</p>}
      {!neutral && (
        <div className="vn-expr-actions">
          <button type="button" className="btn btn-sm" disabled={disabled || state === "running"} onClick={onReroll}>
            {t("vnSpriteRerollFace")}
          </button>
          <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => input.current?.click()}>
            {t("vnSpriteUploadFace")}
          </button>
          <FileInput input={input} onFile={onUpload} />
        </div>
      )}
    </figure>
  );
}

/** 藏起來的選檔欄：PNG／WebP，選完清空（同一檔可再選） */
export function FileInput({ input, onFile }: { input: React.RefObject<HTMLInputElement | null>; onFile: (file: File) => void }) {
  return (
    <input
      ref={input}
      type="file"
      accept="image/png,image/webp"
      hidden
      onChange={(event) => {
        const file = event.currentTarget.files?.[0];
        event.currentTarget.value = "";
        if (file) onFile(file);
      }}
    />
  );
}

/** 臉框編輯：框內拖拉移動、右下角拉動縮放，座標換回定稿像素並夾在圖內 */
export function FaceBoxEditor({ sprite, onApply }: { sprite: SpriteView; onApply: (face: FaceBox) => void }) {
  const saved = sprite.meta.face;
  const [box, setBox] = useState(saved);
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => setBox(saved), [saved.x, saved.y, saved.w, saved.h]);
  const clamp = (next: FaceBox): FaceBox => {
    const w = Math.min(Math.max(next.w, MIN_FACE), sprite.width);
    const h = Math.min(Math.max(next.h, MIN_FACE), sprite.height);
    return {
      w,
      h,
      x: Math.min(Math.max(next.x, 0), sprite.width - w),
      y: Math.min(Math.max(next.y, 0), sprite.height - h),
    };
  };
  const drag = (mode: "move" | "resize") => (event: ReactPointerEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const scale = sprite.width / (frame.current?.getBoundingClientRect().width || sprite.width);
    const start = { x: event.clientX, y: event.clientY, box };
    const target = event.currentTarget;
    target.setPointerCapture?.(event.pointerId);
    const move = (moved: PointerEvent) => {
      const dx = Math.round((moved.clientX - start.x) * scale);
      const dy = Math.round((moved.clientY - start.y) * scale);
      setBox(
        clamp(
          mode === "move"
            ? { ...start.box, x: start.box.x + dx, y: start.box.y + dy }
            : { ...start.box, w: start.box.w + dx, h: start.box.h + dy },
        ),
      );
    };
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  };
  const changed = box.x !== saved.x || box.y !== saved.y || box.w !== saved.w || box.h !== saved.h;
  return (
    <div className="vn-face-editor">
      <div className="vn-face-actions">
        <span className="field-note">{t("vnSpriteFaceHint")}</span>
        {changed && (
          <>
            <button type="button" className="btn btn-sm btn-primary" onClick={() => onApply(box)}>
              {t("vnSpriteFaceApply")}
            </button>
            <button type="button" className="btn btn-sm" onClick={() => setBox(saved)}>
              {t("vnSpriteFaceReset")}
            </button>
          </>
        )}
      </div>
      <div
        ref={frame}
        className="vn-face-frame"
        style={{
          aspectRatio: `${sprite.width} / ${sprite.height}`,
          width: `min(100%, ${Math.round((480 * sprite.width) / sprite.height)}px)`,
        }}
      >
        <img src={sprite.body} alt="" draggable={false} />
        <div
          className="vn-face-box"
          style={percentBox(box, sprite.width, sprite.height)}
          onPointerDown={drag("move")}
        >
          <span className="vn-face-handle" onPointerDown={drag("resize")} />
        </div>
      </div>
    </div>
  );
}
