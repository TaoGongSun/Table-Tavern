// 版本分頁、側欄小點、啟動提醒共用的判斷。只放純函式，畫面與 controller 都從這裡取。
import { t } from "../../i18n";
import type { UpdateLevel, UpdateOffer, UpdatePhase } from "./useUpdateController";
import type { RollbackPhase, RollbackPreview, VersionRow } from "./useVersionStoreController";

export type Preferences = Record<string, unknown> | undefined;

/** 略過與否一律看目前設定：略過、取消略過、安裝失敗後重讀設定都會跟著變。 */
export function isOfferSkipped(offer: UpdateOffer, preferences: Preferences): boolean {
  return preferences?.update_skipped_version === offer.version;
}

/** 知道有比目前新、且沒被略過的版本，小點就一直亮。 */
export function showUpdateDot(offer: UpdateOffer | null, preferences: Preferences): boolean {
  return offer !== null && !isOfferSkipped(offer, preferences);
}

export type StartupReminder = "banner" | "dialog";

/** 只用在啟動那次自動檢查。小修只亮點；被略過或這版已提醒過都不出。 */
export function startupReminder(
  offer: UpdateOffer | null,
  preferences: Preferences,
): StartupReminder | null {
  if (!offer || isOfferSkipped(offer, preferences)) return null;
  if (preferences?.update_reminded_version === offer.version) return null;
  const byLevel: Record<UpdateLevel, StartupReminder | null> = {
    format: "dialog",
    feature: "banner",
    patch: null,
  };
  return byLevel[offer.level];
}

/** 更新流程進行中：等待、下載、確認、版本被換掉後的重新檢查、安裝。 */
export function updateFlowBusy(phase: UpdatePhase): boolean {
  return (
    phase.kind === "waiting" ||
    phase.kind === "downloading" ||
    phase.kind === "confirm" ||
    phase.kind === "rechecking" ||
    phase.kind === "installing"
  );
}

/** 回退流程進行中：預覽、等待、安裝。 */
export function rollbackFlowBusy(phase: RollbackPhase): boolean {
  return phase.kind === "preview" || phase.kind === "waiting" || phase.kind === "installing";
}

export type VersionRowActions = { current: boolean; rollback: boolean; delete: boolean };

/** 目前那列不給動作；其餘都能刪，eligible 的另給「回到這版」。 */
export function versionRowActions(row: VersionRow): VersionRowActions {
  if (row.current) return { current: true, rollback: false, delete: false };
  return { current: false, rollback: row.eligible, delete: true };
}

/** 一鍵「回到上一版」只認清單上同時是 previous 與 eligible 的那一列。 */
export function previousRow(rows: VersionRow[] | undefined): VersionRow | null {
  return rows?.find((row) => row.previous && row.eligible) ?? null;
}

export type RollbackCheck =
  | { kind: "invalid"; message: string }
  | { kind: "preview"; preview: RollbackPreview };

export type RollbackNotice =
  | { kind: "invalid"; message: string }
  | { kind: "scanFailed" }
  | { kind: "none" }
  | { kind: "lists"; will: string[]; maybe: string[] };

/** 目標不合格只能關閉；再先判 `scan_failed`，最後才看兩組是不是都空。 */
export function rollbackNotice(check: RollbackCheck): RollbackNotice {
  if (check.kind === "invalid") return { kind: "invalid", message: check.message };
  const { preview } = check;
  if (preview.scan_failed) return { kind: "scanFailed" };
  if (preview.will_be_readonly.length === 0 && preview.maybe_readonly.length === 0) {
    return { kind: "none" };
  }
  return {
    kind: "lists",
    will: preview.will_be_readonly.map((world) => world.name),
    maybe: preview.maybe_readonly.map((world) => world.name),
  };
}

/** 回退確認窗的內文。可繼續的那幾種另寫「目前這版會被標為略過」。 */
export function rollbackDialogText(notice: RollbackNotice, currentVersion: string): string {
  const skipNote = t("rollbackSkipNote", { version: currentVersion });
  switch (notice.kind) {
    case "invalid":
      return t("rollbackInvalid", { reason: notice.message });
    case "scanFailed":
      return `${t("rollbackScanFailed")}\n\n${skipNote}`;
    case "none":
      return `${t("rollbackNoReadonly")}\n\n${skipNote}`;
    case "lists": {
      const groups = [
        notice.will.length > 0 ? `${t("rollbackWillReadonly")}\n${notice.will.join("\n")}` : "",
        notice.maybe.length > 0 ? `${t("rollbackMaybeReadonly")}\n${notice.maybe.join("\n")}` : "",
      ].filter((group) => group !== "");
      return [...groups, skipNote].join("\n\n");
    }
  }
}

/** 後端槽裡的版本已被後來的檢查換掉（slot.rs 的 UPDATE_CHANGED）。 */
export function updateChanged(message: string): boolean {
  return message.includes("要更新的版本已經換了");
}

/** Mac 回「無法自動替換」時改給下載頁。後端訊息固定是這句繁中。 */
export function cannotReplace(message: string): boolean {
  return message.includes("無法自動替換");
}

export const RELEASES_URL = "https://github.com/TaoGongSun/Table-Tavern/releases";

/** 空的寫 0 KB；有東西但不到 1 KB 才進位成 1 KB。 */
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
