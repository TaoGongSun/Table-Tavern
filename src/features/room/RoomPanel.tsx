// 多人房間模式面板（multiplayer-room 包 2）：桌面版當 GM 執行端。執行端迴圈全在 Rust（視窗縮小或鎖定時
// webview 可能被節流），這裡只讀狀態、下開關；劇情要看就用手機。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { t } from "../../i18n";
import { Dialog } from "../../shared/ui/Dialog";

export interface RoomStatus {
  /** 執行端迴圈在跑（房間模式開著） */
  active: boolean;
  /** 房間屬於哪一桌；沒有房間時 null */
  worldId: string | null;
  server: string;
  roomCode: string | null;
  inviteUrl: string | null;
  pairCode: string | null;
  /** 出額度者用的手機連結：邀請連結多帶配對碼 */
  payerLink: string | null;
  online: boolean;
  keepAwake: boolean | null;
  modelLabel: string | null;
  lastError: string | null;
}

const POLL_MS = 2000;

function CopyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="room-row">
      <span className="room-label">{label}</span>
      <input className="room-value" readOnly value={value} onFocus={(event) => event.currentTarget.select()} />
      <button type="button" className="btn" onClick={() => void navigator.clipboard?.writeText(value)}>
        {t("roomCopy")}
      </button>
    </div>
  );
}

export function RoomPanel({ worldId, onClose }: { worldId: string; onClose: () => void }) {
  const [status, setStatus] = useState<RoomStatus | null>(null);
  const [server, setServer] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<RoomStatus>("room_status"));
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const run = async (command: string, args?: Record<string, unknown>) => {
    setBusy(true);
    setError("");
    try {
      await invoke(command, args);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const otherTable = status?.active === true && status.worldId !== worldId;
  const here = status?.active === true && status.worldId === worldId;
  const end = here ? (
    <button type="button" className="btn" disabled={busy} onClick={() => void run("room_exit")}>
      {t("roomExit")}
    </button>
  ) : (
    <button
      type="button"
      className="btn btn-primary"
      disabled={busy || status === null || otherTable}
      onClick={() => void run("room_enter", { worldId, server: server ?? status?.server ?? "" })}
    >
      {t("roomEnter")}
    </button>
  );

  return (
    <Dialog title={t("roomTitle")} size="m" onDismiss={onClose} closeButton end={end}>
      <p>{t("roomIntro")}</p>
      {!here && (
        <p className="risk-box" role="note">
          {t("roomRisk")}
        </p>
      )}
      {otherTable && <p role="alert">{t("roomOtherTable")}</p>}
      {!here && (
        <label className="room-row">
          <span className="room-label">{t("roomServer")}</span>
          <input
            className="room-value"
            value={server ?? status?.server ?? ""}
            onChange={(event) => setServer(event.target.value)}
            placeholder="https://"
          />
        </label>
      )}
      {here && status && (
        <>
          <p>{status.online ? t("roomOnline") : t("roomOffline")}</p>
          {status.keepAwake !== null && <p>{status.keepAwake ? t("roomKeepAwakeOn") : t("roomKeepAwakeOff")}</p>}
          {status.roomCode && (
            <div className="room-row">
              <span className="room-label">{t("roomCode")}</span>
              <strong>{status.roomCode}</strong>
            </div>
          )}
          {status.inviteUrl && <CopyRow label={t("roomInvite")} value={status.inviteUrl} />}
          {status.payerLink && <CopyRow label={t("roomPayerLink")} value={status.payerLink} />}
          <div className="room-row">
            <span className="room-label">{t("roomPairCode")}</span>
            <strong>{status.pairCode ?? "—"}</strong>
            <button type="button" className="btn" disabled={busy || !status.online} onClick={() => void run("room_pair_code")}>
              {t("roomResetHost")}
            </button>
          </div>
          <div className="room-row">
            <span className="room-label">{t("roomModel")}</span>
            <span>{status.modelLabel === "default" ? t("roomModelDefault") : (status.modelLabel ?? "—")}</span>
          </div>
          {status.lastError && (
            <div className="room-row">
              <span className="room-label">{t("roomLastError")}</span>
              <span>{status.lastError}</span>
            </div>
          )}
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </Dialog>
  );
}
