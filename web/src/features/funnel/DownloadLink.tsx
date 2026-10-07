import { t } from "../../i18n";
import { DOWNLOAD_HASH } from "./download-route";
import type { ReleaseInfo } from "./releases";

/** 頁首常駐、隨時可點的下載連結（D10 裁決內容）。版本號抓得到才顯示；打開下載頁（平台檔、安裝說明、功能對照）。 */
export function DownloadLink({ release }: { release: ReleaseInfo }) {
  return (
    <a className="download-link" href={DOWNLOAD_HASH} data-testid="download-link">
      {release.version ? t("downloadDesktopVersion", { version: release.version }) : t("downloadDesktop")}
    </a>
  );
}
