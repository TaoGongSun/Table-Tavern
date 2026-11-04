// 編卡畫面的角色圖（room-player-cards §2、§3.4）：上傳（先轉 PNG）、移除、AI 生圖（花 GM 主機的額度：常駐告知、同一房間第一次按先確認、
// 先預覽再按「用這張」或「丟掉」；主機關掉生圖時只留說明，主機沒有房間能用的生圖路線就整段不出現）。
// 上傳、移除、用這張、從圖庫選跟桌面版一樣只改草稿，按存檔才寫入、按返回就撤回（CardSheet）。
// 上傳、用這張、從圖庫選三條路都先過 2:3 裁切窗（room-phone-gallery §3.5）：上傳在手機切好再傳；圖庫圖只送裁切框，執行端拿原圖切。
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { errorText } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import type { Act } from "../room/useRoom";
import { CardGallery } from "./CardGallery";
import { cropTo, toPng, type CropBox } from "./card-upload";
import { CropSheet } from "./CropSheet";

const GEN_WAIT_MS = 11 * 60_000; // 伺服器最多等 10 分鐘
/** 這次開頁已確認過生圖額度的房間：同一房間只問第一次（重新整理後再問一次） */
const confirmedRooms = new Set<string>();

interface Props {
  view: RoomView;
  act: Act;
  cardId: string;
  /** 畫面上的圖（草稿優先） */
  image: string | null;
  /** 新圖或移除都只進草稿；null＝移除 */
  setDraft: (draft: ImageDraft) => void;
  /** 生圖用編卡畫面當下的草稿（跟桌面版一樣，沒存檔也能生） */
  draft: { name: string; description: string };
  /** false＝桌面版：沒有上傳 */
  canUpload: boolean;
  /** 圖庫張數與目前套用的是哪一張（讀卡時帶回） */
  gallery: { count: number; inUse: string | null };
  /** 套用裁切到草稿寫好前：編卡畫面整區停用（存檔、返回都不能按） */
  hold: (on: boolean) => void;
}

/** 待存的圖：要上傳的 PNG，或 GM 主機圖庫裡的那張（可帶裁切框） */
export type ImageDraft =
  | { png: Blob; preview: string }
  | { file: string; preview: string; crop?: CropBox }
  | null;

/** 草稿預覽：把預覽圖照同一個框切出來給玩家看；切不了回 null（用原預覽） */
async function cutPreview(src: string, box: CropBox) {
  const blob = await fetch(src)
    .then((r) => r.blob())
    .catch(() => null);
  return blob && (await cropTo(blob, box, 640));
}

