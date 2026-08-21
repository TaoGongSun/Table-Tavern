import { FormEvent, useEffect, useState } from "react";
import Cropper, { Area } from "react-easy-crop";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../i18n";
import { explainAiError } from "../ai-error";
import { KOFI_URL } from "../appearance";
import { AppConfig } from "../backend-contracts";
import { CharacterCard, DraftImage, Tier } from "../card-model";
import { CLI_LABELS, CliInfo, detectClis } from "../cli";
import { tierLabel } from "../model-catalog";

const GALLERY_PAGE_SIZE = 12;

// 角色圖示快捷選項；輸入框沒限制在這幾個，系統 emoji 鍵盤打什麼都行
const AVATAR_EMOJIS = ["🎭", "🧙", "🗡️", "🏹", "🛡️", "🐺", "🦊", "🐉", "👑", "💀", "🌙", "🕯️"];
const DEFAULT_AVATAR = "🎭";
const AVATAR_MAX_CHARS = 4;

// Claude Code CLI 只輸出文字，沒有生圖工具：選到它就直說，不要讓玩家等一輪才拿到失敗訊息
const NO_IMAGE_CLIS = ["claude"];

// 以「看得到的字元」為單位截斷：input 的 maxLength 算的是 UTF-16 單元，
// 一顆 🗡️ 就佔 3 個，拿來限長會讓 emoji 只打得下一顆。
function clampChars(value: string, max: number) {
  const chars =
    typeof Intl.Segmenter === "function"
      ? Array.from(new Intl.Segmenter().segment(value), (unit) => unit.segment)
      : Array.from(value);
  return chars.slice(0, max).join("");
}

function CropDialog({
  title,
  src,
  aspect,
  cropShape,
  onConfirm,
  onCancel,
}: {
  title: string;
  src: string;
  aspect: number;
  cropShape: "rect" | "round";
  onConfirm: (image: DraftImage) => Promise<void>;
  onCancel: () => void;
}) {
  const [crop, setCrop] = useState({ x: 0, y: 0 });
  const [zoom, setZoom] = useState(1);
  const [croppedAreaPixels, setCroppedAreaPixels] = useState<Area | null>(null);
  const [message, setMessage] = useState("");

  async function confirmCrop() {
    if (!croppedAreaPixels) return;
    setMessage("");
    try {
      const image = new Image();
      await new Promise<void>((resolve, reject) => {
        image.onload = () => resolve();
        image.onerror = () => reject(new Error("Unable to load image"));
        image.src = src;
      });
      const size = cropShape === "round" ? 256 : Math.min(Math.round(croppedAreaPixels.width), 1024);
      const height =
        cropShape === "round"
          ? 256
          : Math.max(1, Math.round((croppedAreaPixels.height / croppedAreaPixels.width) * size));
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Unable to create image canvas");
      // 頭像存正方形原樣，圓形與黑框由 CSS 畫（拍板規格），canvas 不做圓形裁切
      context.drawImage(
        image,
        croppedAreaPixels.x,
        croppedAreaPixels.y,
        croppedAreaPixels.width,
        croppedAreaPixels.height,
        0,
        0,
        size,
        height,
      );
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((result) => (result ? resolve(result) : reject(new Error("Unable to crop image"))), "image/png");
      });
      // bytes 給存檔用、url 給暫存預覽用（圖像按儲存才落地）
      await onConfirm({
        bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
        url: canvas.toDataURL("image/png"),
      });
      onCancel();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title} onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <strong>{title}</strong>
          <button type="button" className="modal-close" aria-label={t("closeBtn")} onClick={onCancel}>×</button>
        </div>
        <div className="crop-area">
          <Cropper
            image={src}
            crop={crop}
            zoom={zoom}
            aspect={aspect}
            cropShape={cropShape}
            onCropChange={setCrop}
            onZoomChange={setZoom}
            onCropComplete={(_, area) => setCroppedAreaPixels(area)}
          />
        </div>
        <label className="crop-zoom">
          {t("zoomLabel")}
          <input type="range" min={1} max={4} step={0.05} value={zoom} onChange={(event) => setZoom(Number(event.currentTarget.value))} />
        </label>
        <div className="row">
          <button type="button" onClick={() => void confirmCrop()}>{t("cropConfirm")}</button>
          <button type="button" onClick={onCancel}>{t("cropCancel")}</button>
          {message && <span role="alert">{message}</span>}
        </div>
      </div>
    </div>
  );
}

