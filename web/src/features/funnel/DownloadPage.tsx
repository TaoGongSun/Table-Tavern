import { t, type MsgKey } from "../../i18n";
import { FEATURE_COMPARE } from "./feature-compare";
import type { ReleaseInfo } from "./releases";

const cell = (value: boolean | MsgKey) => (typeof value === "string" ? t(value) : t(value ? "compareYes" : "compareNo"));

/** 版本那一行：只有 API 明確回沒有（404）才說還沒有正式版；查不到只說暫時取不到 */
const STATUS_TEXT: Record<Exclude<ReleaseInfo["status"], "ok">, MsgKey> = {
  loading: "downloadLoading",
  none: "downloadNoRelease",
  error: "downloadUnavailable",
};

/**
 * 下載頁：版本、各平台安裝檔、未簽章／未公證時的繞過步驟、功能對照表。沒有正式版、還在查或查不到都只給發佈頁連結。
 * 蓋在畫面上，關掉就回到原本那桌。
 */
export function DownloadPage({ release, onClose }: { release: ReleaseInfo; onClose: () => void }) {
  const file = (href: string, label: string) => (
    <a className="button primary" href={href} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
  return (
    <div className="overlay download-overlay" role="dialog" aria-modal="true" aria-labelledby="download-title" data-testid="download-page">
      <div className="panel download-page">
        <h2 id="download-title">{t("downloadTitle")}</h2>
        <p>{t("downloadLead")}</p>
        <p className="download-version" data-testid="download-version">
          {release.status === "ok" ? t("downloadVersion", { version: release.version ?? "" }) : t(STATUS_TEXT[release.status])}
        </p>
        <div className="quota-actions">
          {release.windows && file(release.windows, t("downloadWindows"))}
          {release.mac && file(release.mac, t("downloadMac"))}
          <a className="button" href={release.pageUrl} target="_blank" rel="noopener noreferrer">
            {t("downloadAllFiles")}
          </a>
        </div>

        <h3>{t("downloadInstallTitle")}</h3>
        <ul className="download-steps">
          <li>{t("downloadInstallWindows")}</li>
          <li>{t("downloadInstallMac")}</li>
        </ul>
        <p className="chat-note">{t("downloadInstallWhy")}</p>

        <h3>{t("downloadCompareTitle")}</h3>
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

        <button type="button" className="ghost" onClick={onClose}>
          {t("downloadClose")}
        </button>
      </div>
    </div>
  );
}
