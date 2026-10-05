import { type Dispatch, type SetStateAction, useRef } from "react";
import type { AppConfig } from "../shared/contracts/backend-contracts";
import { PALETTE } from "../features/characters/card-model";
import { FormatBanner, FormatRepair } from "../features/world-format/FormatNotice";
import type { RepairNotice, TableGate } from "../features/world-format/open-world";
import type { CardInterfaceController } from "../features/card-interface/useCardInterfaceController";
import type { CharacterController } from "../features/characters/useCharacterController";
import type { ChatController } from "../features/play/useChatController";
import type { ImportController } from "../features/import/useImportController";
import type { SceneActions } from "../features/play/useSceneActions";
import type { TableStateController } from "../features/table-state/useTableStateController";
import {
  GM_TARGET,
  type WorkspaceNavigationController,
} from "../controllers/useWorkspaceNavigationController";
import { t } from "../i18n";
import { ErrorNote } from "../shared/ui/atoms";
import { CardInterfaceOverlay } from "../features/card-interface/CardInterfaceOverlay";
import { MainView } from "./MainView";
import { Onboarding } from "../features/ai-connection/Onboarding";
import { PlayView } from "../features/play/PlayView";
import { CastRail } from "../features/characters/CastRail";
import { StateBar } from "../features/table-state/StateBar";
import { TableToolbar } from "./TableToolbar";
import type { SettingsTab } from "../features/settings/SettingsWindow";
import type { PendingRefactorCard } from "../features/refactor/refactor-card";

// GM 卡的銅金色：發言對象晶片沿用書皮的 --fac，與角色卡的陣營色區隔
const GM_COLOR = "#8a6a3c";

export type EditingTableName = { value: string } | null;

interface AppWorkspaceProps {
  table: string;
  tableName: string;
  scene: number;
  editingName: EditingTableName;
  setEditingName: Dispatch<SetStateAction<EditingTableName>>;
  hasStateBar: boolean;
  chattedSinceImport: boolean;
  worldEditorRefreshKey: number;
  config: AppConfig;
  error: string;
  transport: string | undefined;
  characters: CharacterController;
  chat: ChatController;
  tableState: TableStateController;
  cardInterface: CardInterfaceController;
  imports: ImportController;
  navigation: WorkspaceNavigationController;
  sceneActions: SceneActions;
  /** 進出桌進行中：回大廳與換幕停用 */
  tableOpBusy: boolean;
  onRenameTable: (raw: string) => void;
  onGoLobby: () => void;
  onUndoImport: () => void;
  /** 匯入或撤銷匯入正排在進行中的回合後面 */
  turnWaiting: boolean;
  onOpenSettings: (tab: SettingsTab) => void;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onConfigSaved: (config: AppConfig) => void;
  onEntryConverted: () => Promise<void>;
  onRefactorApplied: (live: () => boolean) => Promise<void>;
  pendingRefactorCard: PendingRefactorCard | null;
  onPendingRefactorCardTaken: (generation: number) => void;
  gate: TableGate;
  readOnlyNotice: { appVersion: string | null; backupAvailable: boolean } | null;
  repairNotice: RepairNotice | null;
  skippedLines: number;
  onUseBackup: () => void;
  onOpenRepairFolder: () => void;
  /** 有沒被略過的新版：工具列齒輪掛紅點，點了直接開版本分頁 */
  updateDot: boolean;
}