export function CardEditor({
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
  sponsorUnlocked,
  onPreference,
  onOpenAiSettings,
  isPlayer = false,
  onConverted,
}: {
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
  sponsorUnlocked: boolean;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
  isPlayer?: boolean;
  onConverted: () => Promise<void>;
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
  const [aiGenLockedOpen, setAiGenLockedOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiSource, setAiSource] = useState("api");
  const [aiFraming, setAiFraming] = useState("full");
  const [aiClis, setAiClis] = useState<CliInfo[]>([]);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [aiGenError, setAiGenError] = useState("");
  const [galleryFiles, setGalleryFiles] = useState<string[]>([]);
  const [galleryImages, setGalleryImages] = useState<Record<string, string>>({});
  const [galleryLoaded, setGalleryLoaded] = useState(0);
  // 存檔前判斷「有沒有改名」用；新卡是空字串（第一次存檔不算改名）
  const [originalName, setOriginalName] = useState("");

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
    invoke<CharacterCard>("read_character", { worldId: world, characterId })
      .then((loaded) => {
        setCard(loaded);
        setSavedCardJson(JSON.stringify(loaded));
        setOriginalName(loaded.name);
      })
      .catch((reason) => setMessage(String(reason)));
  }, [world, characterId, isNew, newCardColor]);

  const trialsUsed = Number(config.preferences["ai_image_trials_used"] ?? 0);
  const sourceOptions = ["api", ...aiClis.map((cli) => cli.id)];
  const sourceCannotGenerate = NO_IMAGE_CLIS.includes(aiSource);

  async function loadGalleryPage(files: string[], start: number) {
    const page = files.slice(start, start + GALLERY_PAGE_SIZE);
    const images = await Promise.all(page.map(async (file) => [file, await invoke<string>("read_gallery_image", { worldId: world, characterId, file })] as const));
    setGalleryImages((current) => ({ ...current, ...Object.fromEntries(images) }));
    setGalleryLoaded(Math.min(start + page.length, files.length));
  }

  async function refreshGallery() {
    const files = await invoke<string[]>("list_gallery_images", { worldId: world, characterId });
    setGalleryFiles(files);
    setGalleryImages({});
    setGalleryLoaded(0);
    await loadGalleryPage(files, 0);
  }

  function openAiGenerator() {
    if (!sponsorUnlocked && trialsUsed >= 3) {
      setAiGenLockedOpen(true);
      return;
    }
    const savedSource = String(config.preferences["image_source"] ?? "");
    // 聊天用的來源不一定會生圖（例如 claude），跟隨不到就退回 API，玩家一打開就是能按的狀態
    const transport = String(config.preferences["transport"] ?? "api");
    const fallback = NO_IMAGE_CLIS.includes(transport) ? "api" : transport;
    void detectClis()
      .then((detected) => {
        setAiClis(detected);
        const detectedSources = ["api", ...detected.map((cli) => cli.id)];
        setAiSource(detectedSources.includes(savedSource) ? savedSource : fallback);
      })
      .catch(() => {
        setAiClis([]);
        setAiSource(savedSource === "api" ? savedSource : fallback);
      });
    setAiPrompt(card?.gen_prompt ?? "");
    setAiFraming(config.preferences["image_framing"] === "half" ? "half" : "full");
    setAiGenError("");
    setAiGenOpen(true);
    void refreshGallery().catch(() => {
      setGalleryFiles([]);
      setGalleryImages({});
      setGalleryLoaded(0);
    });
  }

  async function generateImage() {
    setAiGenerating(true);
    setAiGenError("");
    try {
      await invoke<string>("generate_character_image", {
        worldId: world,
        characterId,
        name: card?.name.trim() ?? "",
        description: card?.public_md ?? "",
        extraPrompt: aiPrompt,
        source: aiSource,
        framing: aiFraming,
      });
      // 追加描寫記進草稿，跟其他欄位一起等按儲存才落地
      setCard((current) => (current ? { ...current, gen_prompt: aiPrompt } : current));
      await refreshGallery();
      await onPreference("image_source", aiSource);
      await onPreference("image_framing", aiFraming);
      if (!sponsorUnlocked) await onPreference("ai_image_trials_used", trialsUsed + 1);
    } catch (reason) {
      setAiGenError(String(reason));
    } finally {
      setAiGenerating(false);
    }
  }

  async function deleteGalleryImage(file: string) {
    const accepted = await confirm(t("aiGalleryDeleteConfirm"), { title: t("aiGalleryDeleteTitle"), kind: "warning" });
    if (!accepted) return;
    await invoke("delete_gallery_image", { worldId: world, characterId, file });
    setGalleryFiles((current) => current.filter((item) => item !== file));
    setGalleryImages((current) => {
      const { [file]: _, ...remaining } = current;
      return remaining;
    });
    setGalleryLoaded((current) => Math.max(0, current - (galleryImages[file] ? 1 : 0)));
  }

  if (!card) return message ? <p role="alert">{message}</p> : null;

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
      await showMessage(t("convertCardUnsaved"), { title: t("convertCardToEntry") });
      return;
    }
    if (card.archived === false) {
      await showMessage(t("convertCardInUse"), { title: t("convertCardToEntry") });
      return;
    }
    const accepted = await confirm(t("convertCardConfirm"), {
      title: t("convertCardToEntry"),
      kind: "warning",
    });
    if (!accepted) return;
    try {
      await invoke("character_to_worldbook_entry", { worldId: world, characterId });
      await showMessage(t("convertCardDone"));
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
    });
    if (accepted) setDraftImage(null);
  }

  async function removeAvatar() {
    const accepted = await confirm(t("removeAvatarConfirm"), {
      title: t("removeAvatarTitle"),
      kind: "warning",
    });
    if (accepted) setDraftAvatar(null);
  }

  return (
    <form onSubmit={save} className="settings-form">
      {/* 頂部切兩塊：左邊是這張卡的動作（返回獨立成第二列貼齊儲存下方，按鈕變多後夾在刪除
          旁邊很難找），右邊是圖片與它的操作鈕；打字欄位維持全寬在下方（2026-07-28 使用者拍板） */}
      <div className="card-editor-top">
        <div className="card-editor-actions">
          <div className="row">
            <button type="submit">{t("saveCard")}</button>
            {!isNew && (
              <>
                <button type="button" title={t("exportCardHint")} onClick={() => void exportCard()}>
                  {t("exportCard")}
                </button>
                {!isPlayer && (
                  <button type="button" onClick={() => void convertCardToWorldbookEntry()}>
                    {t("convertCardToEntry")}
                  </button>
                )}
                {!isPlayer && (
                  <button
                    type="button"
                    className="archive-button"
                    onClick={() => void toggleArchived()}
                  >
                    {card?.archived === true ? t("restoreCharacter") : t("archiveCharacter")}
                  </button>
                )}
                <button type="button" className="delete-character" onClick={() => void onDeleted()}>
                  {t("deleteCharacter")}
                </button>
              </>
            )}
          </div>
          <div className="row">
            <button type="button" onClick={() => void handleBack()}>
              {t("backToNow")}
            </button>
          </div>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <div className="card-editor-media">
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
              <span className="card-editor-avatar-emoji" style={{ ["--ring" as string]: card.color }}>
                {card.avatar}
              </span>
            )}
          </div>
          <div className="row">
            <button type="button" onClick={() => document.getElementById(`character-image-${characterId}`)?.click()}>
              {t(shownImage ? "replaceImageBtn" : "addImageBtn")}
            </button>
            {/* 名字給圖庫資料夾用、公開設定進提示詞；欄位沒填就生不出像樣的圖，故先鎖住。
                提示掛在外層 span：disabled 的按鈕不收滑鼠事件，title 掛上去不會浮出來 */}
            <span className="hint-wrap" data-hint={aiGenBlocked ? t("aiGenNeedsContent") : undefined}>
              <button
                type="button"
                className="ai-gen-btn"
                disabled={aiGenBlocked}
                onClick={openAiGenerator}
              >
                ✨ {t("aiGenBtn")}
              </button>
            </span>
            {shownImage && (
              <>
                <button type="button" onClick={() => void removeImage()}>{t("removeImageBtn")}</button>
                <button type="button" onClick={() => setCroppingAvatar(true)}>{t("makeAvatarBtn")}</button>
              </>
            )}
            {shownAvatar && <button type="button" onClick={() => void removeAvatar()}>{t("removeAvatarBtn")}</button>}
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
      <label>
        {t(isPlayer ? "playerNameLabel" : "nameLabel")}
        <input
          value={card.name}
          placeholder={t(isPlayer ? "playerNamePlaceholder" : "newCharacterPlaceholder")}
          onChange={(e) => setCard({ ...card, name: e.currentTarget.value })}
        />
      </label>
      {/* 改名只換之後的顯示名稱，已送出的對話仍顯示舊名（2026-07-27 拍板） */}
      {!isNew && card.name.trim() !== originalName && (
        <p className="field-note" role="note">
          {t("renameNote")}
        </p>
      )}
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
        <div className="modal-overlay" onClick={() => !aiGenerating && setAiGenOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenTitle")} onClick={(event) => event.stopPropagation()}>
            <h2>{t("aiGenTitle")}</h2>
            <label>{t("aiGenPromptLabel")}<textarea rows={3} value={aiPrompt} placeholder={t("aiGenPromptPlaceholder")} onChange={(event) => setAiPrompt(event.currentTarget.value)} /></label>
            <fieldset className="ai-gen-framing">
              <legend>{t("aiGenFramingLabel")}</legend>
              {(["full", "half"] as const).map((framing) => (
                <label key={framing}>
                  <input
                    type="radio"
                    name="ai-gen-framing"
                    checked={aiFraming === framing}
                    disabled={aiGenerating}
                    onChange={() => setAiFraming(framing)}
                  />
                  {framing === "full" ? t("aiGenFramingFull") : t("aiGenFramingHalf")}
                </label>
              ))}
            </fieldset>
            <label>{t("aiGenSourceLabel")}
              <div className="row">
                <select value={aiSource} onChange={(event) => setAiSource(event.currentTarget.value)} disabled={aiGenerating}>
                  {sourceOptions.map((source) => <option key={source} value={source}>{source === "api" ? t("aiGenSourceApi") : CLI_LABELS[source] ?? source}</option>)}
                  {!sourceOptions.includes(aiSource) && <option value={aiSource}>{CLI_LABELS[aiSource] ?? aiSource}</option>}
                </select>
                <button type="button" disabled={aiGenerating} onClick={onOpenAiSettings}>⚙ {t("aiTab")}</button>
              </div>
            </label>
            {sourceCannotGenerate && <div className="ai-gen-error" role="alert">{t("aiGenSourceNoImage", { provider: CLI_LABELS[aiSource] ?? aiSource })}</div>}
            {/* 生圖來源可以不經設定頁直接換，這裡也要講一次等一下的系統詢問是誰在問 */}
            {aiSource !== "api" && !sourceCannotGenerate && (
              <p className="cli-permission-note" role="note">
                {t("cliPermissionNote", { provider: CLI_LABELS[aiSource] ?? aiSource })}
              </p>
            )}
            {!sponsorUnlocked && <p role="note">{t("aiGenTrialNote", { n: Math.max(0, 3 - trialsUsed) })}</p>}
            {aiGenError && <div className="ai-gen-error" role="alert"><div>{t(explainAiError(aiGenError, aiSource) ?? "aiGenFailed")}</div><small>{aiGenError}</small></div>}
            {galleryFiles.length > 0 && (
              <section aria-label={t("aiGalleryTitle")}>
                <h3>{t("aiGalleryTitle")}</h3>
                <div className="ai-gallery">
                  {galleryFiles.slice(0, galleryLoaded).map((file) => galleryImages[file] && (
                    <div className="ai-gallery-thumb" key={file}>
                      <button
                        type="button"
                        className="ai-gallery-pick"
                        title={t("aiGalleryPick")}
                        onClick={() => { setAiGenOpen(false); setPendingImage(galleryImages[file]); }}
                      >
                        <img src={galleryImages[file]} alt="" />
                      </button>
                      <button
                        type="button"
                        className="ai-gallery-delete"
                        aria-label={t("aiGalleryDeleteTitle")}
                        onClick={() => void deleteGalleryImage(file).catch((reason) => setAiGenError(String(reason)))}
                      >×</button>
                    </div>
                  ))}
                </div>
                {galleryFiles.length > galleryLoaded && <button type="button" onClick={() => void loadGalleryPage(galleryFiles, galleryLoaded)}>{t("aiGalleryLoadMore", { n: galleryFiles.length - galleryLoaded })}</button>}
              </section>
            )}
            {/* 主要動作放右下（2026-07-27 使用者拍板：此對話框例外，不置頂） */}
            <div className="ai-gen-footer">
              <button type="button" disabled={aiGenerating} onClick={() => setAiGenOpen(false)}>{t("cropCancel")}</button>
              <button type="button" className="ai-gen-submit" disabled={aiGenerating || sourceCannotGenerate} onClick={() => void generateImage()}>
                {aiGenerating ? t("aiGenerating") : `✨ ${t("aiGenBtn")}`}
              </button>
            </div>
          </div>
        </div>
      )}
      {aiGenLockedOpen && (
        <div className="modal-overlay" onClick={() => setAiGenLockedOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenLockedTitle")} onClick={(event) => event.stopPropagation()}>
            <div className="row"><button type="button" onClick={() => void openUrl(KOFI_URL)}>{t("sponsorBtn")}</button><button type="button" onClick={() => setAiGenLockedOpen(false)}>{t("closeBtn")}</button></div>
            <h2>{t("aiGenLockedTitle")}</h2><p>{t("aiGenLockedBody")}</p>
          </div>
        </div>
      )}
      {lightboxOpen && shownImage && (
        <div
          className="modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-label={t("viewImageLabel")}
          onClick={() => setLightboxOpen(false)}
        >
          <img className="lightbox-image" src={shownImage} alt="" />
        </div>
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
    </form>
  );
}
