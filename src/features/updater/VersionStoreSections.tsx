import { useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import type { VersionCenter } from "./useVersionCenter";
import type { BackupRow, VersionRow } from "./useVersionStoreController";
import { formatBytes, previousRow, versionRowActions } from "./version-center";

/** AI 回應中按了更新或回退：等它結束，或現在停下、或取消等待。 */
export function WaitingNotice({ onStop, onCancel }: { onStop: () => void; onCancel: () => void }) {
  return (
    <div className="version-waiting" role="status">
      <p>{t("waitForResponse")}</p>
      <div className="row">
        <button type="button" className="btn" onClick={onStop}>
          {t("stopResponseBtn")}
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          {t("cancelWaitBtn")}
        </button>
      </div>
    </div>
  );
}

export function RollbackSection({ center }: { center: VersionCenter }) {
  const { store, busy } = center;
  const { phase } = store;
  const [deleteError, setDeleteError] = useState("");
  const previous = previousRow(store.versions?.versions);

  async function deleteVersion(row: VersionRow) {
    const body = row.previous
      ? `${t("deleteVersionBody")}\n${t("deleteVersionPreviousWarn")}`
      : t("deleteVersionBody");
    const accepted = await confirm(body, {
      title: t("deleteVersionTitle", { version: row.version }),
      kind: "warning",
      okLabel: t("dialogDelete"),
      cancelLabel: t("dialogCancel"),
    });
    if (!accepted) return;
    setDeleteError("");
    try {
      await store.deleteVersion(row.version);
    } catch (reason) {
      setDeleteError(t("deleteFailed", { reason: String(reason) }));
    }
  }

  return (
    <section className="version-section" aria-label={t("rollbackSectionTitle")}>
      <h3>{t("rollbackSectionTitle")}</h3>
      {previous && (
        <div className="row">
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => void store.startRollbackPrevious()}
          >
            {t("rollbackPreviousBtn", { version: previous.version })}
          </button>
        </div>
      )}
      {phase.kind === "preview" && <p role="status">{t("rollbackPreviewing")}</p>}
      {phase.kind === "waiting" && (
        <WaitingNotice onStop={center.stopResponse} onCancel={store.cancelWait} />
      )}
      {phase.kind === "installing" && <p role="status">{t("rollbackInstalling")}</p>}
      {phase.kind === "error" && (
        <p role="alert">{t("rollbackFailed", { reason: phase.message })}</p>
      )}
      {store.loadError && (
        <div className="row">
          <p role="alert">{t("versionsLoadFailed", { reason: store.loadError })}</p>
          <button type="button" className="btn" onClick={() => void store.refresh()}>
            {t("refreshListBtn")}
          </button>
        </div>
      )}
      {deleteError && <p role="alert">{deleteError}</p>}
      {store.versions && (
        <details>
          <summary>{t("versionListSummary")}</summary>
          {store.versions.versions.length === 0 ? (
            <p>{t("versionListEmpty")}</p>
          ) : (
            <ul className="version-list">
              {store.versions.versions.map((row) => (
                <li key={row.version} className="version-row">
                  <span className="version-row-name">v{row.version}</span>
                  <span>{formatBytes(row.size)}</span>
                  <span>{row.usable ? t("versionUsable") : t("versionUnusable")}</span>
                  <VersionRowButtons
                    row={row}
                    disabled={busy}
                    onRollback={() => void store.startRollback(row.version)}
                    onDelete={() => void deleteVersion(row)}
                  />
                </li>
              ))}
            </ul>
          )}
        </details>
      )}
    </section>
  );
}

function VersionRowButtons({
  row,
  disabled,
  onRollback,
  onDelete,
}: {
  row: VersionRow;
  disabled: boolean;
  onRollback: () => void;
  onDelete: () => void;
}) {
  const actions = versionRowActions(row);
  if (actions.current) return <span className="table-badge">{t("versionCurrentBadge")}</span>;
  return (
    <>
      {actions.rollback && (
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={onRollback}>
          {t("rollbackToBtn")}
        </button>
      )}
      {actions.delete && (
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={onDelete}>
          {t("versionDeleteBtn")}
        </button>
      )}
    </>
  );
}

export function StorageSection({ center }: { center: VersionCenter }) {
  const { store, busy } = center;
  const [error, setError] = useState("");

  async function deleteBackup(row: BackupRow) {
    const accepted = await confirm(
      row.kind === "pre" ? t("deleteBackupPreBody") : t("deleteBackupNewerBody"),
      {
        title: t("deleteBackupTitle"),
        kind: "warning",
        okLabel: t("dialogDelete"),
        cancelLabel: t("dialogCancel"),
      },
    );
    if (!accepted) return;
    setError("");
    try {
      await store.deleteBackup(row.world_id, row.kind);
    } catch (reason) {
      setError(t("deleteFailed", { reason: String(reason) }));
    }
  }

  async function openFolder(directory: string) {
    setError("");
    try {
      await revealItemInDir(directory);
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <section className="version-section" aria-label={t("storageSectionTitle")}>
      <h3>{t("storageSectionTitle")}</h3>
      {store.versions && (
        <p>{t("versionStoreTotal", { size: formatBytes(store.versions.total_bytes) })}</p>
      )}
      {store.backups && (
        <>
          <p>{t("backupStoreTotal", { size: formatBytes(store.backups.total_bytes) })}</p>
          {store.backups.backups.length === 0 ? (
            <p>{t("backupListEmpty")}</p>
          ) : (
            <ul className="version-list">
              {store.backups.backups.map((row) => (
                <li key={`${row.world_id}:${row.kind}`} className="version-row">
                  <span className="version-row-name">{row.name}</span>
                  <span>{row.kind === "pre" ? t("backupKindPre") : t("backupKindNewer")}</span>
                  <span>{formatBytes(row.size)}</span>
                  {row.deletable ? (
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busy}
                      onClick={() => void deleteBackup(row)}
                    >
                      {t("versionDeleteBtn")}
                    </button>
                  ) : (
                    <>
                      <span className="table-badge">{t("needsRepairBadge")}</span>
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() => void openFolder(row.directory)}
                      >
                        {t("openFolderBtn")}
                      </button>
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
