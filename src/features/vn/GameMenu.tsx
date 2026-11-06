// 遊戲選單（Ren'Py game menu）：Esc、右鍵或快捷列「選單」開；桌名當標題，按鈕直列。
// 玩家常用項在上、離開類（回標題、回大廳）在最底；聊天視圖與匯出收進「更多…」子選單。重播桌不出生成類項目，也沒有子選單。
import { t } from "../../i18n";
import { Panel } from "./StoryPanels";

export interface GameMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** 從這項起另成一段（「更多…」、離開類） */
  section?: boolean;
}

interface GameMenuActions {
  replay: boolean;
  busy: boolean;
  nextDisabled: boolean;
  open: (panel: "saves" | "settings" | "endings" | "gallery" | "chapters") => void;
  /** 先收起選單再做（離開播放畫面或要看到結果的項目） */
  close: () => void;
  openMore: () => void;
  onTitle: () => void;
  onNextChapter: () => void;
  onShowChat: () => void;
  onExportCard: () => void;
  onExportWork: () => void;
  onHide: () => void;
  onLobby: () => void;
}

const closing = (o: GameMenuActions, run: () => void) => () => {
  o.close();
  run();
};

/** 主選單：存讀、設定、（下一章）、章節、結局、美術一覽、隱藏介面；（更多…）；回標題、回大廳 */
export function gameMenuItems(o: GameMenuActions): GameMenuItem[] {
  return [
    { key: "saves", label: t("vnSaves"), disabled: o.busy, onSelect: () => o.open("saves") },
    { key: "settings", label: t("vnTitleSettings"), onSelect: () => o.open("settings") },
    ...(o.replay
      ? []
      : [{ key: "next", label: t("vnNextChapter"), disabled: o.nextDisabled, onSelect: closing(o, o.onNextChapter) }]),
    { key: "replay", label: t(o.replay ? "vnWorkChapterSelect" : "vnReplayChapter"), onSelect: () => o.open("chapters") },
    { key: "endings", label: t("vnEndingsList"), onSelect: () => o.open("endings") },
    { key: "gallery", label: t("vnGallery"), onSelect: () => o.open("gallery") },
    { key: "hide", label: t("vnHideUi"), onSelect: closing(o, o.onHide) },
    ...(o.replay ? [] : [{ key: "more", label: t("vnMenuMore"), onSelect: o.openMore, section: true }]),
    { key: "title", label: t("vnBackToTitle"), disabled: o.busy, onSelect: closing(o, o.onTitle), section: true },
    { key: "lobby", label: t("backToLobby"), onSelect: o.onLobby },
  ];
}

/** 「更多…」子選單：聊天視圖與兩種匯出（只在非重播桌） */
export function gameMoreItems(o: GameMenuActions): GameMenuItem[] {
  return [
    { key: "chat", label: t("vnShowChat"), onSelect: o.onShowChat },
    { key: "export-card", label: t("vnCardExport"), disabled: o.busy, onSelect: o.onExportCard },
    { key: "export-work", label: t("vnWorkExport"), disabled: o.busy, onSelect: closing(o, o.onExportWork) },
  ];
}

/** `onBack` 有給＝子選單：頂部放「← 返回」；底部「關閉」照樣收起整個選單 */
export function GameMenu({
  title,
  items,
  onClose,
  onBack,
}: {
  title: string;
  items: GameMenuItem[];
  onClose: () => void;
  onBack?: () => void;
}) {
  return (
    <Panel label={title} onClose={onClose}>
      {onBack && (
        <button type="button" className="btn btn-sm vn-menu-back" onClick={onBack}>
          ← {t("settingsBack")}
        </button>
      )}
      <div className="vn-story-list vn-game-menu">
        {items.map((item) => (
          <button
            key={item.key}
            type="button"
            className={`btn${item.section ? " vn-menu-section" : ""}`}
            disabled={item.disabled}
            onClick={item.onSelect}
          >
            {item.label}
          </button>
        ))}
      </div>
    </Panel>
  );
}
