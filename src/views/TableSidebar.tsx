import { ReactNode, useRef, useState } from "react";
import { t } from "../i18n";
import { tierLabel } from "../model-catalog";
import { WorldMeta } from "../backend-contracts";
import { CharacterCard, CharacterMeta } from "../card-model";
import { useDragReorder } from "../drag-reorder";
import gmBook from "../assets/gm-book.png";

// 側欄寬度是純 UI 狀態，存瀏覽器端即可，不進 config.json。
// 下限擋在這裡，上限交給 CSS 的 max-width: 50%（視窗縮小時自動夾住）。
const SIDEBAR_WIDTH_KEY = "sidebar_width";
const TABLE_LIST_OPEN_KEY = "table_list_open";
const SIDEBAR_DEFAULT_WIDTH = 224;
const SIDEBAR_MIN_WIDTH = 176;
const SIDEBAR_KEY_STEP = 16;

interface TableSidebarProps {
  // 桌次清單
  worlds: WorldMeta[];
  table: string;
  /** 生成中：開桌／刪桌一律鎖住 */
  busy: boolean;
  /** 改名輸入框正落在側欄這一列（主欄標題那個入口由 header 自己判斷） */
  renamingTable: boolean;
  renameForm: (className: string) => ReactNode;
  onStartRename: (name: string) => void;
  onNewTable: () => void;
  onGenerateTable: () => void;
  onSwitchTable: (id: string) => void;
  onDeleteTable: (id: string) => void;
  // 角色區
  /** 發言對象是 GM 時的代號（App 持有 speaker，這裡只拿它比對） */
  gmId: string;
  /** 側欄描邊的那張：編輯畫面時是正在編輯的卡，其餘畫面是發言對象 */
  selectedCard: string;
  /** 聊天畫面當下的發言對象（其他畫面為空字串）：決定提示語是「取消」還是「選為」 */
  speakingCard: string;
  gmImage: string | null;
  player: CharacterCard | null;
  playerImage: string | null;
  playerAvatar: string | null;
  cast: CharacterMeta[];
  images: Record<string, string>;
  avatars: Record<string, string>;
  archived: CharacterMeta[];
  onReorder: (ordered: CharacterMeta[]) => void;
  onSelectGm: () => void;
  onOpenWorldEditor: () => void;
  onOpenPlayerCard: () => void;
  onSelectCard: (id: string) => void;
  onEditCard: (id: string) => void;
  onRestore: (id: string) => void;
  onRestoreAutoHidden: (id: string) => void;
  onDeleteCharacter: (id: string) => void;
  onCreateCard: () => void;
  onImportFile: (file: File) => void;
  /** 這桌有匯入紀錄且還沒向 AI 開演：復原鈕才掛得上 */
  canUndoImport: boolean;
  onUndoImport: () => void;
  onOpenSettings: () => void;
}

