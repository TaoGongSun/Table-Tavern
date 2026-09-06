import type { Dispatch, SetStateAction } from "react";
import type { AppConfig, WorldMeta } from "../backend-contracts";
import { PALETTE } from "../card-model";
import type { CardInterfaceController } from "../controllers/useCardInterfaceController";
import type { CharacterController } from "../controllers/useCharacterController";
import type { ChatController } from "../controllers/useChatController";
import type { ImportController } from "../controllers/useImportController";
import type { SceneActions } from "../controllers/useSceneActions";
import type { TableStateController } from "../controllers/useTableStateController";
import {
  GM_TARGET,
  type WorkspaceNavigationController,
} from "../controllers/useWorkspaceNavigationController";
import { t } from "../i18n";
import { ErrorNote } from "./atoms";
import { CardInterfaceOverlay } from "./CardInterfaceOverlay";
import { MainView } from "./MainView";
import { Onboarding } from "./Onboarding";
import { PlayView } from "./PlayView";
import { TableSidebar } from "./TableSidebar";
import { StateBar, WorkspaceHeader } from "./WorkspaceHeader";

// GM 卡的銅金色：發言對象晶片沿用書皮的 --fac，與角色卡的陣營色區隔
const GM_COLOR = "#8a6a3c";

export type EditingTableName = {
  at: "header" | "list";
  value: string;
} | null;

interface AppWorkspaceProps {
  worlds: WorldMeta[];
  table: string;
  tableName: string;
  scene: number;
  editingName: EditingTableName;
  setEditingName: Dispatch<SetStateAction<EditingTableName>>;
  hasStateBar: boolean;
  chattedSinceImport: boolean;
  worldEditorRefreshKey: number;
  config: AppConfig;
  sponsorUnlocked: boolean;
  error: string;
  transport: string | undefined;
  characters: CharacterController;
  chat: ChatController;
  tableState: TableStateController;
  cardInterface: CardInterfaceController;
  imports: ImportController;
  navigation: WorkspaceNavigationController;
  sceneActions: SceneActions;
  onRenameTable: (raw: string) => void;
  onNewTable: () => void;
  onGenerateTable: () => void;
  onSwitchTable: (id: string) => void;
  onDeleteTable: (id: string) => void;
  onUndoImport: () => void;
  onOpenSettings: (tab: "appearance" | "ai") => void;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onConfigSaved: (config: AppConfig) => void;
  onEntryConverted: () => Promise<void>;
  onRefactorApplied: () => Promise<void>;
}