export function AppWorkspace({
  table,
  tableName,
  scene,
  editingName,
  setEditingName,
  hasStateBar,
  chattedSinceImport,
  worldEditorRefreshKey,
  config,
  error,
  transport,
  characters,
  chat,
  tableState,
  cardInterface,
  imports,
  navigation,
  sceneActions,
  tableOpBusy,
  onRenameTable,
  onGoLobby,
  onUndoImport,
  turnWaiting,
  onOpenSettings,
  onPreference,
  onConfigSaved,
  onEntryConverted,
  onRefactorApplied,
  pendingRefactorCard,
  onPendingRefactorCardTaken,
  gate,
  readOnlyNotice,
  repairNotice,
  skippedLines,
  onUseBackup,
  onOpenRepairFolder,
  updateDot,
}: AppWorkspaceProps) {
  const {
    leaveGuard,
    speaker,
    setSpeaker,
    mainView,
    setMainView,
    cardView,
    editingPlayerCard,
    gmTargeted,
    editCard,
    openPlayerCard,
    selectCard,
    selectGm,
    openWorldEditor,
    openNewCard,
    openSceneReader,
    deleteCharacter,
    deletePlayerCard,
    finishCardSaved,
    finishPlayerCardSaved,
    finishRemoval,
  } = navigation;
  const {
    canUndoScene,
    advanceScene,
    forkScene,
    revertScene,
    regenerateSummary,
    exportTranscript,
    sceneDisplayLabel,
    sceneChipLabel,
  } = sceneActions;

  // 改桌名的輸入框：包成表單讓 Enter 走瀏覽器的表單送出，
  // 中文輸入法組字中的 Enter 會被輸入法吃掉（對話輸入框同款做法），不會誤判成確認改名
  // 一次改名只結算一次：Enter 送出後輸入框卸載會再引發 blur、Esc 取消後也一樣，
  // 不擋的話前者多送一次 rename_world、後者取消了還是存下去
  const renameSettled = useRef(true);
  function startRename(name: string) {
    renameSettled.current = false;
    setEditingName({ value: name });
  }
  function settleRename(value: string | null) {
    if (renameSettled.current) return;
    renameSettled.current = true;
    if (value === null) setEditingName(null);
    else onRenameTable(value);
  }

  function renameForm(className: string) {
    const value = editingName?.value ?? "";
    return (
      <form
        className="table-title-form"
        onSubmit={(event) => {
          event.preventDefault();
          settleRename(value);
        }}
      >
        <input
          className={className}
          autoFocus
          value={value}
          aria-label={t("tableNameAria")}
          onChange={(event) => {
            const next = event.currentTarget.value;
            setEditingName((previous) => (previous ? { value: next } : previous));
          }}
          onBlur={() => settleRename(value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") settleRename(null);
          }}
        />
      </form>
    );
  }

  const targetName = gmTargeted ? "GM" : (characters.metaOf(speaker)?.name ?? speaker);
  const requestReplyLabel = t("requestReplyBtn", {
    name: speaker ? targetName : t("characterFallback"),
  });
  // AI 錯誤：遊玩畫面放在輸入框上方（內捲、不推動送出鈕）；編輯頁、前幕閱讀、需修復時留在主欄底
  const errorNote = error ? <ErrorNote text={error} transport={transport} /> : null;
  const errorInComposer = gate !== "repair" && mainView === null;
  const generatingMeta = chat.generating !== null ? characters.metaOf(chat.generating.id) : undefined;

  return (
    <>
      <CastRail
        gmId={GM_TARGET}
        speaker={speaker}
        locked={gate !== "play"}
        gmImage={characters.gmImage}
        player={characters.player}
        playerImage={characters.playerImage}
        playerAvatar={characters.playerAvatar}
        cast={characters.active}
        images={characters.images}
        avatars={characters.avatars}
        archived={characters.archived}
        onReorder={(ordered) => void characters.reorder(ordered)}
        onSelectGm={() => void selectGm()}
        onOpenWorldEditor={() => void openWorldEditor()}
        onOpenPlayerCard={() => void openPlayerCard()}
        onSelectCard={(id) => void selectCard(id)}
        onEditCard={(id) => void editCard(id)}
        onRestore={(id) => void characters.restore(id)}
        onRestoreAutoHidden={(id) => void characters.restoreAutoHidden(id)}
        onDeleteCharacter={(id) => void deleteCharacter(id)}
        onCreateCard={() => void openNewCard()}
        onImportFile={(file) => void imports.importFile(file)}
        canUndoImport={imports.receipts.length > 0 && !chattedSinceImport}
        onUndoImport={() => void onUndoImport()}
        turnWaiting={turnWaiting}
      />

      <main className="chat-main">
        <TableToolbar
          tableName={tableName}
          renaming={editingName !== null}
          renameForm={renameForm}
          onStartRename={startRename}
          locked={gate !== "play"}
          showCardInterface={mainView === null && cardInterface.shellReady}
          onOpenCardInterface={() => cardInterface.open()}
          busy={chat.busy || tableOpBusy}
          onGoLobby={onGoLobby}
          hasEvents={chat.events.length > 0}
          onAdvanceScene={advanceScene}
          onExportTranscript={exportTranscript}
          scene={scene}
          sceneLabel={sceneChipLabel(scene)}
          sceneLabelOf={sceneDisplayLabel}
          onOpenScene={(n) => void openSceneReader(n)}
          updateDot={updateDot}
          onOpenSettings={() => onOpenSettings("appearance")}
          onOpenVersions={() => onOpenSettings("versions")}
        />

        {gate === "repair" && repairNotice ? (
          <FormatRepair notice={repairNotice} onOpenFolder={onOpenRepairFolder} />
        ) : (
          <>
            {gate === "readonly" && readOnlyNotice && (
              <FormatBanner
                version={readOnlyNotice.appVersion}
                backupAvailable={readOnlyNotice.backupAvailable}
                skipped={skippedLines}
                onUseBackup={onUseBackup}
              />
            )}
            {/* 介面由 App 接管的桌不顯示頂部狀態欄：狀態在卡片畫面裡，模型也不再寫 state 圍欄 */}
            {gate === "play" &&
              mainView === null &&
              !cardInterface.interfaceTakeover &&
              (hasStateBar || Object.keys(tableState.tree).length > 0) && (
              <StateBar
                fields={tableState.fields}
                tree={tableState.tree}
                jumps={tableState.jumps}
                bindings={tableState.bindings}
                editing={tableState.editing}
                onBeginEdit={tableState.beginEdit}
                onChangeEditValue={tableState.changeEditValue}
                onSave={(path, tree, value) => void tableState.save(path, tree, value)}
                onCancelEdit={tableState.cancelEdit}
                onMarkCounter={(path) => void tableState.markCounter(path)}
                onBind={(characterId, path) => void tableState.bind(characterId, path)}
                player={characters.player}
                castCount={characters.list.length}
                cast={characters.active}
              />
            )}

            <MainView
              sceneLabelOf={sceneDisplayLabel}
              world={table}
              worldName={tableName}
              sceneReading={mainView?.kind === "scene" ? mainView.n : null}
              onFork={(n) => void forkScene(n)}
              cardKind={cardView?.kind ?? null}
              cardId={cardView?.id ?? ""}
              cardName={characters.metaOf(cardView?.id ?? "")?.name ?? ""}
              editingPlayerCard={editingPlayerCard}
              nextColor={PALETTE[characters.list.length % PALETTE.length]}
              cardImage={
                editingPlayerCard
                  ? (characters.playerImage ?? undefined)
                  : characters.images[cardView?.id ?? ""]
              }
              cardAvatar={
                editingPlayerCard
                  ? (characters.playerAvatar ?? undefined)
                  : characters.avatars[cardView?.id ?? ""]
              }
              onImagesChanged={() =>
                editingPlayerCard
                  ? characters.reloadPlayer(characters.player?.id ?? null)
                  : characters.reloadImages()
              }
              onCardSaved={finishCardSaved}
              onPlayerCardSaved={finishPlayerCardSaved}
              onFinishRemoval={finishRemoval}
              onDeleteCharacter={deleteCharacter}
              onDeletePlayerCard={deletePlayerCard}
              onClose={() => setMainView(null)}
              leaveGuard={leaveGuard}
              config={config}
              onPreference={onPreference}
              onOpenAiSettings={() => onOpenSettings("ai")}
              isBusy={chat.isBusy}
              worldOpen={mainView?.kind === "world"}
              worldEditorRefreshKey={worldEditorRefreshKey}
              onEntryConverted={onEntryConverted}
              onRefactorApplied={onRefactorApplied}
              pendingRefactorCard={pendingRefactorCard}
              onPendingRefactorCardTaken={onPendingRefactorCardTaken}
              playView={
                <PlayView
                  onboarding={<Onboarding config={config} onSaved={onConfigSaved} />}
                  sceneLabel={sceneDisplayLabel(scene)}
                  storyKey={`${table}\u0000${scene}`}
                  events={chat.events}
                  notices={chat.notices}
                  metaOf={characters.metaOf}
                  generating={chat.generating}
                  generatingMeta={generatingMeta}
                  streamText={chat.streamText}
                  busy={chat.busy || tableOpBusy}
                  canRestore={chat.canRestore}
                  onRestoreUndone={() => void chat.restoreUndone()}
                  locked={gate === "readonly"}
                  canUndoScene={gate === "play" && canUndoScene}
                  onRegenerateSummary={() => void regenerateSummary()}
                  onRevertScene={() => void revertScene()}
                  awayTooLong={chat.awayTooLong}
                  speaker={speaker}
                  gmTargeted={gmTargeted}
                  targetName={targetName}
                  targetColor={
                    gmTargeted ? GM_COLOR : (characters.metaOf(speaker)?.color ?? "#888888")
                  }
                  targetImage={
                    gmTargeted ? characters.gmImage : (characters.avatars[speaker] ?? null)
                  }
                  targetEmoji={characters.metaOf(speaker)?.avatar ?? "🎭"}
                  onClearTarget={() => setSpeaker("")}
                  input={chat.input}
                  onInputChange={chat.setInput}
                  castEmpty={characters.active.length === 0}
                  onSubmit={chat.send}
                  canStop={chat.canStop}
                  onStop={chat.stopResponse}
                  requestReplyLabel={requestReplyLabel}
                  onUndoLast={() => void chat.undoLast()}
                  onRequestReply={() => void chat.replyFromTarget()}
                  onGmNarrate={chat.gmNarrate}
                  onGmAdvance={chat.gmAdvance}
                  errorNote={errorInComposer ? errorNote : null}
                />
              }
            />
          </>
        )}
        {!errorInComposer && errorNote && <div className="main-error">{errorNote}</div>}
      </main>

      {/* 卡片自帶介面整面取代對話；殼本身已含敘事畫面，不用再疊聊天記錄；且只在遊玩畫面出現——
          切去編輯畫面時 mainView 不再是 null，這裡直接不渲染，覆蓋層就跟著消失，不用另外清 cardUiOpen */}
      {gate === "play" && mainView === null && cardInterface.uiOpen && cardInterface.shellReady && (
        <CardInterfaceOverlay
          generatingName={
            chat.generating === null
              ? null
              : chat.generating.kind === "narration"
                ? "GM"
                : (generatingMeta?.name ?? "GM")
          }
          shellDoc={cardInterface.shellDoc}
          shellKey={cardInterface.shellKey}
          chat={cardInterface.chat}
          mvu={cardInterface.mvu}
          onClose={() => cardInterface.close()}
        />
      )}
    </>
  );
}
