import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import type { RepairNotice } from "./open-world";

interface FormatBannerProps {
  /** null＝版本不明，橫幅省略 X */
  version: string | null;
  backupAvailable: boolean;
  skipped: number;
  onUseBackup: () => void;
}

/** 唯讀桌頂端：說明、略過行數、以及改用轉換前備份。 */
export function FormatBanner({ version, backupAvailable, skipped, onUseBackup }: FormatBannerProps) {
  return (
    <div className="format-banner" role="status">
      <p>
        {version === null
          ? t("readOnlyBannerUnknown")
          : t("readOnlyBanner", { version })}
      </p>
      {skipped > 0 && <p>{t("readOnlySkipped", { count: skipped })}</p>}
      {backupAvailable && (
        <div className="row">
          <button type="button" onClick={onUseBackup}>
            {t("useBackupBtn")}
          </button>
        </div>
      )}
    </div>
  );
}

interface FormatRepairProps {
  notice: RepairNotice;
  onOpenFolder: () => void;
}

/** 說明文字依後端給的原因代碼翻譯；io 附上系統錯誤原文（本身也可能是代碼）。 */
export function repairText({ reason, error }: RepairNotice): string {
  return reason === "io"
    ? t("needsRepair_io", { error: backendText(error ?? "") })
    : t(`needsRepair_${reason}`);
}

/** 對不上恢復表、或主資料夾不見：說明狀況，並打開該桌的資料夾。 */
export function FormatRepair({ notice, onOpenFolder }: FormatRepairProps) {
  return (
    <div className="format-repair">
      <h2>{t("needsRepairTitle")}</h2>
      <p>{repairText(notice)}</p>
      <div className="row">
        <button type="button" onClick={onOpenFolder}>
          {t("openFolderBtn")}
        </button>
      </div>
    </div>
  );
}
