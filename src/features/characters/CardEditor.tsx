import { FormEvent, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import { AppConfig } from "../../shared/contracts/backend-contracts";
import { CharacterCard, DraftImage, Tier } from "./card-model";
import { tierLabel } from "../ai-connection/model-catalog";
import { IconArchive, IconBook, IconDelete, IconExport, IconSparkle } from "../../shared/ui/icons";
import type { MoreMenuItem } from "../../shared/ui/MoreMenu";
import { ModalShell } from "../../shared/ui/Dialog";
import { AiImageDialog, CropDialog } from "./CardImageDialogs";
import { EditPage } from "../../shared/ui/EditPage";

// 角色圖示快捷選項；輸入框沒限制在這幾個，系統 emoji 鍵盤打什麼都行
const AVATAR_EMOJIS = ["🎭", "🧙", "🗡️", "🏹", "🛡️", "🐺", "🦊", "🐉", "👑", "💀", "🌙", "🕯️"];
const DEFAULT_AVATAR = "🎭";
const AVATAR_MAX_CHARS = 4;

// 以「看得到的字元」為單位截斷：input 的 maxLength 算的是 UTF-16 單元，
// 一顆 🗡️ 就佔 3 個，拿來限長會讓 emoji 只打得下一顆。
function clampChars(value: string, max: number) {
  const chars =
    typeof Intl.Segmenter === "function"
      ? Array.from(new Intl.Segmenter().segment(value), (unit) => unit.segment)
      : Array.from(value);
  return chars.slice(0, max).join("");
}

export function CardEditor({
  title,
  world,
  characterId,
  isNew,
  newCardColor,
  imageDataUrl,
  avatarImgUrl,
  onImagesChanged,
  onSaved,
  onArchived,
  onDeleted,
  onBack,
  leaveGuard,
  config,
  onPreference,
  onOpenAiSettings,
  isPlayer = false,
  onConverted,
  isBusy,
}: {
  /** 頂列標題：卡種與角色名由 MainView 決定 */
  title: string;
  world: string;
  /** 開編輯器前已由 new_id 拿好，草稿期生圖與存檔用同一個 id */
  characterId: string;
  /** true＝建新卡的空白草稿，尚未寫入過任何檔案 */
  isNew: boolean;
  /** 側欄要離開這張卡時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  newCardColor: string;
  imageDataUrl?: string;
  avatarImgUrl?: string;
  onImagesChanged: () => Promise<void>;
  onBack: () => void;
  onSaved: (id: string) => void;
  onArchived: () => Promise<void>;
  onDeleted: () => Promise<void>;
  config: AppConfig;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
  isPlayer?: boolean;
  onConverted: () => Promise<void>;
  /** 回合或換幕進行中：轉條目會刪卡，這段期間不轉 */
  isBusy: () => boolean;
}) {
  const [card, setCard] = useState<CharacterCard | null>(null);
  const [savedCardJson, setSavedCardJson] = useState("");
  // 圖像操作一律暫存，按儲存才落地（2026-07-27 使用者拍板）：
  // undefined＝沒動過（沿用 props 的已存檔圖）、null＝已標記移除、物件＝待存的新圖
  const [draftImage, setDraftImage] = useState<DraftImage | null | undefined>(undefined);
  const [draftAvatar, setDraftAvatar] = useState<DraftImage | null | undefined>(undefined);
  const [message, setMessage] = useState("");
  const [pendingImage, setPendingImage] = useState<string | null>(null);
  const [croppingAvatar, setCroppingAvatar] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const [aiGenOpen, setAiGenOpen] = useState(false);
  // 存檔前判斷「有沒有改名」用；新卡是空字串（第一次存檔不算改名）
  const [originalName, setOriginalName] = useState("");
  // 頂列儲存鈕在表單外，用 form 屬性指過來
  const formId = useId();

  useEffect(() => {
    setMessage("");
    setDraftImage(undefined);
    setDraftAvatar(undefined);
    if (isNew) {
      const blank: CharacterCard = {
        id: characterId,
        name: "",
        color: newCardColor,
        avatar: DEFAULT_AVATAR,
        tier: "balanced",
        show_image: true,
        archived: false,
        auto_hidden: false,
        public_md: "",
        private_md: "",
        gen_prompt: "",
      };
      setCard(blank);
      setSavedCardJson(JSON.stringify(blank));
      setOriginalName("");
      return;
    }
    // 切卡後才回來的舊請求一律丟掉：晚到的上一張卡會蓋掉已載入、可能已在編輯的這張
    let stale = false;
    invoke<CharacterCard>("read_character", { worldId: world, characterId })
      .then((loaded) => {
        if (stale) return;
        setCard(loaded);
        setSavedCardJson(JSON.stringify(loaded));
        setOriginalName(loaded.name);
      })
      .catch((reason) => {
        if (!stale) setMessage(String(reason));
      });
    return () => {
      stale = true;
    };
  }, [world, characterId, isNew, newCardColor]);

  // 已儲存之外的訊息都是擋下或失敗
  const messageIsError = message !== "" && message !== t("saved");

  // 切換編輯對象時 state 裡還是上一張卡（讀檔在 effect 裡非同步）：ID 對上才算載好，
  // 否則會短暫顯示舊卡、頂列儲存還會把舊卡寫進新 id。載好之前沒有可遺失的修改，離開不必問
  if (!card || card.id !== characterId) {
    leaveGuard.current = async () => true;
    return (
      <EditPage title={title} onBack={onBack} message={message} messageIsError={messageIsError} />
    );
  }

  const shownImage = draftImage === undefined ? imageDataUrl : draftImage?.url;
  const shownAvatar = draftAvatar === undefined ? avatarImgUrl : draftAvatar?.url;
  const aiGenBlocked = !card.name.trim() || !card.public_md.trim();
  const unsavedCount =
    (JSON.stringify(card) !== savedCardJson ? 1 : 0) +
    (draftImage !== undefined ? 1 : 0) +
    (draftAvatar !== undefined ? 1 : 0);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (!card) return;
    const target = card.name.trim();
    if (!target) {
      setMessage(t("nameRequiredError"));
      return;
    }
    // 圖示清空就回預設，免得沒圖也沒 emoji 的空白角色
    const saved: CharacterCard = {
      ...card,
      name: target,
      avatar: card.avatar.trim() || DEFAULT_AVATAR,
      private_md: isPlayer ? "" : card.private_md,
      tier: isPlayer ? "balanced" : card.tier,
    };
    // 改名只換之後的顯示名稱（id 定址不受影響），欄位下的說明太容易看漏，儲存前再提醒一次
    const renaming = !isNew && target !== originalName;
    if (
      renaming &&
      !(await confirm(t("renameConfirm", { from: originalName, to: target }), {
        title: t("renameConfirmTitle"),
        kind: "warning",
        okLabel: t("renameOk"),
        cancelLabel: t("dialogCancel"),
      }))
    ) {
      return;
    }
    try {
      await invoke("write_character", { worldId: world, card: saved });
      if (draftImage === null) await invoke("delete_character_image", { worldId: world, characterId });
      else if (draftImage) await invoke("save_character_image", { worldId: world, characterId, data: draftImage.bytes });
      if (draftAvatar === null) await invoke("delete_character_avatar", { worldId: world, characterId });
      else if (draftAvatar) await invoke("save_character_avatar", { worldId: world, characterId, data: draftAvatar.bytes });
      setDraftImage(undefined);
      setDraftAvatar(undefined);
      await onImagesChanged();
      setCard(saved);
      setSavedCardJson(JSON.stringify(saved));
      setOriginalName(target);
      setMessage(t("saved"));
      onSaved(characterId);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function confirmLeave() {
    if (unsavedCount === 0) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
      okLabel: t("unsavedLeaveOk"),
      cancelLabel: t("unsavedLeaveCancel"),
    });
  }
  // 側欄切換編輯對象時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  // 同一顆鈕雙向切換：隱藏區進來的卡按它就是還原，免得編輯器裡出現按了沒意義的「隱藏角色」
  async function toggleArchived() {
    setMessage("");
    try {
      await invoke("set_character_archived", {
        worldId: world,
        characterId,
        archived: card?.archived !== true,
      });
      await onArchived();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 匯出成 SillyTavern 角色卡：內容取自已存檔的那份，所以草稿沒存完先擋下
  async function exportCard() {
    setMessage("");
    if (!card) return;
    if (unsavedCount > 0) {
      setMessage(t("exportCardNeedsSave"));
      return;
    }
    try {
      const path = await saveDialog({
        defaultPath: `${card.name.trim() || "card"}.png`,
        filters: [
          { name: t("exportCardPng"), extensions: ["png"] },
          { name: t("exportCardJson"), extensions: ["json"] },
        ],
      });
      if (!path) return;
      await invoke("export_character", { worldId: world, characterId, path });
      await revealItemInDir(path);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function convertCardToWorldbookEntry() {
    setMessage("");
    if (!card) return;
    if (unsavedCount > 0) {
      await showMessage(t("convertCardUnsaved"), {
        title: t("convertCardToEntry"),
        okLabel: t("dialogAck"),
      });
      return;
    }
    if (isBusy()) {
      setMessage(t("worldBusy"));
      return;
    }
    const accepted = await confirm(t("convertCardConfirm"), {
      title: t("convertCardToEntry"),
      kind: "warning",
      okLabel: t("convertCardToEntry"),
      cancelLabel: t("dialogCancel"),
    });
    if (!accepted) return;
    // 確認框等人作答期間回合可能已經開跑，看當下不看舊閉包
    if (isBusy()) {
      setMessage(t("worldBusy"));
      return;
    }
    try {
      await invoke("character_to_worldbook_entry", { worldId: world, characterId });
      await showMessage(t("convertCardDone"), { okLabel: t("dialogAck") });
      await onConverted();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  function chooseImage(file: File) {
    const reader = new FileReader();
    reader.onload = () => setPendingImage(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => setMessage(String(reader.error));
    reader.readAsDataURL(file);
  }

  // 移除圖片／頭像都會讓卡片退回下一層顯示，先問一聲（2026-07-27 使用者回饋）
  async function removeImage() {
    const accepted = await confirm(t("removeImageConfirm"), {
      title: t("removeImageTitle"),
      kind: "warning",
      okLabel: t("dialogRemove"),
      cancelLabel: t("dialogCancel"),
    });
    if (accepted) setDraftImage(null);
  }

  async function removeAvatar() {
    const accepted = await confirm(t("removeAvatarConfirm"), {
      title: t("removeAvatarTitle"),
      kind: "warning",
      okLabel: t("dialogRemove"),
      cancelLabel: t("dialogCancel"),
    });
    if (accepted) setDraftAvatar(null);
  }

  // ⋯ 依卡種：既有角色卡四項、玩家卡兩項；新卡存檔前沒有可匯出／刪除的東西，整顆不出現
  const moreItems: MoreMenuItem[] = isNew
    ? []
    : [
        {
          key: "export",
          label: t("exportCard"),
          hint: t("exportCardHint"),
          icon: <IconExport />,
          onSelect: () => void exportCard(),
        },
        ...(isPlayer
          ? []
          : [
              {
                key: "convert",
                label: t("convertCardToEntry"),
                icon: <IconBook />,
                onSelect: () => void convertCardToWorldbookEntry(),
              },
              // 同一項雙向換字：隱藏區進來的卡按它就是還原
              {
                key: "archive",
                label: card.archived === true ? t("restoreCharacter") : t("archiveCharacter"),
                icon: <IconArchive />,
                onSelect: () => void toggleArchived(),
              },
            ]),
        // 確認框在 controller 裡，這裡不再包一層
        {
          key: "delete",
          label: t("deleteCharacter"),
          icon: <IconDelete />,
          danger: true,
          onSelect: () => void onDeleted(),
        },
      ];

  return (
    <EditPage
      title={title}
      onBack={() => void handleBack()}
      formId={formId}
      moreItems={moreItems}
      unsavedCount={unsavedCount}
      message={message}
      messageIsError={messageIsError}
    >
      <form id={formId} onSubmit={save} className="settings-form">
        {/* 頂部「圖＋名字」一塊：左圖、右欄名字與圖片操作；欄寬不夠時右欄自動折到圖下方。
            其餘打字欄位維持全寬在下方 */}
        <div className="card-editor-top">
          <div className="card-editor-avatar">
            {shownImage ? (
              <button
                type="button"
                className="card-editor-image-zoom"
                aria-label={t("viewImageLabel")}
                title={t("viewImageLabel")}
                onClick={() => setLightboxOpen(true)}
              >
                <img className="card-editor-image" src={shownImage} alt="" />
              </button>
            ) : shownAvatar ? (
              <img className="avatar-round card-editor-avatar-round" src={shownAvatar} alt="" />
            ) : (
              <span
                className="card-editor-avatar-emoji"
                style={{ ["--ring" as string]: card.color }}
              >
                {card.avatar}
              </span>
            )}
          </div>
          <div className="card-editor-identity">
            <label>
              {t(isPlayer ? "playerNameLabel" : "nameLabel")}
              <input
                value={card.name}
                placeholder={t(
                  isPlayer ? "playerNamePlaceholder" : "newCharacterPlaceholder",
                )}
                onChange={(e) => setCard({ ...card, name: e.currentTarget.value })}
              />
            </label>
            {/* 改名只換之後的顯示名稱，已送出的對話仍顯示舊名（2026-07-27 拍板） */}
            {!isNew && card.name.trim() !== originalName && (
              <p className="field-note" role="note">
                {t("renameNote")}
              </p>
            )}
            <div className="card-editor-media-actions">
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => document.getElementById(`character-image-${characterId}`)?.click()}
              >
                {t(shownImage ? "replaceImageBtn" : "addImageBtn")}
              </button>
              {/* 名字給圖庫資料夾用、公開設定進提示詞；欄位沒填就生不出像樣的圖，故先鎖住。
                  提示掛在外層 span：disabled 的按鈕不收滑鼠事件，title 掛上去不會浮出來 */}
              <span
                className="hint-wrap"
                data-hint={aiGenBlocked ? t("aiGenNeedsContent") : undefined}
              >
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={aiGenBlocked}
                  onClick={() => setAiGenOpen(true)}
                >
                  <IconSparkle />
                  {t("aiGenBtn")}
                </button>
              </span>
              {shownImage && (
                <>
                  <button type="button" className="btn btn-sm" onClick={() => void removeImage()}>
                    {t("removeImageBtn")}
                  </button>
                  <button
                    type="button"
                    className="btn btn-sm"
                    onClick={() => setCroppingAvatar(true)}
                  >
                    {t("makeAvatarBtn")}
                  </button>
                </>
              )}
              {shownAvatar && (
                <button type="button" className="btn btn-sm" onClick={() => void removeAvatar()}>
                  {t("removeAvatarBtn")}
                </button>
              )}
              <input
                id={`character-image-${characterId}`}
                type="file"
                accept="image/png,image/jpeg,image/webp"
                hidden
                onChange={(event) => {
                  const file = event.currentTarget.files?.[0];
                  event.currentTarget.value = "";
                  if (file) chooseImage(file);
                }}
              />
            </div>
          </div>
        </div>
        {/* emoji 只在沒有圖可顯示時才會用到：有頭像、或有大圖且開關開著，這一欄就沒意義（2026-07-28 使用者拍板） */}
        {!shownAvatar && !(shownImage && card.show_image) && (
          <label>
            {t("avatarEmojiLabel")}
            <div className="emoji-row">
              <input
                className="emoji-input"
                value={card.avatar}
                onChange={(e) =>
                  setCard({
                    ...card,
                    avatar: clampChars(e.currentTarget.value.replace(/\s/g, ""), AVATAR_MAX_CHARS),
                  })
                }
              />
              {AVATAR_EMOJIS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className="emoji-preset"
                  aria-pressed={card.avatar === emoji}
                  onClick={() => setCard({ ...card, avatar: emoji })}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </label>
        )}
        <label>
          {t(isPlayer ? "playerPublicLabel" : "publicLabel")}
          <textarea
            rows={4}
            value={card.public_md}
            onChange={(e) => setCard({ ...card, public_md: e.currentTarget.value })}
          />
        </label>
        {!isPlayer && (
          <label>
            {t("privateLabel")}
            <textarea
              rows={4}
              value={card.private_md}
              onChange={(e) => setCard({ ...card, private_md: e.currentTarget.value })}
            />
          </label>
        )}
        {shownImage && (
          <label className="inline">
            <input
              type="checkbox"
              checked={card.show_image}
              onChange={(e) => setCard({ ...card, show_image: e.currentTarget.checked })}
            />
            {t("showImageLabel")}
          </label>
        )}
        {!isPlayer && (
          <label>
            {t("tierLabel")}
            <select
              value={card.tier}
              onChange={(e) => setCard({ ...card, tier: e.currentTarget.value as Tier })}
            >
              {(["best", "balanced", "fast"] as const).map((tier) => (
                <option key={tier} value={tier}>
                  {tierLabel(tier)}
                </option>
              ))}
            </select>
          </label>
        )}
      </form>
      {pendingImage && (
        <CropDialog
          title={t("cropImageTitle")}
          src={pendingImage}
          aspect={2 / 3}
          cropShape="rect"
          onConfirm={async (image) => setDraftImage(image)}
          onCancel={() => setPendingImage(null)}
        />
      )}
      {aiGenOpen && (
        <AiImageDialog
          world={world}
          characterId={characterId}
          name={card.name.trim()}
          description={card.public_md}
          initialPrompt={card.gen_prompt ?? ""}
          config={config}
          onPreference={onPreference}
          onOpenAiSettings={onOpenAiSettings}
          onPromptUsed={(prompt) =>
            setCard((current) => (current ? { ...current, gen_prompt: prompt } : current))
          }
          onPick={(dataUrl) => {
            setAiGenOpen(false);
            setPendingImage(dataUrl);
          }}
          onClose={() => setAiGenOpen(false)}
        />
      )}
      {/* 大圖檢視：無框，點圖或遮罩都關 */}
      {lightboxOpen && shownImage && (
        <ModalShell
          className="lightbox"
          label={t("viewImageLabel")}
          onDismiss={() => setLightboxOpen(false)}
          backdrop
        >
          <div data-dialog-content="" tabIndex={-1}>
            <img
              className="lightbox-image"
              src={shownImage}
              alt=""
              onClick={() => setLightboxOpen(false)}
            />
          </div>
        </ModalShell>
      )}
      {croppingAvatar && shownImage && (
        <CropDialog
          title={t("cropAvatarTitle")}
          src={shownImage}
          aspect={1}
          cropShape="round"
          onConfirm={async (image) => setDraftAvatar(image)}
          onCancel={() => setCroppingAvatar(false)}
        />
      )}
    </EditPage>
  );
}
