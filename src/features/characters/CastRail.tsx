// 左側陣容欄：GM 卡→玩家卡→角色卡→封存區，底部「＋ 新增」。
// 角色卡（不計 GM、玩家卡）2 張以上＝寬欄，0–1 張＝窄欄：窄欄只剩頭像與名字，單卡／只有世界書
// 這種最常見的起手狀況不必讓出一整條欄寬。寬窄完全由張數決定；手動拖過的欄寬只在寬欄生效，
// 窄欄固定 76px 且不寫回記憶，切回寬欄就是上次拖到的寬度。
import { useCallback, useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import type { CharacterCard, CharacterMeta } from "./card-model";
import { IconPlus } from "../../shared/ui/icons";
import { MoreMenu, type MoreMenuItem } from "../../shared/ui/MoreMenu";
import { useDragReorder } from "../../shared/ui/drag-reorder";
import { ArchiveSection, NarrowArchive } from "./CastArchive";
import {
  NarrowCastTile,
  NarrowGmTile,
  NarrowPlayerTile,
  WideCastCard,
  WideGmCard,
  WidePlayerCard,
} from "./CastCards";

// 欄寬是純 UI 狀態，存瀏覽器端即可，不進 config.json。
// 上限＝主欄至少留 MAIN_MIN_WIDTH：JS 這邊夾拖曳值，CSS 的 max-width 夾視窗縮小時的既有值。
const SIDEBAR_WIDTH_KEY = "sidebar_width";
const SIDEBAR_DEFAULT_WIDTH = 224;
const SIDEBAR_MIN_WIDTH = 176;
const MAIN_MIN_WIDTH = 520;
const SIDEBAR_KEY_STEP = 16;

interface CastRailProps {
  /** 發言對象是 GM 時的代號（App 持有 speaker，這裡只拿它比對） */
  gmId: string;
  /** 目前的發言對象；空字串＝還沒選。描邊與 aria-pressed 一律看它 */
  speaker: string;
  /** 唯讀或需要修復：寫入動作停用（不隱藏，位置不動） */
  locked: boolean;
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
  /** 這桌有匯入紀錄且還沒向 AI 開演：復原項才掛得上 */
  canUndoImport: boolean;
  onUndoImport: () => void;
}

function storedWidth() {
  return Number(localStorage.getItem(SIDEBAR_WIDTH_KEY)) || SIDEBAR_DEFAULT_WIDTH;
}

export function CastRail(props: CastRailProps) {
  const {
    gmId,
    speaker,
    locked,
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
  } = props;
  const narrow = cast.length < 2;
  const [sidebarWidth, setSidebarWidth] = useState(storedWidth);
  const railRef = useRef<HTMLElement>(null);
  const importInputRef = useRef<HTMLInputElement>(null);
  const castDrag = useDragReorder(cast, (character) => character.id, onReorder);
  // 進行中的欄寬拖曳怎麼收尾：切成窄欄（分隔線消失）或卸載時要拆掉全域 listener
  const stopResize = useRef<(() => void) | null>(null);
  useEffect(() => () => stopResize.current?.(), [narrow]);

  function resizeSidebar(next: number) {
    const max = Math.max(SIDEBAR_MIN_WIDTH, window.innerWidth - MAIN_MIN_WIDTH);
    const clamped = Math.min(Math.max(next, SIDEBAR_MIN_WIDTH), max);
    setSidebarWidth(clamped);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(Math.round(clamped)));
  }

  function startSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const onMove = (moveEvent: PointerEvent) => resizeSidebar(moveEvent.clientX);
    const finish = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      stopResize.current = null;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
    stopResize.current = finish;
  }

  // 窄欄的封存鈕消失、或欄切回寬欄時，面板還開著：焦點交給「＋」鈕，不要掉到 body
  const focusAddButton = useCallback(() => {
    railRef.current?.querySelector<HTMLElement>(".rail-add")?.focus();
  }, []);

  const addItems: MoreMenuItem[] = [
    { key: "create", label: t("createCard"), onSelect: onCreateCard },
    {
      key: "import",
      label: t("importCard"),
      hint: t("importCardHint"),
      onSelect: () => importInputRef.current?.click(),
    },
    ...(canUndoImport
      ? [
          {
            key: "undo",
            label: t("undoLastImport"),
            hint: t("undoLastImportHint"),
            onSelect: onUndoImport,
          },
        ]
      : []),
  ];

  const archiveActions = {
    locked,
    onEdit: onEditCard,
    onRestore,
    onRestoreAutoHidden,
    onDelete: onDeleteCharacter,
  };

  return (
    <>
      <aside
        ref={railRef}
        className={`sidebar${narrow ? " sidebar-narrow" : ""}`}
        aria-label={t("castAria")}
        style={narrow ? undefined : { width: sidebarWidth }}
      >
        <div className="character-list">
          {narrow ? (
            <>
              <NarrowGmTile
                selected={speaker === gmId}
                image={gmImage}
                locked={locked}
                onSelect={onSelectGm}
                onEdit={onOpenWorldEditor}
              />
              <NarrowPlayerTile
                player={player}
                image={playerImage}
                avatar={playerAvatar}
                locked={locked}
                onOpen={onOpenPlayerCard}
              />
              {cast.map((character) => (
                <NarrowCastTile
                  key={character.id}
                  character={character}
                  selected={speaker === character.id}
                  image={images[character.id]}
                  avatar={avatars[character.id]}
                  locked={locked}
                  onSelect={() => onSelectCard(character.id)}
                  onEdit={() => onEditCard(character.id)}
                />
              ))}
            </>
          ) : (
            <>
              <WideGmCard
                selected={speaker === gmId}
                image={gmImage}
                locked={locked}
                onSelect={onSelectGm}
                onEdit={onOpenWorldEditor}
              />
              <WidePlayerCard
                player={player}
                image={playerImage}
                avatar={playerAvatar}
                locked={locked}
                onOpen={onOpenPlayerCard}
              />
              {castDrag.order.map((character) => (
                <WideCastCard
                  key={character.id}
                  character={character}
                  selected={speaker === character.id}
                  dragging={castDrag.draggingKey === character.id}
                  image={images[character.id]}
                  avatar={avatars[character.id]}
                  locked={locked}
                  dragProps={locked ? null : castDrag.rowProps(character)}
                  justDragged={castDrag.justDragged}
                  onSelect={() => onSelectCard(character.id)}
                  onEdit={() => onEditCard(character.id)}
                />
              ))}
              <ArchiveSection archived={archived} onEmpty={focusAddButton} {...archiveActions} />
            </>
          )}
        </div>
        <div className="rail-foot">
          {narrow && (
            <NarrowArchive
              archived={archived}
              onTriggerGone={focusAddButton}
              {...archiveActions}
            />
          )}
          {/* 建卡＝直接開空白角色卡編輯器，名字與內容都在那邊填（2026-07-27 使用者拍板） */}
          {narrow ? (
            <MoreMenu
              label={t("castAdd")}
              className="btn btn-ghost btn-icon rail-add"
              trigger={<IconPlus />}
              disabled={locked}
              items={addItems}
            />
          ) : (
            <MoreMenu
              label={t("castAdd")}
              className="btn rail-add"
              trigger={
                <>
                  <IconPlus />
                  <span className="btn-label">{t("castAdd")}</span>
                </>
              }
              ariaLabelTrigger={false}
              disabled={locked}
              items={addItems}
            />
          )}
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
        </div>
      </aside>

      {!narrow && (
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
      )}
    </>
  );
}
