// VN 桌進桌後的畫面。播放器還沒做，先放桌名與說明，並可切到聊天視圖看這桌紀錄（反向切回在播放器那包）。
import type { ReactNode } from "react";
import { t } from "../../i18n";
import { IconHome } from "../../shared/ui/icons";

export function VnTableView({
  tableName,
  notice,
  busy,
  onShowChat,
  onGoLobby,
}: {
  tableName: string;
  /** 錯誤通知（匯入等流程在這個畫面上失敗時才看得到） */
  notice: ReactNode;
  busy: boolean;
  onShowChat: () => void;
  onGoLobby: () => void;
}) {
  return (
    <main className="vn-table-view">
      <header className="vn-table-top">
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          aria-label={t("backToLobby")}
          title={t("backToLobby")}
          disabled={busy}
          onClick={onGoLobby}
        >
          <IconHome />
        </button>
        <h1>{tableName}</h1>
      </header>
      {notice}
      <p>{t("vnPlayerPending")}</p>
      <button type="button" className="btn btn-primary" onClick={onShowChat}>
        {t("vnShowChat")}
      </button>
    </main>
  );
}
