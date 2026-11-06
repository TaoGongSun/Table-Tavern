import { useState } from "react";
import { IconBook, IconEdit, IconPlus, IconSparkle, IconUsers } from "@desktop/shared/ui/icons";
import { getLang, t, type MsgKey } from "../../i18n";
import { PCLOUD_PAGE } from "../../shared/endpoints/origins";
import { downloadHash, ROOM_GUIDE_HASH, useDownloadTable, type DownloadTable } from "./download-route";
import { FEATURE_COMPARE } from "./feature-compare";
import type { ReleaseInfo } from "./releases";
import { version as DESKTOP_VERSION } from "../../../../package.json";
import { DOWNLOAD_DOORS, SHARED_FEATURES, TOUR_HERO, type DoorShot } from "./tour-features";
import { GUIDE_SHOTS, pickGuideImage, pickTourImage, shotSize, TOUR_IMAGES, type TourImages } from "./tour-images";
import { useOverlayPage } from "./useOverlayPage";

const cell = (value: boolean | MsgKey) => (typeof value === "string" ? t(value) : t(value ? "compareYes" : "compareNo"));

/** 版本那一行：只有 API 明確回沒有（404）才說還沒有正式版；查不到只說暫時取不到 */
const STATUS_TEXT: Record<Exclude<ReleaseInfo["status"], "ok">, MsgKey> = {
  loading: "downloadLoading",
  none: "downloadNoRelease",
  error: "downloadUnavailable",
};

/** 簡中頁與中國供應商模式只給 pCloud：GitHub 在中國連不上 */
const pcloudOnly = (cn: boolean) => cn || getLang() === "zh-CN";
/** pCloud 頁的版本行：不打 GitHub，用建置時的桌面版版本號（換版時 pCloud 檔可能晚一步換上） */
const versionLine = (cn: boolean, release: ReleaseInfo) =>
  pcloudOnly(cn) ? t("downloadVersion", { version: DESKTOP_VERSION }) : release.status === "ok" ? t("downloadVersion", { version: release.version ?? "" }) : t(STATUS_TEXT[release.status]);

