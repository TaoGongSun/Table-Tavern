import { t } from "../../i18n";
import { DOWNLOAD_HASH } from "./download-route";

/** 碰到桌面版才有的功能時的下載提示（D10 固定節點），接在原本的說明後面。 */
export function DesktopOnly() {
  return (
    <span className="desktop-only" data-testid="desktop-only">
      {t("desktopOnly")}
      <a href={DOWNLOAD_HASH}>{t("desktopOnlyLink")}</a>
    </span>
  );
}
