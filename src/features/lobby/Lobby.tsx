// 大廳：沒有載入任何桌時的畫面。頂列（品牌、版本、設定）、通知區、新桌區、牌桌卡片牆。
// 新桌入口集中在「新桌區」一塊，日後換成引導精靈時整塊替換即可。
import type { ReactNode } from "react";
import { t } from "../../i18n";
import type { WorldMeta } from "../../shared/contracts/backend-contracts";
import { IconPlus, IconSettings, IconSparkle } from "../../shared/ui/icons";
import { TableCard } from "./TableCard";

interface LobbyProps {
  worlds: WorldMeta[];
  /** 進出桌進行中或生成中：新桌入口與所有桌卡操作停用 */
  busy: boolean;
  /** 通知區：大廳錯誤、更新橫幅、尚未設定 AI 的引導（由 App 組好傳進來） */
  notices: ReactNode;
  /** 目前版本號；讀到之前是 null，不畫版本鈕 */
  appVersion: string | null;
  /** 有比目前新、且沒被略過的版本 */
  updateDot: boolean;
  onOpenSettings: () => void;
  onOpenVersions: () => void;
  onNewTable: () => void;
  onGenerateTable: () => void;
  onEnterTable: (id: string) => void;
  onRenameTable: (id: string, raw: string) => void;
  onDeleteTable: (id: string) => void;
}

export function Lobby({
  worlds,
  busy,
  notices,
  appVersion,
  updateDot,
  onOpenSettings,
  onOpenVersions,
  onNewTable,
  onGenerateTable,
  onEnterTable,
  onRenameTable,
  onDeleteTable,
}: LobbyProps) {
  return (
    <div className="lobby">
      <header className="lobby-top">
        <span className="lobby-wordmark">Table Tavern</span>
        <span className="toolbar-spacer" />
        {appVersion !== null && (
          <button
            type="button"
            className="btn btn-ghost lobby-version"
            aria-label={t(updateDot ? "versionButtonUpdateAria" : "versionButtonAria", {
              version: appVersion,
            })}
            title={updateDot ? t("versionButtonUpdateAria", { version: appVersion }) : undefined}
            onClick={onOpenVersions}
          >
            v{appVersion}
            {updateDot && <span className="update-dot" aria-hidden="true" />}
          </button>
        )}
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          aria-label={t("settingsBtn")}
          title={t("settingsBtn")}
          onClick={onOpenSettings}
        >
          <IconSettings />
        </button>
      </header>
      <div className="lobby-body">
        <div className="lobby-notices">{notices}</div>
        <div className="lobby-new">
          <button type="button" className="lobby-new-card" disabled={busy} onClick={onNewTable}>
            <span className="lobby-new-icon">
              <IconPlus />
            </span>
            <span className="lobby-new-text">
              <b>{t("newTable")}</b>
              <span>{t("lobbyNewTableHint")}</span>
            </span>
          </button>
          <button
            type="button"
            className="lobby-new-card"
            disabled={busy}
            onClick={onGenerateTable}
          >
            <span className="lobby-new-icon">
              <IconSparkle />
            </span>
            <span className="lobby-new-text">
              <b>{t("genTableBtn")}</b>
              <span>{t("lobbyGenTableHint")}</span>
            </span>
          </button>
        </div>
        <div className="lobby-heading">
          <h2>{t("lobbyYourTables")}</h2>
          <span>{t("lobbyTableCount", { n: worlds.length })}</span>
        </div>
        <section className="lobby-grid" aria-label={t("tableListAria")}>
          {worlds.map((world) => (
            <TableCard
              key={world.id}
              world={world}
              busy={busy}
              onEnter={onEnterTable}
              onRename={onRenameTable}
              onDelete={onDeleteTable}
            />
          ))}
        </section>
      </div>
    </div>
  );
}
