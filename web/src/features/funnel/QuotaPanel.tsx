import { t } from "../../i18n";
import { DOWNLOAD_HASH } from "./download-route";
import type { ReleaseInfo } from "./releases";

/** 今日免費次數用完的導流面板：只導向下載桌面版（D11），不提任何付費選項；安裝說明與功能對照在下載頁。 */
export function QuotaPanel({ release, onClose }: { release: ReleaseInfo; onClose: () => void }) {
  const file = (href: string, label: string) => (
    <a className="button primary" href={href} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="quota-title" data-testid="quota-panel">
      <div className="panel quota-panel">
        <h2 id="quota-title">{t("quotaTitle")}</h2>
        <p>{t("quotaBody")}</p>
        <div className="quota-actions">
          {release.windows && file(release.windows, t("quotaWindows"))}
          {release.mac && file(release.mac, t("quotaMac"))}
          <a className={release.windows || release.mac ? "button" : "button primary"} href={DOWNLOAD_HASH}>
            {t("quotaPage")}
          </a>
        </div>
        <button type="button" className="ghost" onClick={onClose}>
          {t("quotaClose")}
        </button>
      </div>
    </div>
  );
}
