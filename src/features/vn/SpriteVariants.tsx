// 立繪分頁的變體列：每個變體的身體預覽與重骰／上傳替換／刪除；獨立表情組多一個臉框與表情列。
// VN 卡宣告但還沒生成的變體是空格（生成或上傳，不自動生）。新增變體：填服裝或姿態（同時是劇本 `@變體` 用的名字），生成或上傳。
import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";
import { t } from "../../i18n";
import type { SpriteSlotStatus, SpriteView } from "../../shared/contracts/backend-contracts";
import { ExpressionRow, FaceBoxEditor, FileInput } from "./SpriteExpressionRow";
import { SpriteFigure, spriteLayers } from "./SpriteFigure";
import { fileBytes, type SpriteSlot } from "./useSpriteSlot";

export function SpriteVariants({
  sprite,
  slot,
  statuses,
  paused = false,
  onReroll,
}: {
  sprite: SpriteView;
  slot: SpriteSlot;
  statuses: Record<string, SpriteSlotStatus>;
  paused?: boolean;
  onReroll: (set: string, expressions: readonly string[]) => void;
}) {
  const [label, setLabel] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const ids = { worldId: slot.worldId, characterId: slot.characterId };
  const disabled = slot.busy !== null;
  const generate = (key: string | null, name: string) =>
    slot.run("generate", () => invoke("vn_sprite_variant_generate", { ...ids, key, label: name }));
  const upload = (key: string | null, name: string, file: File) =>
    slot.run("upload", async () =>
      invoke("vn_sprite_variant_upload", { ...ids, key, label: name, data: await fileBytes(file) }),
    );
  return (
    <section className="vn-variants">
      <h3>{t("vnSpriteVariants")}</h3>
      {sprite.meta.variants.map((variant) => (
        <VariantRow
          key={variant.key}
          sprite={sprite}
          variantKey={variant.key}
          label={variant.label}
          own={variant.face !== null}
          disabled={disabled}
          paused={paused}
          statuses={statuses}
          onGenerate={() => void generate(variant.key, variant.label)}
          onUpload={(file) => void upload(variant.key, variant.label, file)}
          onDelete={() => void slot.run("work", () => invoke("vn_sprite_variant_delete", { ...ids, key: variant.key }))}
          onFace={(face) => slot.run("work", () => invoke("vn_sprite_set_face", { ...ids, variant: variant.key, face }))}
          onReroll={(expressions) => onReroll(variant.key, expressions)}
          onUploadFace={(file, expression) => void slot.upload(file, expression, variant.key)}
        />
      ))}
      {sprite.declared_variants
        .filter((declared) => !sprite.meta.variants.some((variant) => variant.key === declared.key))
        .map((declared) => (
          <DeclaredRow
            key={declared.key}
            label={declared.label}
            disabled={disabled}
            onGenerate={() => void generate(declared.key, declared.label)}
            onUpload={(file) => void upload(declared.key, declared.label, file)}
          />
        ))}
      <div className="vn-picker-actions">
        <input
          className="input"
          value={label}
          maxLength={24}
          placeholder={t("vnSpriteVariantPlaceholder")}
          onChange={(event) => setLabel(event.currentTarget.value)}
        />
        <button type="button" className="btn btn-sm" disabled={disabled || !label.trim()} onClick={() => void generate(null, label)}>
          {t("vnSpriteVariantGenerate")}
        </button>
        <button type="button" className="btn btn-sm" disabled={disabled || !label.trim()} onClick={() => input.current?.click()}>
          {t("vnSpriteVariantUpload")}
        </button>
        <FileInput input={input} onFile={(file) => void upload(null, label, file)} />
      </div>
    </section>
  );
}

/** 卡宣告、還沒生成的變體：只有名字與生成／上傳 */
function DeclaredRow({
  label,
  disabled,
  onGenerate,
  onUpload,
}: {
  label: string;
  disabled: boolean;
  onGenerate: () => void;
  onUpload: (file: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className="vn-variant">
      <div className="vn-picker-actions">
        <strong>{label}</strong>
        <span className="field-note">{t("vnSpriteVariantDeclared")}</span>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={onGenerate}>
          {t("vnSpriteVariantGenerate")}
        </button>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => input.current?.click()}>
          {t("vnSpriteVariantUpload")}
        </button>
        <FileInput input={input} onFile={onUpload} />
      </div>
    </div>
  );
}

function VariantRow({
  sprite,
  variantKey,
  label,
  own,
  disabled,
  statuses,
  paused = false,
  onGenerate,
  onUpload,
  onDelete,
  onFace,
  onReroll,
  onUploadFace,
}: {
  sprite: SpriteView;
  variantKey: string;
  label: string;
  own: boolean;
  disabled: boolean;
  statuses: Record<string, SpriteSlotStatus>;
  paused?: boolean;
  onGenerate: () => void;
  onUpload: (file: File) => void;
  onDelete: () => void;
  onFace: Parameters<typeof FaceBoxEditor>[0]["onApply"];
  onReroll: (expressions: readonly string[]) => void;
  onUploadFace: (file: File, expression: string) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const layers = spriteLayers(sprite, variantKey, "neutral");
  // 獨立組的臉框編輯：把變體身體當成定稿交給同一個編輯器
  const asBase = { ...sprite, body: layers.body, width: layers.width, height: layers.height, meta: { ...sprite.meta, face: layers.box } };
  return (
    <div className="vn-variant">
      <div className="vn-picker-actions">
        <strong>{label}</strong>
        <span className="field-note">{t(own ? "vnSpriteVariantOwn" : "vnSpriteVariantShared")}</span>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={onGenerate}>
          {t("vnSpriteRerollFace")}
        </button>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => input.current?.click()}>
          {t("vnSpriteUploadFace")}
        </button>
        <FileInput input={input} onFile={onUpload} />
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={onDelete}>
          {t("vnSpriteVariantDelete")}
        </button>
      </div>
      {own ? (
        <>
          <FaceBoxEditor sprite={asBase} onApply={onFace} />
          <ExpressionRow
            sprite={sprite}
            set={variantKey}
            paused={paused}
            statuses={statuses}
            disabled={disabled}
            onReroll={(expression) => onReroll([expression])}
            onUpload={onUploadFace}
          />
        </>
      ) : (
        <SpriteFigure sprite={sprite} variant={variantKey} expression="neutral" alt={label} className="vn-expr-figure vn-variant-figure" />
      )}
    </div>
  );
}
