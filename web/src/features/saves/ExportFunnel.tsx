import { t } from "../../i18n";
import type { ReleaseInfo } from "../funnel/releases";

/** 匯出存檔後的「用桌面版繼續」導流（D10 固定節點：匯出存檔）。 */
export function ExportFunnel({ release, onClose }: { release: ReleaseInfo; onClose: () => void }) {
  return (
    <aside className="panel export-funnel" role="status" data-testid="export-funnel">
      <strong>{t("exportFunnelTitle")}</strong>
      <p>{t("exportFunnelBody")}</p>
      <div className="quota-actions">
        <a className="button primary" href={release.pageUrl} target="_blank" rel="noopener noreferrer">
          {release.version ? t("downloadDesktopVersion", { version: release.version }) : t("downloadDesktop")}
        </a>
        <button type="button" className="ghost" onClick={onClose}>
          {t("exportFunnelClose")}
        </button>
      </div>
    </aside>
  );
}
