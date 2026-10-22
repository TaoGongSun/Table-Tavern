import { useEffect, useRef, useState } from "react";
import { getLang, t, type MsgKey } from "../../i18n";
import { FEATURE_COMPARE } from "./feature-compare";
import type { ReleaseInfo } from "./releases";
import { TOUR_FEATURES, TOUR_HERO, type TourFeature } from "./tour-features";
import { pickTourImage, TOUR_IMAGES, type TourImages } from "./tour-images";

const cell = (value: boolean | MsgKey) => (typeof value === "string" ? t(value) : t(value ? "compareYes" : "compareNo"));

/** 版本那一行：只有 API 明確回沒有（404）才說還沒有正式版；查不到只說暫時取不到 */
const STATUS_TEXT: Record<Exclude<ReleaseInfo["status"], "ok">, MsgKey> = {
  loading: "downloadLoading",
  none: "downloadNoRelease",
  error: "downloadUnavailable",
};

/** 截圖的固定比例（通道視窗 800×600 拍出、裁掉底部灰帶後的 2 倍像素） */
const SHOT_WIDTH = 1600;
const SHOT_HEIGHT = 1136;

/** 下載鈕：沒有對應安裝檔就不出該平台鈕，「所有版本與檔案」永遠在（沒有平台檔時它就是主要按鈕） */
function DownloadButtons({ release, testId }: { release: ReleaseInfo; testId: string }) {
  const file = (href: string, label: string) => (
    <a className="button primary" href={href} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
  return (
    <div className="tour-cta" data-testid={testId}>
      {release.windows && file(release.windows, t("downloadWindows"))}
      {release.mac && file(release.mac, t("downloadMac"))}
      <a className={release.windows || release.mac ? "button" : "button primary"} href={release.pageUrl} target="_blank" rel="noopener noreferrer">
        {t("downloadAllFiles")}
      </a>
    </div>
  );
}

/** 圖示卡的線條圖：換幕＝三格膠捲、本機資料＝資料夾、截圖缺的卡＝視窗 */
function TourIcon({ name }: { name: string }) {
  const common = { className: "tour-icon", viewBox: "0 0 48 48", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true };
  if (name === "scenes")
    return (
      <svg {...common}>
        <rect x="4" y="10" width="40" height="28" rx="3" />
        <path d="M17 10v28M31 10v28M4 15h40M4 33h40" />
      </svg>
    );
  if (name === "local")
    return (
      <svg {...common}>
        <path d="M5 14a3 3 0 0 1 3-3h10l4 5h18a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3z" />
        <path d="M5 21h38" />
      </svg>
    );
  return (
    <svg {...common}>
      <rect x="5" y="9" width="38" height="30" rx="3" />
      <path d="M5 16h38M10 12.5h1M14 12.5h1" />
    </svg>
  );
}

/**
 * 下載頁（桌面版導覽）：頁首、主視覺與第一組下載鈕、特色卡、功能對照表、安裝被擋的繞過步驟與第二組下載鈕。
 * 滿版蓋在畫面上、自己捲動；開著時鎖住底下頁面的捲動，Esc 或「回到遊戲」關掉，焦點還給打開前的元素。
 */
export function DownloadPage({ release, onClose, images = TOUR_IMAGES }: { release: ReleaseInfo; onClose: () => void; images?: TourImages }) {
  const closeButton = useRef<HTMLButtonElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const [failed, setFailed] = useState<ReadonlySet<string>>(() => new Set());

  useEffect(() => {
    const opener = document.activeElement;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    closeButton.current?.focus({ preventScroll: true });
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeRef.current();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = overflow;
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true });
    };
  }, []);

  const lang = getLang();
  const imageOf = (key: string) => (failed.has(key) ? null : pickTourImage(images, key, lang));
  const fail = (key: string) => () => setFailed((prev) => new Set(prev).add(key));
  const shot = (key: string, alt: string, lazy: boolean) => {
    const src = imageOf(key);
    return src === null ? null : (
      <figure className="tour-shot">
        <img src={src} width={SHOT_WIDTH} height={SHOT_HEIGHT} alt={alt} loading={lazy ? "lazy" : undefined} onError={fail(key)} />
      </figure>
    );
  };

  const shown = TOUR_FEATURES.filter((feature) => feature.form === "shot" && imageOf(feature.key) !== null);
  const iconCards = TOUR_FEATURES.filter((feature) => !shown.includes(feature));
  const hero = shot(TOUR_HERO, t("tourTitle"), false);
  const text = (feature: TourFeature) => (
    <div className="tour-feature-text">
      <h3>{t(feature.title)}</h3>
      <p>{t(feature.body)}</p>
    </div>
  );

  return (
    <div className="overlay tour-overlay" role="dialog" aria-modal="true" aria-labelledby="tour-title" data-testid="download-page">
      <header className="tour-header">
        <div className="tour-inner tour-header-row">
          <span className="tour-brand">{t("tourBrand")}</span>
          <button type="button" className="ghost" ref={closeButton} onClick={onClose}>
            {t("downloadClose")}
          </button>
        </div>
      </header>

      <section className={hero ? "tour-inner tour-hero" : "tour-inner tour-hero tour-hero-text-only"}>
        <div className="tour-hero-text">
          <h1 id="tour-title">{t("tourTitle")}</h1>
          <p className="tour-lead">{t("tourLead")}</p>
          <DownloadButtons release={release} testId="download-cta" />
          <p className="download-version" data-testid="download-version">
            {release.status === "ok" ? t("downloadVersion", { version: release.version ?? "" }) : t(STATUS_TEXT[release.status])}
          </p>
        </div>
        {hero}
      </section>

      <section className="tour-inner tour-section">
        <h2>{t("tourFeaturesTitle")}</h2>
        {shown.map((feature) => (
          <article key={feature.key} className="tour-feature" data-testid={`tour-${feature.key}`}>
            {shot(feature.key, t(feature.title), true)}
            {text(feature)}
          </article>
        ))}
        {iconCards.length > 0 && (
          <ul className="tour-icons">
            {iconCards.map((feature) => (
              <li key={feature.key} className="tour-icon-card" data-testid={`tour-${feature.key}`}>
                <TourIcon name={feature.key} />
                {text(feature)}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="tour-inner tour-section">
        <h2>{t("downloadCompareTitle")}</h2>
        <div className="compare-scroll">
          <table className="compare-table" data-testid="feature-compare">
            <thead>
              <tr>
                <th scope="col">{t("downloadCompareFeature")}</th>
                <th scope="col">{t("downloadCompareWeb")}</th>
                <th scope="col">{t("downloadCompareDesktop")}</th>
              </tr>
            </thead>
            <tbody>
              {FEATURE_COMPARE.map((row) => (
                <tr key={row.feature}>
                  <th scope="row">{t(row.feature)}</th>
                  <td>{cell(row.web)}</td>
                  <td>{cell(row.desktop)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="tour-inner tour-section tour-install">
        <h2>{t("downloadInstallTitle")}</h2>
        <ul className="download-steps">
          <li>{t("downloadInstallWindows")}</li>
          <li>{t("downloadInstallMac")}</li>
        </ul>
        <p className="tour-install-note">{t("downloadInstallWhy")}</p>
        <DownloadButtons release={release} testId="download-cta-bottom" />
      </section>
    </div>
  );
}
