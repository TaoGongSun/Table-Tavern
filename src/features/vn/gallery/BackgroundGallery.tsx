// 美術一覽的背景分頁：地點 × 時段排格（plans/vn-setup-wizard.md §4）。有自己圖檔的格顯示圖與「重生成」；
// 沒有的格顯示基底＋該時段色調遮罩的預覽與「生成專屬圖」；排隊中可取消、失敗可重試；點圖開燈箱。
// 圖走舞台那份 useBackgrounds，生成、上傳完成的事件讓舞台與這裡一起重讀。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { t } from "../../../i18n";
import type { BackgroundPlace, BackgroundStatus } from "../../../shared/contracts/backend-contracts";
import { explainAiError, redactAiErrorDetail } from "../../../shared/ui/ai-error";
import { backendText } from "../../../shared/ui/backend-text";
import { tintOf } from "../StageBackdrop";
import { backdropKey, type BackdropImage, type useBackgrounds } from "../useBackgrounds";
import { fileBytes } from "../useSpriteSlot";
import type { Viewing } from "./ArtViewer";

const FIXED_PERIODS = ["day", "morning", "dusk", "night"];

/** 固定四時段用語系文字；卡自訂時段沒有標籤，顯示卡上的鍵 */
export const periodLabel = (period: string) =>
  FIXED_PERIODS.includes(period) ? t(`vnPeriod_${period}` as "vnPeriod_day") : period;

/** 失敗原因給玩家看的那句：命中 AI 錯誤分流用人話，否則翻譯代碼後的原文（遮掉帳號識別碼） */
function failureText(reason: string | null) {
  const key = explainAiError(reason ?? "");
  return key ? t(key) : redactAiErrorDetail(backendText(reason));
}

type Command = "vn_background_regenerate" | "vn_background_generate_period" | "vn_background_cancel";

