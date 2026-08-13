// 主欄下半部的畫面切換：單幕閱讀／角色卡編輯／世界設定／遊玩畫面四選一，外加前幕清單浮層。
// mainView 的所有權留在 App，這裡只依 App 判定好的值分派——四種卡片畫面的儲存、封存、
// 刪除接線各不相同（新卡存完要留在編輯器、玩家卡走另一支善後、只有既有角色卡能真刪），
// 原本的三元式逐條照抄過來，不合併。
import { ReactNode } from "react";
import { t } from "../i18n";
import { AppConfig } from "../backend-contracts";
import { ActReader, EditPane } from "./atoms";
import { CardEditor } from "./CardEditor";
import { WorldEditor } from "./WorldEditor";

interface MainViewProps {
  /** 前幕浮層開著（「至少換過一幕」的條件由 App 併進來） */
  actsOpen: boolean;
  /** 目前第幾幕：浮層列出 0 到 scene-1 */
  scene: number;
  onHideActs: () => void;
  onOpenScene: (n: number) => void;
  sceneLabelOf: (n: number) => string;
  world: string;
  worldName: string;
  /** 正在讀第幾幕；null＝不在單幕閱讀 */
  sceneReading: number | null;
  onFork: (n: number) => void;
  /** 正在編哪種卡；null＝不在卡片編輯 */
  cardKind: "character" | "new-character" | "player" | "new-player" | null;
  cardId: string;
  /** 編輯既有角色卡時標題要用的角色名 */
  cardName: string;
  editingPlayerCard: boolean;
  /** 新卡與世界書轉出的卡共用的下一個陣營色 */
  nextColor: string;
  cardImage: string | undefined;
  cardAvatar: string | undefined;
  onImagesChanged: () => Promise<void>;
  onCardSaved: (id: string) => Promise<void>;
  onPlayerCardSaved: (id: string) => Promise<void>;
  /** 隱藏或轉出後的善後：名單重載、發言對象改人、關掉編輯面板 */
  onFinishRemoval: (id: string) => Promise<void>;
  onDeleteCharacter: (id: string) => Promise<void>;
  onDeletePlayerCard: (id: string) => Promise<void>;
  /** 關掉編輯畫面回到遊玩 */
  onClose: () => void;
  leaveGuard: { current: (() => Promise<boolean>) | null };
  config: AppConfig;
  sponsorUnlocked: boolean;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
  worldOpen: boolean;
  /** 復原匯入改動了世界書：換這把 key 讓整支編輯器重新掛載重載 */
  worldEditorRefreshKey: number;
  onEntryConverted: () => Promise<void>;
  onRefactorApplied: () => Promise<void>;
  /** 遊玩畫面（messages＋composer）；元素在 App 建好傳進來 */
  playView: ReactNode;
}

export function MainView({
  actsOpen,
  scene,
  onHideActs,
  onOpenScene,
  sceneLabelOf,
  world,
  worldName,
  sceneReading,
  onFork,
  cardKind,
  cardId,
  cardName,
  editingPlayerCard,
  nextColor,
  cardImage,
  cardAvatar,
  onImagesChanged,
  onCardSaved,
  onPlayerCardSaved,
  onFinishRemoval,
  onDeleteCharacter,
  onDeletePlayerCard,
  onClose,
  leaveGuard,
  config,
  sponsorUnlocked,
  onPreference,
  onOpenAiSettings,
  worldOpen,
  worldEditorRefreshKey,
  onEntryConverted,
  onRefactorApplied,
  playView,
}: MainViewProps) {
  return (
    <div className="chat-body">
      {actsOpen && (
        <div className="acts-flyout">
          <button type="button" className="acts-flyout-hide" onClick={onHideActs}>
            {t("hideActs")}
          </button>
          <div className="acts-flyout-list">
            {Array.from({ length: scene }, (_, n) => n).map((n) => (
              <button key={n} type="button" onClick={() => onOpenScene(n)}>
                {sceneLabelOf(n)}
              </button>
            ))}
          </div>
        </div>
      )}
      {sceneReading !== null ? (
        <ActReader
          world={world}
          worldName={worldName}
          scene={sceneReading}
          label={sceneLabelOf(sceneReading)}
          onBack={onClose}
          onFork={() => void onFork(sceneReading)}
        />
      ) : cardKind !== null ? (
        <EditPane
          title={
            cardKind === "new-character"
              ? t("newCardTitle")
              : cardKind === "new-player"
                ? t("newPlayerCardTitle")
                : cardKind === "player"
                  ? t("editPlayerCardTitle")
                  : t("editCardSummary", { name: cardName })
          }
        >
          <CardEditor
            world={world}
            characterId={cardId}
            isNew={cardKind === "new-character" || cardKind === "new-player"}
            isPlayer={editingPlayerCard}
            newCardColor={nextColor}
            imageDataUrl={cardImage}
            avatarImgUrl={cardAvatar}
            onImagesChanged={onImagesChanged}
            onSaved={(saved) =>
              void (editingPlayerCard ? onPlayerCardSaved(saved) : onCardSaved(saved))
            }
            onArchived={
              cardKind === "character" ? () => onFinishRemoval(cardId) : async () => onClose()
            }
            onDeleted={
              cardKind === "character"
                ? () => onDeleteCharacter(cardId)
                : cardKind === "player"
                  ? () => onDeletePlayerCard(cardId)
                  : async () => onClose()
            }
            onBack={onClose}
            leaveGuard={leaveGuard}
            config={config}
            sponsorUnlocked={sponsorUnlocked}
            onPreference={onPreference}
            onOpenAiSettings={onOpenAiSettings}
            onConverted={() => onFinishRemoval(cardId)}
          />
        </EditPane>
      ) : worldOpen ? (
        <EditPane title={t("worldSummary")}>
          <WorldEditor
            key={worldEditorRefreshKey}
            world={world}
            worldName={worldName}
            onBack={onClose}
            leaveGuard={leaveGuard}
            convertColor={nextColor}
            onEntryConverted={onEntryConverted}
            onRefactorApplied={onRefactorApplied}
          />
        </EditPane>
      ) : (
        playView
      )}
    </div>
  );
}
