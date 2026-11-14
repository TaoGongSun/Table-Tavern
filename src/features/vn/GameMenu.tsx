// 遊戲選單（Ren'Py game menu）：Esc、右鍵或快捷列「選單」開；桌名當標題，按鈕直列。
// 非重播四組：進度（存讀、進度頁、美術）、劇情操作（結束本章）、系統（設定、更多）、離開。
// 重播桌沒有旗標，不出進度頁；章節與結局各自一顆。沒有結束本章與「更多…」，隱藏介面留在主選單。
import { t } from "../../i18n";
import { Panel } from "./StoryPanels";

export interface GameMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** 從這項起另成一段（組與組之間） */
  section?: boolean;
}

interface GameMenuActions {
  replay: boolean;
  busy: boolean;
  nextDisabled: boolean;
  open: (panel: "saves" | "settings" | "endings" | "gallery" | "chapters" | "progress") => void;
  /** 先收起選單再做（離開播放畫面或要看到結果的項目） */
  close: () => void;
  openMore: () => void;
  onTitle: () => void;
  onNextChapter: () => void;
  onShowChat: () => void;
  onOpenScript: () => void;
  onExportCard: () => void;
  onExportWork: () => void;
  onHide: () => void;
  onLobby: () => void;
}

const closing = (o: GameMenuActions, run: () => void) => () => {
  o.close();
  run();
};

/** 主選單。確認「結束本章」在播放器的 `onNextChapter` 裡做，這裡只負責叫它。 */
export function gameMenuItems(o: GameMenuActions): GameMenuItem[] {
  return [
    { key: "saves", label: t("vnSaves"), disabled: o.busy, onSelect: () => o.open("saves") },
    ...(o.replay
      ? [
          { key: "replay", label: t("vnWorkChapterSelect"), onSelect: () => o.open("chapters") },
          { key: "endings", label: t("vnEndingsList"), onSelect: () => o.open("endings") },
        ]
      : [{ key: "progress", label: t("vnProgressPanel"), onSelect: () => o.open("progress") }]),
    { key: "gallery", label: t("vnGallery"), onSelect: () => o.open("gallery") },
    ...(o.replay
      ? []
      : [
          {
            key: "next",
            label: t("vnNextChapter"),
            disabled: o.nextDisabled,
            onSelect: closing(o, o.onNextChapter),
            section: true,
          },
        ]),
    { key: "settings", label: t("vnTitleSettings"), onSelect: () => o.open("settings"), section: true },
    ...(o.replay
      ? [{ key: "hide", label: t("vnHideUi"), onSelect: closing(o, o.onHide) }]
      : [{ key: "more", label: t("vnMenuMore"), onSelect: o.openMore }]),
    { key: "title", label: t("vnBackToTitle"), disabled: o.busy, onSelect: closing(o, o.onTitle), section: true },
    { key: "lobby", label: t("backToLobby"), onSelect: o.onLobby },
  ];
}

/** 「更多…」：隱藏介面放第一項（H 鍵不經過這裡）；其餘是聊天視圖、劇本（世界設定的劇本分頁）與兩種匯出 */
export function gameMoreItems(o: GameMenuActions): GameMenuItem[] {
  return [
    { key: "hide", label: t("vnHideUi"), onSelect: closing(o, o.onHide) },
    { key: "chat", label: t("vnShowChat"), onSelect: o.onShowChat },
    { key: "script", label: t("vnBibleTab"), onSelect: o.onOpenScript },
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
