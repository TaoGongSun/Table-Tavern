// 牌桌頂部工具列：左邊桌名與幕晶片，右邊介面卡／換幕／設定／⋯。
// 單行不換行（800×600 為最小設計尺寸）：擠不下時桌名先縮，再縮兩顆有字鈕的標籤，圖示鈕不縮。
// 唯讀或需修復時按鈕只停用不消失，位置不隨狀態位移。
import type { ReactNode } from "react";
import { t } from "../i18n";
import {
  IconCardInterface,
  IconExpand,
  IconExport,
  IconSceneAdvance,
  IconSettings,
} from "../shared/ui/icons";
import { MoreMenu } from "../shared/ui/MoreMenu";

interface TableToolbarProps {
  tableName: string;
  /** 改名輸入框正落在工具列（側欄那個入口由 TableSidebar 自己判斷） */
  renaming: boolean;
  renameForm: (className: string) => ReactNode;
  onStartRename: (name: string) => void;
  /** 唯讀或需要修復：桌名不能改，寫入與嚴格讀取的動作停用 */
  locked?: boolean;
  /** 這桌有可用的卡片介面殼，且人在遊玩畫面 */
  showCardInterface: boolean;
  onOpenCardInterface: () => void;
  busy: boolean;
  hasEvents: boolean;
  onAdvanceScene: () => void;
  onExportTranscript: () => void;
  /** 目前第幾幕：0 代表還沒換過幕，幕晶片只是文字 */
  scene: number;
  /** 幕晶片文字（編號＋版本，不含幕名） */
  sceneLabel: string;
  actsOpen: boolean;
  onToggleActs: () => void;
  onCloseActs: () => void;
  /** 有沒被略過的新版：齒輪掛紅點，點了直接開版本分頁（判準同舊側欄版本鈕） */
  updateDot: boolean;
  onOpenSettings: () => void;
  onOpenVersions: () => void;
}

export function TableToolbar({
  tableName,
  renaming,
  renameForm,
  onStartRename,
  locked = false,
  showCardInterface,
  onOpenCardInterface,
  busy,
  hasEvents,
  onAdvanceScene,
  onExportTranscript,
  scene,
  sceneLabel,
  actsOpen,
  onToggleActs,
  onCloseActs,
  updateDot,
  onOpenSettings,
  onOpenVersions,
}: TableToolbarProps) {
  return (
    <header className="table-toolbar">
      {/* clip-path 會把按鈕自己的焦點框裁掉，框改畫在外層 */}
      <span className="table-title-wrap">
        {locked ? (
          <span className="table-title table-title-locked" title={tableName}>
            <span className="table-title-text">{tableName}</span>
          </span>
        ) : renaming ? (
          renameForm("table-title-input")
        ) : (
          <button
            type="button"
            className="table-title"
            title={`${tableName}\n${t("renameHint")}`}
            onClick={() => onStartRename(tableName)}
          >
            <span className="table-title-text">{tableName}</span>
          </button>
        )}
      </span>
      {/* 幕晶片＝前幕清單的入口；還沒換過幕就沒有前幕可看，只顯示文字 */}
      {scene > 0 ? (
        <button
          type="button"
          className="scene-chip"
          aria-expanded={actsOpen}
          disabled={locked}
          onClick={onToggleActs}
        >
          {sceneLabel}
          <IconExpand />
        </button>
      ) : (
        <span className="scene-chip scene-chip-static">{sceneLabel}</span>
      )}
      <span className="toolbar-spacer" />
      {/* 沒有可用殼的桌完全不出現這顆鈕——不是每張卡都帶介面；且只在遊玩畫面出現 */}
      {showCardInterface && (
        <button
          type="button"
          className="btn btn-ghost btn-shrink"
          title={t("cardInterfaceOpen")}
          disabled={locked}
          onClick={onOpenCardInterface}
        >
          <IconCardInterface />
          <span className="btn-label">{t("cardInterfaceOpen")}</span>
        </button>
      )}
      <button
        type="button"
        className="btn btn-shrink"
        title={t("sceneAdvanceHint")}
        disabled={locked || busy || !hasEvents}
        onClick={onAdvanceScene}
      >
        <IconSceneAdvance />
        <span className="btn-label">{t("sceneAdvance")}</span>
      </button>
      <span className="toolbar-sep" aria-hidden="true" />
      <button
        type="button"
        className="btn btn-ghost btn-icon toolbar-settings"
        aria-label={t(updateDot ? "settingsUpdateAria" : "settingsBtn")}
        title={t(updateDot ? "settingsUpdateAria" : "settingsBtn")}
        onClick={updateDot ? onOpenVersions : onOpenSettings}
      >
        <IconSettings />
        {updateDot && <span className="update-dot" aria-hidden="true" />}
      </button>
      <MoreMenu
        label={t("moreActions")}
        onOpen={onCloseActs}
        items={[
          {
            key: "export",
            label: t("exportTranscript"),
            hint: t("exportTranscriptHint"),
            icon: <IconExport />,
            disabled: locked,
            onSelect: onExportTranscript,
          },
        ]}
      />
    </header>
  );
}
