// 編卡畫面的角色圖（room-player-cards §2、§3.4）：上傳（先轉 PNG）、移除、AI 生圖（花 GM 主機的額度：常駐告知、同一房間第一次按先確認、
// 先預覽再按「用這張」或「丟掉」；主機關掉生圖時只留說明，主機沒有房間能用的生圖路線就整段不出現）。三個動作都馬上存，不等表單存檔。
import { useState } from "react";
import { t } from "../../i18n";
import { errorText } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { toPng, type Upload } from "./card-upload";

const GEN_WAIT_MS = 11 * 60_000; // 伺服器最多等 10 分鐘
/** 這次開頁已確認過生圖額度的房間：同一房間只問第一次（重新整理後再問一次） */
const confirmedRooms = new Set<string>();

interface Props {
  view: RoomView;
  act: Act;
  cardId: string;
  image: string | null;
  setImage: (image: string | null) => void;
  /** 生圖用編卡畫面當下的草稿（跟桌面版一樣，沒存檔也能生） */
  draft: { name: string; description: string };
  upload: Upload | null;
}

export function CardImage({ view, act, cardId, image, setImage, draft, upload }: Props) {
  const { run, error, setError } = useRun(act);
  const [busy, setBusy] = useState<"" | "upload" | "gen" | "save">("");
  const [preview, setPreview] = useState<{ file: string; preview: string } | null>(null);
  const [extra, setExtra] = useState("");
  const [framing, setFraming] = useState<"full" | "half">("half");
  const host = view.executor.label;
  const genOn = view.executor.imageGen;

  async function pick(file: File | undefined) {
    if (!file || !upload) return;
    setBusy("upload");
    const png = await toPng(file);
    const r = png ? await upload("image", png) : { ok: false as const, code: "E_UNSUPPORTED_FORMAT" as const };
    setBusy("");
    if (!r.ok) return setError(errorText(r.code ?? "E_INTERNAL"));
    setError(null);
    setImage(URL.createObjectURL(png!));
  }

  async function generate() {
    const room = view.room.code;
    if (!confirmedRooms.has(room) && !window.confirm(t("cardGenConfirm", { host }))) return;
    confirmedRooms.add(room);
    setBusy("gen");
    const payload = { cardId, name: draft.name.trim(), description: draft.description.slice(0, 3000), extraPrompt: extra, framing };
    const reply = await act("image.gen", payload, GEN_WAIT_MS);
    setBusy("");
    // 主機關了開關：別說成「只有房主可以」
    if (!reply.ok) return setError(reply.code === "E_FORBIDDEN" ? t("cardGenOff") : errorText(reply.code, reply.retryAfterMs));
    setError(null);
    setPreview(reply.result);
  }

  async function save(patch: Record<string, unknown>, next: string | null) {
    setBusy("save");
    const ok = await run("card.save", { cardId, ...patch });
    setBusy("");
    if (!ok) return;
    setImage(next);
    setPreview(null);
  }

  return (
    <div className="card-image" data-testid="card-image">
      <h3>{t("cardImage")}</h3>
      {image ? <img src={image} alt="" data-testid="card-image-now" /> : <p className="muted">{t("cardImageNone")}</p>}
      <div className="panel-actions">
        {upload && (
          <label className="button-like">
            {busy === "upload" ? t("cardImageUploading") : t("cardImageUpload")}
            <input type="file" accept="image/png,image/jpeg,image/webp" hidden disabled={!!busy} onChange={(e) => void pick(e.target.files?.[0])} />
          </label>
        )}
        {image && <button type="button" className="ghost" disabled={!!busy} onClick={() => void save({ removeImage: true }, null)}>{t("cardImageRemove")}</button>}
      </div>
      {view.executor.imageGenRoute && (
        <>
          <h3>{t("cardGen")}</h3>
          <p className="muted" data-testid="card-gen-notice">{genOn ? t("cardGenNotice", { host }) : t("cardGenOff")}</p>
        </>
      )}
      {genOn && (
        <>
          <label>
            {t("cardGenExtra")}
            <input value={extra} maxLength={1000} onChange={(e) => setExtra(e.target.value)} />
          </label>
          <div className="panel-actions">
            <select value={framing} onChange={(e) => setFraming(e.target.value as "full" | "half")} aria-label={t("cardGenFraming")}>
              <option value="half">{t("cardFramingHalf")}</option>
              <option value="full">{t("cardFramingFull")}</option>
            </select>
            <button type="button" disabled={!!busy || !draft.name.trim()} onClick={() => void generate()}>
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
            <button type="button" disabled={!!busy} onClick={() => void save({ imageFromGallery: preview.file }, preview.preview)}>{t("cardGenUse")}</button>
            <button type="button" className="ghost" disabled={!!busy} onClick={() => setPreview(null)}>{t("cardGenDiscard")}</button>
          </div>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
