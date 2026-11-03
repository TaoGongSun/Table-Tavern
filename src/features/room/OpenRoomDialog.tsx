// 首頁「開房間」：風險說明、伺服器網址（自架用，收在「進階」），選角色卡（可多選，跟匯入同一條路）或不選卡直接開。
import { useRef, useState } from "react";
import { t } from "../../i18n";
import { Dialog } from "../../shared/ui/Dialog";

export function OpenRoomDialog({
  server: initialServer,
  onCancel,
  onOpen,
}: {
  server: string;
  onCancel: () => void;
  onOpen: (files: File[], server: string) => void;
}) {
  const [server, setServer] = useState(initialServer);
  const input = useRef<HTMLInputElement>(null);
  return (
    <Dialog title={t("roomTitle")} size="m" onDismiss={onCancel} closeButton>
      <p>{t("roomIntro")}</p>
      <p className="risk-box" role="note">
        {t("roomRisk")}
      </p>
      <details className="room-advanced">
        <summary>{t("roomAdvanced")}</summary>
        <label className="room-row">
          <span className="room-label">{t("roomServer")}</span>
          <input className="room-value" value={server} onChange={(event) => setServer(event.target.value)} placeholder="https://" />
        </label>
      </details>
      <div className="room-row">
        <button type="button" className="btn btn-primary" disabled={!server.trim()} onClick={() => input.current?.click()}>
          {t("roomPickCards")}
        </button>
        <button type="button" className="btn" disabled={!server.trim()} onClick={() => onOpen([], server)}>
          {t("roomNoCards")}
        </button>
      </div>
      <input
        ref={input}
        type="file"
        multiple
        accept=".png,.json,image/png,application/json"
        hidden
        data-testid="room-card-files"
        onChange={(event) => {
          const files = [...(event.currentTarget.files ?? [])];
          event.currentTarget.value = "";
          if (files.length > 0) onOpen(files, server);
        }}
      />
    </Dialog>
  );
}