export function BackgroundGallery({
  worldId,
  backgrounds,
  readOnly,
  onView,
}: {
  worldId: string;
  backgrounds: ReturnType<typeof useBackgrounds>;
  readOnly: boolean;
  onView: (viewing: Viewing) => void;
}) {
  const { images, status, load } = backgrounds;
  const [places, setPlaces] = useState<BackgroundPlace[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 佇列有任何變化（排入、完成、失敗、取消）就重列：專屬圖多了、基底剛生好、新地點排進來
  useEffect(() => {
    let live = true;
    invoke<BackgroundPlace[]>("vn_backgrounds_list", { worldId })
      .then((list) => live && setPlaces(list))
      .catch((reason) => live && setError(backendText(reason)));
    return () => {
      live = false;
    };
  }, [worldId, status]);

  // 只讀有自己圖檔的時段（基底只讀一次，其他格重用它疊遮罩）
  useEffect(() => {
    for (const place of places ?? [])
      for (const period of place.dedicated) load({ location: place.location ?? "", slug: place.slug, period });
  }, [places, load]);

  const act = (command: string, args: Record<string, unknown>) => {
    setError(null);
    invoke(command, { worldId, ...args }).catch((reason) => setError(backendText(reason)));
  };

  if (places === null) return error ? <p className="field-warn">{error}</p> : null;
  if (places.length === 0) return <p className="vn-chapter-note">{t("vnGalleryNoBackgrounds")}</p>;
  return (
    <div className="vn-gallery-places">
      {error && (
        <p className="field-warn" role="alert">
          {error}
        </p>
      )}
      {places.map((place) => (
        <section key={place.slug} className="vn-gallery-place">
          <h3>{place.location ?? t("vnGalleryUnnamed")}</h3>
          <div className="vn-gallery-periods">
            {place.periods.map((period, index) => (
              <BackgroundCell
                key={period}
                place={place}
                period={period}
                first={index === 0}
                image={images[backdropKey({ slug: place.slug, period: place.dedicated.includes(period) ? period : (place.base ?? "") })]}
                job={status[backdropKey({ slug: place.slug, period })]}
                editable={!readOnly && place.location !== null}
                onView={onView}
                onCommand={(command) => act(command, { slug: place.slug, period })}
                onUpload={(file) =>
                  void fileBytes(file).then((data) => act("vn_background_upload", { slug: place.slug, period, data }))
                }
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function BackgroundCell({
  place,
  period,
  first,
  image,
  job,
  editable,
  onView,
  onCommand,
  onUpload,
}: {
  place: BackgroundPlace;
  period: string;
  first: boolean;
  image: BackdropImage | undefined;
  job: BackgroundStatus | undefined;
  editable: boolean;
  onView: (viewing: Viewing) => void;
  onCommand: (command: Command) => void;
  onUpload: (file: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const own = place.dedicated.includes(period);
  const tint = own || !place.base ? null : tintOf(period, { src: null, period: place.base, rev: 0 });
  const busy = job?.state === "queued" || job?.state === "running";
  const failed = job?.state === "failed";
  const label = `${place.location ?? t("vnGalleryUnnamed")}・${periodLabel(period)}`;
  const caption = own
    ? period === place.base
      ? t("vnGalleryBase")
      : null
    : place.base
      ? t(tint ? "vnGalleryTintPreview" : "vnGalleryBaseShared")
      : null;
  // 有圖：重生成那張；沒專屬圖：生成專屬圖；整個地點還沒基底：第一格生成（成為基底）；失敗的同一個動作改叫「重試」
  const action = own
    ? { label: t("vnGalleryRegenerate"), command: "vn_background_regenerate" as const }
    : place.base
      ? { label: t("vnGalleryGeneratePeriod"), command: "vn_background_generate_period" as const }
      : first || failed
        ? { label: t("vnGalleryGenerate"), command: "vn_background_regenerate" as const }
        : null;
  const generate = action && failed ? { ...action, label: t("vnGalleryRetry") } : action;
  return (
    <figure className="vn-gallery-cell">
      <div className="vn-gallery-frame">
        {image?.src ? (
          <button
            type="button"
            className="vn-gallery-open"
            aria-label={label}
            onClick={() => onView({ kind: "background", src: image.src!, tint, label })}
          >
            <img src={image.src} alt="" />
            {tint && <div className="vn-gallery-tint" style={{ backgroundColor: tint.color, opacity: tint.opacity }} aria-hidden />}
          </button>
        ) : (
          <span className="vn-gallery-empty">{t("vnGalleryNoImage")}</span>
        )}
        {busy && (
          <span className="vn-gallery-busy">
            <span className="vn-pending">{t(job?.state === "queued" ? "vnGalleryQueued" : "vnGalleryRunning")}</span>
            {editable && job?.state === "queued" && (
              <button type="button" className="btn btn-sm" onClick={() => onCommand("vn_background_cancel")}>
                {t("vnGalleryCancel")}
              </button>
            )}
          </span>
        )}
      </div>
      <figcaption>
        <strong>{periodLabel(period)}</strong>
        {caption && <span className="field-note">{caption}</span>}
      </figcaption>
      {/* 錯誤走 AI 錯誤分流翻成人話（認不得的才顯示翻譯後的原文、收成兩行）；遮過帳號的完整原文放 title */}
      {failed && (
        <p className="field-warn vn-gallery-fail" title={failureText(job.reason)}>
          {t("vnGalleryFailed", { reason: failureText(job.reason) })}
        </p>
      )}
      {editable && (
        <div className="vn-gallery-actions">
          {generate && (
            <button
              type="button"
              className="btn btn-sm"
              title={t("vnGalleryQuotaHint")}
              disabled={busy}
              onClick={() => onCommand(generate.command)}
            >
              {generate.label}
            </button>
          )}
          <button type="button" className="btn btn-sm" disabled={busy} onClick={() => input.current?.click()}>
            {t(own ? "vnGalleryUpload" : "vnGalleryUploadOwn")}
          </button>
          <input
            ref={input}
            type="file"
            accept="image/png,image/jpeg,image/webp"
            hidden
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) onUpload(file);
            }}
          />
        </div>
      )}
    </figure>
  );
}
