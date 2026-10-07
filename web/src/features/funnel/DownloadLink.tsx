import { t } from "../../i18n";
import type { ReleaseInfo } from "./releases";

/** 頁首常駐、隨時可點的下載連結（D10 裁決內容）。版本號抓得到才顯示；連到 release 頁讓玩家自己挑平台。 */
export function DownloadLink({ release }: { release: ReleaseInfo }) {
  return (
    <a
      className="download-link"
      href={release.pageUrl}
      target="_blank"
      rel="noopener noreferrer"
      data-testid="download-link"
    >
      {release.version ? t("downloadDesktopVersion", { version: release.version }) : t("downloadDesktop")}
    </a>
  );
}