export function CardImage({
  view,
  act,
  cardId,
  image,
  setDraft,
  draft,
  canUpload,
  gallery,
  hold,
}: Props) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"" | "convert" | "gen">("");
  const [preview, setPreview] = useState<{ file: string; preview: string } | null>(null);
  // picked：草稿裡選的圖庫圖（用這張或從圖庫選），等存檔
  const [picked, setPicked] = useState<string | null>(null);
  const [cropping, setCropping] = useState<{
    src: string;
    done: (box: CropBox) => Promise<void>;
    own?: boolean;
  } | null>(null);
  // 草稿預覽的 object URL：換掉、移除、關畫面時釋放
  const draftUrl = useRef<string | null>(null);
  useEffect(() => () => void (draftUrl.current && URL.revokeObjectURL(draftUrl.current)), []);
  function putDraft(next: ImageDraft, url: string | null = null) {
    if (draftUrl.current) URL.revokeObjectURL(draftUrl.current);
    draftUrl.current = url;
    setDraft(next);
  }
  function closeCrop() {
    if (cropping?.own) URL.revokeObjectURL(cropping.src);
    setCropping(null);
  }
  const [count, setCount] = useState(gallery.count);
  const [galleryOpen, setGalleryOpen] = useState(false);
  const [galleryKey, setGalleryKey] = useState(0);
  const [extra, setExtra] = useState("");
  const [framing, setFraming] = useState<"full" | "half">("half");
  const host = view.executor.label;
  const genOn = view.executor.imageGen;

  function pick(file: File | undefined) {
    if (!file) return;
    setCropping({
      src: URL.createObjectURL(file),
      own: true,
      done: async (box) => {
        setBusy("convert");
        const cut = await cropTo(file, box, 1024);
        const png = cut && (await toPng(cut));
        setBusy("");
        if (!png) return setError(errorText("E_UNSUPPORTED_FORMAT"));
        setError(null);
        const url = URL.createObjectURL(png);
        putDraft({ png, preview: url }, url);
        setPreview(null); // 換成上傳的圖：先前選用的圖庫圖不再是草稿
        setPicked(null);
      },
    });
  }

  /** 圖庫那張（剛生的或圖庫裡的）先裁切，確定才進草稿 */
  function choose(file: string, src: string) {
    setCropping({
      src,
      done: async (box) => {
        const cut = await cutPreview(src, box);
        const url = cut && URL.createObjectURL(cut);
        putDraft({ file, preview: url ?? src, crop: box }, url);
        setPreview(null);
        setPicked(file);
      },
    });
  }

  async function generate() {
    const room = view.room.code;
    if (!confirmedRooms.has(room) && !window.confirm(t("cardGenConfirm", { host }))) return;
    confirmedRooms.add(room);
    setBusy("gen");
    const payload = {
      cardId,
      name: draft.name.trim(),
      description: draft.description.slice(0, 3000),
      extraPrompt: extra,
      framing,
    };
    const reply = await act("image.gen", payload, GEN_WAIT_MS);
    setBusy("");
    // 主機關了開關：別說成「只有房主可以」
    if (!reply.ok)
      return setError(
        reply.code === "E_FORBIDDEN" ? t("cardGenOff") : errorText(reply.code, reply.retryAfterMs),
      );
    setError(null);
    setPreview(reply.result);
    setCount((n) => n + 1);
    setGalleryKey((k) => k + 1); // 圖庫開著就重撈，剛生的那張排第一
  }

  return (
    <div className="card-image" data-testid="card-image">
      <h3>{t("cardImage")}</h3>
      {image ? (
        <img src={image} alt="" data-testid="card-image-now" />
      ) : (
        <p className="muted">{t("cardImageNone")}</p>
      )}
      <div className="panel-actions">
        {canUpload && (
          <label className="button-like">
            {t("cardImageUpload")}
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp"
              hidden
              disabled={!!busy}
              onChange={(e) => {
                void pick(e.target.files?.[0]);
                e.currentTarget.value = ""; // 同一個檔可以再選一次
              }}
            />
          </label>
        )}
        {count > 0 && (
          <button
            type="button"
            className="ghost"
            disabled={!!busy}
            aria-expanded={galleryOpen}
            onClick={() => setGalleryOpen(!galleryOpen)}
          >
            {t("cardGalleryOpen", { n: count })}
          </button>
        )}
        {image && (
          <button
            type="button"
            className="ghost"
            disabled={!!busy}
            onClick={() => (putDraft(null), setPreview(null), setPicked(null))}
          >
            {t("cardImageRemove")}
          </button>
        )}
      </div>
      {picked && (
        <p className="muted" data-testid="card-image-chosen">
          {t("cardGenChosen")}
        </p>
      )}
      {galleryOpen && (
        <CardGallery
          key={galleryKey}
          act={act}
          inUse={gallery.inUse}
          picked={picked}
          onPick={choose}
        />
      )}
      {view.executor.imageGenRoute && (
        <>
          <h3>{t("cardGen")}</h3>
          <p className="muted" data-testid="card-gen-notice">
            {genOn ? t("cardGenNotice", { host }) : t("cardGenOff")}
          </p>
        </>
      )}
      {genOn && (
        <>
          <label>
            {t("cardGenExtra")}
            <input value={extra} maxLength={1000} onChange={(e) => setExtra(e.target.value)} />
          </label>
          <div className="panel-actions">
            <select
              value={framing}
              onChange={(e) => setFraming(e.target.value as "full" | "half")}
              aria-label={t("cardGenFraming")}
            >
              <option value="half">{t("cardFramingHalf")}</option>
              <option value="full">{t("cardFramingFull")}</option>
            </select>
            <button
              type="button"
              disabled={!!busy || !!preview || !draft.name.trim()}
              onClick={() => void generate()}
            >
              {busy === "gen" ? t("cardGenBusy") : t("cardGen")}
            </button>
          </div>
        </>
      )}
      {preview && (
        <div className="card-gen-preview" data-testid="card-gen-preview">
          <p className="muted">{t("cardGenNew")}</p>
          <img src={preview.preview} alt="" />
          <div className="panel-actions">
            <button type="button" onClick={() => choose(preview.file, preview.preview)}>
              {t("cardGenUse")}
            </button>
            {/* 丟掉只收起預覽：圖還在圖庫，之後從圖庫選撿得回來 */}
            <button type="button" className="ghost" onClick={() => setPreview(null)}>
              {t("cardGenDiscard")}
            </button>
          </div>
        </div>
      )}
      {cropping && (
        <CropSheet
          src={cropping.src}
          onCancel={closeCrop}
          // 草稿真的寫好才關窗；這段期間整個編卡畫面停用，不會讀到舊草稿存檔或先離開
          onConfirm={async (box) => {
            hold(true);
            try {
              await cropping.done(box);
            } finally {
              hold(false);
            }
            closeCrop();
          }}
        />
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
