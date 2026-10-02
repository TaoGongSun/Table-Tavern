import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import type { VersionCenter } from "./useVersionCenter";
import { RollbackSection, StorageSection, WaitingNotice } from "./VersionStoreSections";
import { cannotReplace, isOfferSkipped, RELEASES_URL } from "./version-center";

interface VersionTabProps {
  center: VersionCenter;
  preferences: Record<string, unknown>;
  onPreference: (key: string, value: unknown) => void;
}

/** 設定頁「版本」分頁：更新、回退、儲存空間。各區的主要動作放在區塊最上方。 */
export function VersionTab({ center, preferences, onPreference }: VersionTabProps) {
  return (
    <div className="settings-form version-tab">
      <UpdateSection center={center} preferences={preferences} onPreference={onPreference} />
      <RollbackSection center={center} />
      <StorageSection center={center} />
    </div>
  );
}

function UpdateSection({ center, preferences, onPreference }: VersionTabProps) {
  const { update, busy, appVersion } = center;
  const { phase, offer } = update;
  const [skipError, setSkipError] = useState("");
  const skipped = offer !== null && isOfferSkipped(offer, preferences);
  const canStart = (phase.kind === "available" || phase.kind === "error") && !busy;
  const replaceFailed = phase.kind === "error" && cannotReplace(phase.message);

  return (
    <section className="version-section" aria-label={t("updateSectionTitle")}>
      <h3>{t("updateSectionTitle")}</h3>
      <p>{t("currentVersionLabel", { version: appVersion ?? "—" })}</p>
      <div className="row">
        {offer && !replaceFailed && (
          <button type="button" disabled={!canStart} onClick={update.requestInstall}>
            {phase.kind === "error" ? t("updateRetryBtn") : t("updateBtn")}
          </button>
        )}
        {replaceFailed && (
          <button type="button" onClick={() => void openUrl(RELEASES_URL)}>
            {t("openDownloadPageBtn")}
          </button>
        )}
        <button
          type="button"
          disabled={busy || update.checking}
          onClick={() => void update.checkNow()}
        >
          {update.checking ? t("checkingUpdates") : t("checkUpdatesBtn")}
        </button>
      </div>

      {offer && (
        <div className="version-offer">
          <p>
            <strong>{t("newVersionLabel", { version: offer.version })}</strong>
            {skipped && <span className="table-badge">{t("skippedBadge")}</span>}
          </p>
          {offer.level === "format" && <p className="version-note">{t("updateFormatNote")}</p>}
          {phase.kind === "waiting" && (
            <WaitingNotice onStop={center.stopResponse} onCancel={update.cancelWait} />
          )}
          {phase.kind === "downloading" && (
            <p role="status">
              {t("updateDownloading")}
              <progress
                value={phase.total ? phase.downloaded : undefined}
                max={phase.total ?? undefined}
              />
            </p>
          )}
          {phase.kind === "confirm" && <p role="status">{t("updateConfirming")}</p>}
          {phase.kind === "rechecking" && <p role="status">{t("checkingUpdates")}</p>}
          {phase.kind === "installing" && <p role="status">{t("updateInstalling")}</p>}
          {phase.kind === "error" && (
            <p role="alert">
              {replaceFailed ? t("cannotReplaceBody") : t("updateFailed", { reason: phase.message })}
            </p>
          )}
          <details>
            <summary>{t("updateDetailsSummary")}</summary>
            <p className="version-notes">{offer.notes?.trim() || t("updateNotesEmpty")}</p>
            <div className="row">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setSkipError("");
                  const saving = skipped ? update.unskip() : update.skip(offer.version);
                  saving.catch((reason: unknown) => setSkipError(String(reason)));
                }}
              >
                {skipped ? t("unskipVersionBtn") : t("skipVersionBtn")}
              </button>
            </div>
            {skipError && <p role="alert">{skipError}</p>}
          </details>
        </div>
      )}
      {!offer && update.checked && !update.checkError && <p>{t("upToDate")}</p>}
      {update.checkError && (
        <p role="alert">{t("checkUpdatesFailed", { reason: update.checkError })}</p>
      )}

      <label className="inline">
        <input
          type="checkbox"
          checked={preferences.update_auto_check !== false}
          onChange={(event) => onPreference("update_auto_check", event.currentTarget.checked)}
        />
        {t("autoCheckLabel")}
      </label>
      <small className="version-note">{t("autoCheckPrivacy")}</small>
    </section>
  );
}