/** 下載鈕：沒有對應安裝檔就不出該平台鈕，「所有版本與檔案」永遠在（沒有平台檔時它就是主要按鈕） */
function DownloadButtons({ release, cn, testId }: { release: ReleaseInfo; cn: boolean; testId: string }) {
  const link = (href: string, label: string, primary: boolean) => (
    <a className={primary ? "button primary" : "button"} href={href} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
  return (
    <div className="tour-cta" data-testid={testId}>
      {pcloudOnly(cn) ? (
        link(PCLOUD_PAGE, t("downloadPcloud"), true)
      ) : (
        <>
          {release.windows && link(release.windows, t("downloadWindows"), true)}
          {release.mac && link(release.mac, t("downloadMac"), true)}
          {link(release.pageUrl, t("downloadAllFiles"), !release.windows && !release.mac)}
        </>
      )}
    </div>
  );
}

const DOOR_ICON: Record<DownloadTable, typeof IconPlus> = {
  new: IconPlus,
  oneLine: IconSparkle,
  room: IconUsers,
  vn: IconBook,
};

/** 圖示卡的線條圖：用你的 AI、本機資料夾、換幕、編輯。門上的圖示跟大廳同一組。 */
function SharedIcon({ name }: { name: string }) {
  if (name === "subscription") return <IconSparkle className="tour-icon" />;
  if (name === "editor") return <IconEdit className="tour-icon" />;
  const common = { className: "tour-icon", viewBox: "0 0 48 48", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, "aria-hidden": true };
  if (name === "scenes")
    return (
      <svg {...common}>
        <rect x="4" y="10" width="40" height="28" rx="3" />
        <path d="M17 10v28M31 10v28M4 15h40M4 33h40" />
      </svg>
    );
  return (
    <svg {...common}>
      <path d="M5 14a3 3 0 0 1 3-3h10l4 5h18a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3z" />
      <path d="M5 21h38" />
    </svg>
  );
}

/** 手機上點門才把詳情面板捲進畫面；桌機面板就在門下面，不用跳。 */
function scrollPanelOnPhone() {
  if (window.matchMedia?.("(max-width: 899px)").matches) document.getElementById("tour-panel")?.scrollIntoView({ behavior: "smooth", block: "start" });
}

/**
 * 下載頁：主視覺與第一組下載鈕、四扇門與單一詳情面板、每種桌都有的圖示卡、功能對照表、安裝被擋的繞過步驟與第二組下載鈕。
 * 滿版蓋在畫面上、自己捲動；開著時鎖住底下頁面的捲動，Esc 或「回到遊戲」關掉，焦點還給打開前的元素。
 */
export function DownloadPage({
  release,
  cn = false,
  onClose,
  images = TOUR_IMAGES,
  guideShots = GUIDE_SHOTS,
}: {
  release: ReleaseInfo;
  cn?: boolean;
  onClose: () => void;
  images?: TourImages;
  guideShots?: TourImages;
}) {
  const closeButton = useOverlayPage(onClose);
  const table = useDownloadTable();
  const lang = getLang();
  const [failed, setFailed] = useState<ReadonlySet<string>>(() => new Set());
  const fail = (key: string) => () => setFailed((prev) => new Set(prev).add(`${lang}:${key}`));
  const missing = (key: string) => failed.has(`${lang}:${key}`);

  const shotSrc = (shot: DoorShot) => {
    if (missing(shot.key)) return null;
    return shot.source === "guide" ? pickGuideImage(guideShots, shot.key, lang) : pickTourImage(images, shot.key, lang);
  };

  const figure = (shot: DoorShot, alt: string) => {
    const src = shotSrc(shot);
    if (src === null) return null;
    const size = shotSize(shot.key);
    return (
      <figure key={shot.key} className={shot.phone ? "tour-shot tour-shot-phone" : "tour-shot"}>
        <img src={src} width={size.width} height={size.height} alt={alt} loading="lazy" onError={fail(shot.key)} />
      </figure>
    );
  };

  const door = DOWNLOAD_DOORS.find((item) => item.id === table) ?? DOWNLOAD_DOORS[0];
  const heroSize = shotSize(TOUR_HERO);
  const heroSrc = missing(TOUR_HERO) ? null : pickTourImage(images, TOUR_HERO, lang);
  const hero =
    heroSrc === null ? null : (
      <figure className="tour-shot">
        <img src={heroSrc} width={heroSize.width} height={heroSize.height} alt={t("tourTitle")} onError={fail(TOUR_HERO)} />
      </figure>
    );
  const panelShots = door.shots.map((shot) => figure(shot, t(door.name)));

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
          <DownloadButtons release={release} cn={cn} testId="download-cta" />
          <p className="download-version" data-testid="download-version">
            {versionLine(cn, release)}
          </p>
        </div>
        {hero}
      </section>

      <section className="tour-inner tour-section">
        <nav className="tour-doors" aria-label={t("tourDoorsLabel")}>
          {DOWNLOAD_DOORS.map((item) => {
            const Icon = DOOR_ICON[item.id];
            return (
              <a key={item.id} className="tour-door" href={downloadHash(item.id)} aria-current={item.id === door.id ? "true" : undefined} data-testid={`door-${item.id}`} onClick={scrollPanelOnPhone}>
                <Icon className="tour-door-icon" />
                <b>{t(item.name)}</b>
                <span>{t("tourFit", { fit: t(item.fit) })}</span>
                <small>{t(item.onWeb ? "tourOnWeb" : "tourDesktopOnly")}</small>
              </a>
            );
          })}
        </nav>

        <article id="tour-panel" className="tour-panel" data-testid="tour-panel" data-table={door.id} aria-labelledby="tour-panel-title">
          <h2 id="tour-panel-title">{t(door.name)}</h2>
          <p>{t(door.lead)}</p>
          <ul className="tour-points">
            {door.points.map((key) => (
              <li key={key}>{t(key)}</li>
            ))}
          </ul>
          {panelShots.some((node) => node !== null) && <div className={door.shots.length > 1 ? "tour-shots tour-shots-pair" : "tour-shots"}>{panelShots}</div>}
          <p className="tour-web-line" data-testid="tour-web-line">
            {t(door.onWeb ? "tourWebYes" : "tourWebNo")}
          </p>
          {door.cardUi && (
            <div className="tour-card-ui" data-testid="tour-card-ui">
              <h3>{t(door.cardUi.title)}</h3>
              <p>{t(door.cardUi.body)}</p>
              {figure(door.cardUi.shot, t(door.cardUi.title))}
            </div>
          )}
          {door.more && (
            <p className="tour-more">
              <a className="button" href={ROOM_GUIDE_HASH} data-testid="room-guide-link">
                {t(door.more)}
              </a>
            </p>
          )}
        </article>
      </section>

      <section className="tour-inner tour-section">
        <h2>{t("tourFeaturesTitle")}</h2>
        <ul className="tour-icons">
          {SHARED_FEATURES.map((feature) => (
            <li key={feature.key} className="tour-icon-card" data-testid={`tour-${feature.key}`}>
              <SharedIcon name={feature.key} />
              <div className="tour-feature-text">
                <h3>{t(feature.title)}</h3>
                <p>{t(feature.body)}</p>
              </div>
            </li>
          ))}
        </ul>
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
        <DownloadButtons release={release} cn={cn} testId="download-cta-bottom" />
      </section>
    </div>
  );
}
