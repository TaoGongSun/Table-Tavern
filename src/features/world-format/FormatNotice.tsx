import { t } from "../../i18n";

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
  /** 後端給的狀況說明（固定繁中），外框標題才走 i18n */
  message: string;
  onOpenFolder: () => void;
}

/** 對不上恢復表、或主資料夾不見：說明狀況，並打開該桌的資料夾。 */
export function FormatRepair({ message, onOpenFolder }: FormatRepairProps) {
  return (
    <div className="format-repair">
      <h2>{t("needsRepairTitle")}</h2>
      <p>{message}</p>
      <div className="row">
        <button type="button" onClick={onOpenFolder}>
          {t("openFolderBtn")}
        </button>
      </div>
    </div>
  );
}
