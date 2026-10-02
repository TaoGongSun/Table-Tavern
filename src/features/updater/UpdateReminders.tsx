import { useEffect } from "react";
import { t } from "../../i18n";
import { Dialog } from "../../shared/ui/Dialog";
import type { UpdateController } from "./useUpdateController";
import { isOfferSkipped, type StartupReminder } from "./version-center";

interface ReminderProps {
  update: UpdateController;
  preferences: Record<string, unknown>;
  onView: () => void;
}

/** 這一種提醒現在該不該畫：啟動檢查決定了種類，之後被略過就收起來。 */
function shownReminder(update: UpdateController, preferences: Record<string, unknown>, kind: StartupReminder) {
  const reminder = update.reminder;
  if (!reminder || reminder.kind !== kind) return null;
  return isOfferSkipped(reminder.offer, preferences) ? null : reminder.offer;
}

/** 畫出來之後才記「這版提醒過了」。 */
function useMarkShown(update: UpdateController, version: string | null) {
  const { markReminderShown } = update;
  useEffect(() => {
    if (version !== null) markReminderShown(version);
  }, [version, markReminderShown]);
}

/** 功能版：大廳通知區。 */
export function UpdateBanner({ update, preferences, onView }: ReminderProps) {
  const offer = shownReminder(update, preferences, "banner");
  useMarkShown(update, offer?.version ?? null);
  if (!offer) return null;
  return (
    <div className="update-banner" role="status">
      <span>{t("updateBannerText", { version: offer.version })}</span>
      <div className="row">
        <button
          type="button"
          onClick={() => {
            update.dismissReminder();
            onView();
          }}
        >
          {t("updateViewBtn")}
        </button>
        <button
          type="button"
          className="update-banner-close"
          aria-label={t("closeBtn")}
          title={t("closeBtn")}
          onClick={update.dismissReminder}
        >
          ✕
        </button>
      </div>
    </div>
  );
}

/** 含格式轉換的版本：啟動對話框。 */
export function FormatUpdateDialog({ update, preferences, onView }: ReminderProps) {
  const offer = shownReminder(update, preferences, "dialog");
  useMarkShown(update, offer?.version ?? null);
  if (!offer) return null;
  return (
    <Dialog
      title={t("formatDialogTitle", { version: offer.version })}
      onDismiss={update.dismissReminder}
      backdrop
      start={
        <button type="button" className="btn" onClick={update.dismissReminder}>
          {t("updateLaterBtn")}
        </button>
      }
      end={
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => {
            update.dismissReminder();
            onView();
          }}
        >
          {t("updateViewBtn")}
        </button>
      }
    >
      <p>{t("formatDialogBody")}</p>
    </Dialog>
  );
}
