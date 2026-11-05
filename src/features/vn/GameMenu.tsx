// 遊戲選單（Ren'Py game menu）：Esc、右鍵或快捷列「選單」開；桌名當標題，按鈕直列。重播桌不出生成類項目。
import { t } from "../../i18n";
import { Panel } from "./StoryPanels";

export interface GameMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** 進階動作（換章、聊天視圖、匯出、回大廳）從這項起另成一段 */
  section?: boolean;
}

/** 選單項目：回標題、存讀、設定、結局、美術一覽、章節、隱藏介面；另一段放進階動作——非重播桌的下一章、聊天視圖、匯出，最後回大廳 */
export function gameMenuItems(o: {
  replay: boolean;
  busy: boolean;
  nextDisabled: boolean;
  open: (panel: "saves" | "settings" | "endings" | "gallery" | "chapters") => void;
  /** 先收起選單再做（離開播放畫面或要看到結果的項目） */
  close: () => void;
  onTitle: () => void;
  onNextChapter: () => void;
  onShowChat: () => void;
  onExportCard: () => void;
  onExportWork: () => void;
  onHide: () => void;
  onLobby: () => void;
}): GameMenuItem[] {
  const closing = (run: () => void) => () => {
    o.close();
    run();
  };
  return [
    { key: "title", label: t("vnBackToTitle"), disabled: o.busy, onSelect: closing(o.onTitle) },
    { key: "saves", label: t("vnSaves"), disabled: o.busy, onSelect: () => o.open("saves") },
    { key: "settings", label: t("vnTitleSettings"), onSelect: () => o.open("settings") },
    { key: "endings", label: t("vnEndingsList"), onSelect: () => o.open("endings") },
    { key: "gallery", label: t("vnGallery"), onSelect: () => o.open("gallery") },
    { key: "replay", label: t(o.replay ? "vnWorkChapterSelect" : "vnReplayChapter"), onSelect: () => o.open("chapters") },
    { key: "hide", label: t("vnHideUi"), onSelect: closing(o.onHide) },
    ...(o.replay
      ? []
      : [
          { key: "next", label: t("vnNextChapter"), disabled: o.nextDisabled, onSelect: closing(o.onNextChapter), section: true },
          { key: "chat", label: t("vnShowChat"), onSelect: o.onShowChat },
          { key: "export-card", label: t("vnCardExport"), disabled: o.busy, onSelect: o.onExportCard },
          { key: "export-work", label: t("vnWorkExport"), disabled: o.busy, onSelect: closing(o.onExportWork) },
        ]),
    { key: "lobby", label: t("backToLobby"), onSelect: o.onLobby, section: o.replay },
  ];
}

export function GameMenu({ title, items, onClose }: { title: string; items: GameMenuItem[]; onClose: () => void }) {
  return (
    <Panel label={title} onClose={onClose}>
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
