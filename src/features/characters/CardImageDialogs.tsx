// 角色卡編輯頁的圖片對話窗：裁切（大圖 2:3／頭像圓形）與 AI 生圖＋圖庫。
// 兩者都只產出暫存圖交回編輯頁，按儲存才落地。
import { useEffect, useState } from "react";
import Cropper, { Area } from "react-easy-crop";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import { explainAiError } from "../../shared/ui/ai-error";
import { Dialog, SwapLabel } from "../../shared/ui/Dialog";
import { AppConfig } from "../../shared/contracts/backend-contracts";
import { DraftImage } from "./card-model";
import { CLI_LABELS, CliInfo, detectClis } from "../ai-connection/cli";

const GALLERY_PAGE_SIZE = 12;

// Claude Code CLI 只輸出文字，沒有生圖工具：生圖來源不列它
const NO_IMAGE_CLIS = ["claude"];

export function CropDialog({
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
        image.onerror = () => reject(new Error(t("imageLoadFailed")));
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
      if (!context) throw new Error(t("imageCropFailed"));
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
        canvas.toBlob((result) => (result ? resolve(result) : reject(new Error(t("imageCropFailed")))), "image/png");
      });
      // bytes 給存檔用、url 給暫存預覽用（圖像按儲存才落地）
      await onConfirm({
        bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
        url: canvas.toDataURL("image/png"),
      });
      onCancel();
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <Dialog
      title={title}
      onDismiss={onCancel}
      backdrop
      closeButton
      start={
        <button type="button" className="btn" onClick={onCancel}>
          {t("cropCancel")}
        </button>
      }
      end={
        <button type="button" className="btn btn-primary" onClick={() => void confirmCrop()}>
          {t("cropConfirm")}
        </button>
      }
    >
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
      {message && <p role="alert">{backendText(message)}</p>}
    </Dialog>
  );
}

/** AI 生圖：開窗時偵測可用來源並讀圖庫；生成中不能關。圖庫挑的圖交回編輯頁進裁切。 */
export function AiImageDialog({
  world,
  characterId,
  name,
  description,
  initialPrompt,
  config,
  onPreference,
  onOpenAiSettings,
  onPromptUsed,
  onPick,
  onClose,
}: {
  world: string;
  characterId: string;
  name: string;
  description: string;
  /** 上次存在卡上的追加描寫 */
  initialPrompt: string;
  config: AppConfig;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
  /** 生成成功：追加描寫記進編輯頁草稿，跟其他欄位一起等按儲存才落地 */
  onPromptUsed: (prompt: string) => void;
  /** 從圖庫挑了一張：關窗並交給裁切 */
  onPick: (dataUrl: string) => void;
  onClose: () => void;
}) {
  const savedSource = String(config.preferences["image_source"] ?? "");
  // 聊天用的來源不一定會生圖（例如 claude），跟隨不到就退回 API，玩家一打開就是能按的狀態
  const transport = String(config.preferences["transport"] ?? "api");
  const fallback = NO_IMAGE_CLIS.includes(transport) ? "api" : transport;
  const [aiPrompt, setAiPrompt] = useState(initialPrompt);
  const [aiSource, setAiSource] = useState("api");
  const [aiFraming, setAiFraming] = useState(
    config.preferences["image_framing"] === "half" ? "half" : "full",
  );
  const [aiClis, setAiClis] = useState<CliInfo[]>([]);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [aiGenError, setAiGenError] = useState("");
  const [galleryFiles, setGalleryFiles] = useState<string[]>([]);
  const [galleryImages, setGalleryImages] = useState<Record<string, string>>({});
  const [galleryLoaded, setGalleryLoaded] = useState(0);

  const sourceOptions = ["api", ...aiClis.map((cli) => cli.id)];

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

  useEffect(() => {
    void detectClis()
      .then((detected) => {
        const imageClis = detected.filter((cli) => !NO_IMAGE_CLIS.includes(cli.id));
        setAiClis(imageClis);
        const detectedSources = ["api", ...imageClis.map((cli) => cli.id)];
        setAiSource(detectedSources.includes(savedSource) ? savedSource : fallback);
      })
      .catch(() => {
        setAiClis([]);
        setAiSource(savedSource === "api" ? savedSource : fallback);
      });
    void refreshGallery().catch(() => {
      setGalleryFiles([]);
      setGalleryImages({});
      setGalleryLoaded(0);
    });
    // 只在開窗時跑一次，之後換來源、生成都不重新偵測
  }, []);

  async function generateImage() {
    setAiGenerating(true);
    setAiGenError("");
    try {
      await invoke<string>("generate_character_image", {
        worldId: world,
        characterId,
        name,
        description,
        extraPrompt: aiPrompt,
        source: aiSource,
        framing: aiFraming,
      });
      onPromptUsed(aiPrompt);
      await refreshGallery();
      await onPreference("image_source", aiSource);
      await onPreference("image_framing", aiFraming);
    } catch (reason) {
      setAiGenError(String(reason));
    } finally {
      setAiGenerating(false);
    }
  }

  async function deleteGalleryImage(file: string) {
    const accepted = await confirm(t("aiGalleryDeleteConfirm"), {
      title: t("aiGalleryDeleteTitle"),
      kind: "warning",
      okLabel: t("dialogDelete"),
      cancelLabel: t("dialogCancel"),
    });
    if (!accepted) return;
    await invoke("delete_gallery_image", { worldId: world, characterId, file });
    setGalleryFiles((current) => current.filter((item) => item !== file));
    setGalleryImages((current) => {
      const { [file]: _, ...remaining } = current;
      return remaining;
    });
    setGalleryLoaded((current) => Math.max(0, current - (galleryImages[file] ? 1 : 0)));
  }

  return (
    <Dialog
      title={t("aiGenTitle")}
      onDismiss={aiGenerating ? undefined : onClose}
      backdrop
      closeButton
      start={
        <button type="button" className="btn" disabled={aiGenerating} onClick={onClose}>
          {t("cropCancel")}
        </button>
      }
      end={
        <button
          type="button"
          className="btn btn-primary"
          disabled={aiGenerating}
          onClick={() => void generateImage()}
        >
          <SwapLabel
            labels={[`✨ ${t("aiGenBtn")}`, t("aiGenerating")]}
            current={aiGenerating ? 1 : 0}
          />
        </button>
      }
    >
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
      {/* 生圖來源可以不經設定頁直接換，這裡也要講一次等一下的系統詢問是誰在問 */}
      {aiSource !== "api" && (
        <p className="cli-permission-note" role="note">
          {t("cliPermissionNote", { provider: CLI_LABELS[aiSource] ?? aiSource })}
        </p>
      )}
      {aiGenError && <div className="ai-gen-error" role="alert"><div>{t(explainAiError(aiGenError, aiSource) ?? "aiGenFailed")}</div><small>{backendText(aiGenError)}</small></div>}
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
                  onClick={() => onPick(galleryImages[file])}
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
    </Dialog>
  );
}