/** 桌次清單＋角色側欄。寬度與展開狀態是側欄自己的 UI 記憶，其餘一律由 App 注入 */
export function TableSidebar({
  worlds,
  table,
  busy,
  renamingTable,
  renameForm,
  onStartRename,
  onNewTable,
  onGenerateTable,
  onSwitchTable,
  onDeleteTable,
  gmId,
  selectedCard,
  speakingCard,
  gmImage,
  player,
  playerImage,
  playerAvatar,
  cast,
  images,
  avatars,
  archived,
  onReorder,
  onSelectGm,
  onOpenWorldEditor,
  onOpenPlayerCard,
  onSelectCard,
  onEditCard,
  onRestore,
  onRestoreAutoHidden,
  onDeleteCharacter,
  onCreateCard,
  onImportFile,
  canUndoImport,
  onUndoImport,
  onOpenSettings,
}: TableSidebarProps) {
  const [sidebarWidth, setSidebarWidth] = useState(
    () => Number(localStorage.getItem(SIDEBAR_WIDTH_KEY)) || SIDEBAR_DEFAULT_WIDTH,
  );
  const [tableListOpen, setTableListOpen] = useState(
    () => localStorage.getItem(TABLE_LIST_OPEN_KEY) !== "false",
  );
  const importInputRef = useRef<HTMLInputElement>(null);
  const castDrag = useDragReorder(cast, (character) => character.id, onReorder);

  // 拖曳分隔線調側欄寬度：上限由 CSS max-width 夾住，這裡只擋下限
  function resizeSidebar(next: number) {
    const clamped = Math.min(Math.max(next, SIDEBAR_MIN_WIDTH), window.innerWidth / 2);
    setSidebarWidth(clamped);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(Math.round(clamped)));
  }

  function startSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const onMove = (moveEvent: PointerEvent) => resizeSidebar(moveEvent.clientX);
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  return (
    <>
      <aside className="sidebar" style={{ width: sidebarWidth }}>
        <details
          className="table-section"
          open={tableListOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setTableListOpen(next);
            localStorage.setItem(TABLE_LIST_OPEN_KEY, String(next));
          }}
        >
          <summary>{t("tableListAria")}</summary>
          <div className="table-section-content">
            <button className="new-table" onClick={onNewTable} disabled={busy}>
              {t("newTable")}
            </button>
            <button className="gen-table" onClick={onGenerateTable} disabled={busy}>
              {t("genTableBtn")}
            </button>
            <nav className="table-list" aria-label={t("tableListAria")}>
              {worlds.map((w) => (
                <div className="table-row" key={w.id}>
                  {/* 目前這桌再點一次＝改名（切桌沒意義），與主欄標題同一個入口 */}
                  {renamingTable && w.id === table ? (
                    renameForm("table-item-input")
                  ) : (
                    <button
                      className={`table-item ${w.id === table ? "table-item-active" : ""}`}
                      title={w.id === table ? t("renameHint") : undefined}
                      onClick={() => (w.id === table ? onStartRename(w.name) : onSwitchTable(w.id))}
                    >
                      {w.name}
                    </button>
                  )}
                  <button
                    type="button"
                    className="table-delete"
                    aria-label={t("deleteTableTitle")}
                    title={t("deleteTableTitle")}
                    disabled={busy}
                    onClick={() => onDeleteTable(w.id)}
                  >
                    ✕
                  </button>
                </div>
              ))}
            </nav>
          </div>
        </details>
        <section className="character-panel" aria-label={t("castAria")}>
          <div className="character-list">
            {/* GM 卡：與角色卡同款同尺寸同操作（GM 是桌上最重要的一位）——點擊選為發言對象，右下編輯鈕開世界設定＋世界書 */}
            <div
              role="button"
              tabIndex={0}
              className={`tcard tcard-gm ${selectedCard === gmId ? "tcard-selected" : ""}`}
              title={speakingCard === gmId ? t("gmTargetHintClear") : t("gmTargetHint")}
              onClick={() => onSelectGm()}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelectGm();
                }
              }}
            >
              <span className="tcard-art">
                {gmImage ? (
                  <img className="tcard-image" src={gmImage} alt="" />
                ) : (
                  <img className="gm-book" src={gmBook} alt="" />
                )}
              </span>
              <span className="tcard-body">
                <span className="tcard-name-row">
                  <span className="tcard-plate">GM</span>
                </span>
              </span>
              <button
                type="button"
                className="character-card-edit"
                aria-label={t("worldSummary")}
                title={t("worldSummary")}
                onClick={(event) => {
                  event.stopPropagation();
                  onOpenWorldEditor();
                }}
              >
                {t("editBtn")}
              </button>
            </div>
            <div
              role="button"
              tabIndex={0}
              className={`tcard tcard-player${player ? "" : " tcard-player-empty"}`}
              title={t(player ? "playerCardHint" : "playerCardEmptyHint")}
              onClick={() => onOpenPlayerCard()}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onOpenPlayerCard();
                }
              }}
            >
              {player ? (
                <>
                  <span className="tcard-art">
                    {player.show_image && playerImage ? (
                      <img className="tcard-image" src={playerImage} alt="" />
                    ) : playerAvatar ? (
                      <img className="avatar-round tcard-avatar" src={playerAvatar} alt="" />
                    ) : (
                      <span aria-hidden="true">{player.avatar}</span>
                    )}
                  </span>
                  <span className="tcard-body">
                    <span className="tcard-name-row">
                      <span className="tcard-plate">{player.name}</span>
                    </span>
                  </span>
                </>
              ) : (
                <span className="tcard-body">{t("playerCardEmpty")}</span>
              )}
            </div>
            {/* 角色卡＝桌遊組件卡：圖窗＋名字 wedge＋檔位寶石（中＝預設檔位，不掛寶石） */}
            {castDrag.order.map((c) => (
              <div
                key={c.id}
                role="button"
                tabIndex={0}
                className={`tcard ${selectedCard === c.id ? "tcard-selected" : ""}${
                  castDrag.draggingKey === c.id ? " row-dragging" : ""
                }`}
                style={{ ["--fac" as string]: c.color }}
                onClick={() => {
                  if (castDrag.justDragged()) return;
                  onSelectCard(c.id);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelectCard(c.id);
                  }
                }}
                title={`${
                  speakingCard === c.id
                    ? t("castHintClear", { name: c.name })
                    : t("castHint", { name: c.name })
                }｜${t("dragToReorder")}`}
                {...castDrag.rowProps(c)}
              >
                <span className="tcard-art">
                  {c.show_image && images[c.id] ? (
                    <img className="tcard-image" src={images[c.id]} alt="" />
                  ) : avatars[c.id] ? (
                    <img className="avatar-round tcard-avatar" src={avatars[c.id]} alt="" />
                  ) : (
                    <span aria-hidden="true">{c.avatar}</span>
                  )}
                </span>
                <span className="tcard-body">
                  <span className="tcard-name-row">
                    <span className="tcard-plate">{c.name}</span>
                    {c.tier !== "balanced" && (
                      <span className="tcard-gem">{tierLabel(c.tier)}</span>
                    )}
                  </span>
                </span>
                <button
                  type="button"
                  className="character-card-edit"
                  aria-label={t("editCardSummary", { name: c.name })}
                  title={t("editCardSummary", { name: c.name })}
                  onClick={(event) => {
                    event.stopPropagation();
                    onEditCard(c.id);
                  }}
                >
                  {t("editBtn")}
                </button>
              </div>
            ))}
          </div>
          {archived.length > 0 && (
            <details className="archive-section">
              <summary>{t("archiveSectionTitle")}</summary>
              <div className="archive-list">
                {archived.map((character) => {
                  // 沒被玩家手動封存、純粹本幕還沒出場才算自動隱藏；封存優先於自動隱藏顯示
                  const isAutoHidden = !character.archived && character.auto_hidden;
                  return (
                    <div className="archive-row" key={character.id}>
                      <span className="archive-row-name">
                        <span className="archive-row-name-text">{character.name}</span>
                        {isAutoHidden && (
                          <span className="archive-row-badge">{t("autoHiddenBadge")}</span>
                        )}
                      </span>
                      {/* 隱藏卡也要進得了編輯器：轉成世界書條目只能在隱藏狀態下按 */}
                      <button type="button" onClick={() => onEditCard(character.id)}>
                        {t("editBtn")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          isAutoHidden ? onRestoreAutoHidden(character.id) : onRestore(character.id)
                        }
                      >
                        {t("restoreCharacter")}
                      </button>
                      <button
                        type="button"
                        className="delete-character"
                        onClick={() => onDeleteCharacter(character.id)}
                      >
                        {t("deleteCharacter")}
                      </button>
                    </div>
                  );
                })}
              </div>
            </details>
          )}
          {/* 建卡＝直接開空白角色卡編輯器，名字與內容都在那邊填（2026-07-27 使用者拍板） */}
          <div className="character-create">
            <button type="button" onClick={() => onCreateCard()}>
              {t("createCard")}
            </button>
            <button
              type="button"
              title={t("importCardHint")}
              onClick={() => importInputRef.current?.click()}
            >
              {t("importCard")}
            </button>
            <input
              ref={importInputRef}
              type="file"
              accept=".png,.json,image/png,application/json"
              hidden
              onChange={(e) => {
                const file = e.currentTarget.files?.[0];
                e.currentTarget.value = "";
                if (file) onImportFile(file);
              }}
            />
            {canUndoImport && (
              <button type="button" title={t("undoLastImportHint")} onClick={() => onUndoImport()}>
                {t("undoLastImport")}
              </button>
            )}
          </div>
        </section>
        <div className="sidebar-footer">
          <button className="settings-open" onClick={onOpenSettings}>
            ⚙️ {t("settingsBtn")}
          </button>
        </div>
      </aside>

      <div
        className="sidebar-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label={t("sidebarResizerAria")}
        aria-valuenow={Math.round(sidebarWidth)}
        tabIndex={0}
        onPointerDown={startSidebarResize}
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft") resizeSidebar(sidebarWidth - SIDEBAR_KEY_STEP);
          if (e.key === "ArrowRight") resizeSidebar(sidebarWidth + SIDEBAR_KEY_STEP);
        }}
        onDoubleClick={() => resizeSidebar(SIDEBAR_DEFAULT_WIDTH)}
      />
    </>
  );
}
