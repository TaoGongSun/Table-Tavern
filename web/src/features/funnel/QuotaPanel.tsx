import { t } from "../../i18n";
import type { ReleaseInfo } from "./releases";

/** 今日免費次數用完的導流面板：只導向下載桌面版（D11），不提任何付費選項。 */
export function QuotaPanel({ release, onClose }: { release: ReleaseInfo; onClose: () => void }) {
  const link = (href: string, label: string, primary = false) => (
    <a className={primary ? "button primary" : "button"} href={href} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="quota-title" data-testid="quota-panel">
      <div className="panel quota-panel">
        <h2 id="quota-title">{t("quotaTitle")}</h2>
        <p>{t("quotaBody")}</p>
        <div className="quota-actions">
          {release.windows && link(release.windows, t("quotaWindows"), true)}
          {release.mac && link(release.mac, t("quotaMac"), true)}
          {link(release.pageUrl, t("quotaPage"), !release.windows && !release.mac)}
        </div>
        <button type="button" className="ghost" onClick={onClose}>
          {t("quotaClose")}
        </button>
      </div>
    </div>
  );
}