export function AppWorkspace({
  worlds,
  table,
  tableName,
  scene,
  editingName,
  setEditingName,
  hasStateBar,
  chattedSinceImport,
  worldEditorRefreshKey,
  config,
  sponsorUnlocked,
  error,
  transport,
  characters,
  chat,
  tableState,
  cardInterface,
  imports,
  navigation,
  sceneActions,
  onRenameTable,
  onNewTable,
  onGenerateTable,
  onSwitchTable,
  onDeleteTable,
  onUndoImport,
  onOpenSettings,
  onPreference,
  onConfigSaved,
  onEntryConverted,
  onRefactorApplied,
}: AppWorkspaceProps) {
  const {
    leaveGuard,
    speaker,
    setSpeaker,
    mainView,
    setMainView,
    actsOpen,
    setActsOpen,
    cardView,
    editingPlayerCard,
    selectedCard,
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
  } = sceneActions;

  // 改桌名的輸入框（主欄標題與側欄共用）：包成表單讓 Enter 走瀏覽器的表單送出，
  // 中文輸入法組字中的 Enter 會被輸入法吃掉（對話輸入框同款做法），不會誤判成確認改名
  function renameForm(className: string) {
    const value = editingName?.value ?? "";
    return (
      <form
        className="table-title-form"
        onSubmit={(event) => {
          event.preventDefault();
          onRenameTable(value);
        }}
      >
        <input
          className={className}
          autoFocus
          value={value}
          aria-label={t("tableNameAria")}
          onChange={(event) => {
            const next = event.currentTarget.value;
            setEditingName((previous) => (previous ? { ...previous, value: next } : previous));
          }}
          onBlur={() => onRenameTable(value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") setEditingName(null);
          }}
        />
      </form>
    );
  }

  const targetName = gmTargeted ? "GM" : (characters.metaOf(speaker)?.name ?? speaker);
  const requestReplyLabel = t("requestReplyBtn", {
    name: speaker ? targetName : t("characterFallback"),
  });
  const generatingMeta = chat.generating !== null ? characters.metaOf(chat.generating.id) : undefined;

  return (
    <>
      <TableSidebar
        worlds={worlds}
        table={table}
        busy={chat.busy}
        renamingTable={editingName?.at === "list"}
        renameForm={renameForm}
        onStartRename={(name) => setEditingName({ at: "list", value: name })}
        onNewTable={() => void onNewTable()}
        onGenerateTable={() => void onGenerateTable()}
        onSwitchTable={(id) => void onSwitchTable(id)}
        onDeleteTable={(id) => void onDeleteTable(id)}
        gmId={GM_TARGET}
        selectedCard={selectedCard}
        speakingCard={mainView ? "" : speaker}
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
        onOpenSettings={() => onOpenSettings("appearance")}
      />

      <main className="chat-main">
        <WorkspaceHeader
          tableName={tableName}
          renaming={editingName?.at === "header"}
          renameForm={renameForm}
          onStartRename={(name) => setEditingName({ at: "header", value: name })}
          showCardInterface={mainView === null && cardInterface.shellReady}
          onOpenCardInterface={() => cardInterface.open()}
          busy={chat.busy}
          hasEvents={chat.events.length > 0}
          onAdvanceScene={advanceScene}
          onExportTranscript={exportTranscript}
          scene={scene}
          onToggleActs={() => setActsOpen((open) => !open)}
        />

        {mainView === null && (hasStateBar || Object.keys(tableState.tree).length > 0) && (
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
          actsOpen={actsOpen && scene > 0}
          scene={scene}
          onHideActs={() => setActsOpen(false)}
          onOpenScene={(n) => void openSceneReader(n)}
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
          sponsorUnlocked={sponsorUnlocked}
          onPreference={onPreference}
          onOpenAiSettings={() => onOpenSettings("ai")}
          worldOpen={mainView?.kind === "world"}
          worldEditorRefreshKey={worldEditorRefreshKey}
          onEntryConverted={onEntryConverted}
          onRefactorApplied={onRefactorApplied}
          playView={
            <PlayView
              onboarding={<Onboarding config={config} onSaved={onConfigSaved} />}
              sceneLabel={sceneDisplayLabel(scene)}
              events={chat.events}
              metaOf={characters.metaOf}
              generating={chat.generating}
              generatingMeta={generatingMeta}
              streamText={chat.streamText}
              busy={chat.busy}
              canRestore={chat.canRestore}
              onRestoreUndone={() => void chat.restoreUndone()}
              canUndoScene={canUndoScene}
              onRegenerateSummary={() => void regenerateSummary()}
              onRevertScene={() => void revertScene()}
              awayTooLong={chat.awayTooLong}
              speaker={speaker}
              gmTargeted={gmTargeted}
              targetName={targetName}
              targetColor={gmTargeted ? GM_COLOR : (characters.metaOf(speaker)?.color ?? "#888888")}
              targetImage={gmTargeted ? characters.gmImage : (characters.avatars[speaker] ?? null)}
              targetEmoji={characters.metaOf(speaker)?.avatar ?? "🎭"}
              onClearTarget={() => setSpeaker("")}
              input={chat.input}
              onInputChange={chat.setInput}
              castEmpty={characters.active.length === 0}
              onSubmit={chat.send}
              requestReplyLabel={requestReplyLabel}
              onUndoLast={() => void chat.undoLast()}
              onRequestReply={() => void chat.replyFromTarget()}
              onGmNarrate={chat.gmNarrate}
              onGmAdvance={chat.gmAdvance}
            />
          }
        />
        {error && <ErrorNote text={error} transport={transport} />}
      </main>

      {/* 卡片自帶介面整面取代對話；殼本身已含敘事畫面，不用再疊聊天記錄；且只在遊玩畫面出現——
          切去編輯畫面時 mainView 不再是 null，這裡直接不渲染，覆蓋層就跟著消失，不用另外清 cardUiOpen */}
      {mainView === null && cardInterface.uiOpen && cardInterface.shellReady && (
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
          onClose={() => cardInterface.close()}
        />
      )}
    </>
  );
}
