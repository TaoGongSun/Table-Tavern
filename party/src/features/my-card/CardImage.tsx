// 編卡畫面的角色圖（room-player-cards §2、§3.4）：上傳（先轉 PNG）、移除、AI 生圖（花 GM 主機的額度：常駐告知、同一房間第一次按先確認、
// 先預覽再按「用這張」或「丟掉」；主機關掉生圖時只留說明，主機沒有房間能用的生圖路線就整段不出現）。
// 上傳、移除、用這張跟桌面版一樣只改草稿，按存檔才寫入、按返回就撤回（CardSheet）。
import { useState } from "react";
import { t } from "../../i18n";
import { errorText } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import type { Act } from "../room/useRoom";
import { toPng } from "./card-upload";

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
}

/** 待存的圖：要上傳的 PNG，或 GM 主機圖庫裡生好的那張 */
export type ImageDraft = { png: Blob; preview: string } | { file: string; preview: string } | null;

export function CardImage({ view, act, cardId, image, setDraft, draft, canUpload }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"" | "convert" | "gen">("");
  const [preview, setPreview] = useState<{ file: string; preview: string } | null>(null);
  const [extra, setExtra] = useState("");
  const [framing, setFraming] = useState<"full" | "half">("half");
  const host = view.executor.label;
  const genOn = view.executor.imageGen;

  async function pick(file: File | undefined) {
    if (!file) return;
    setBusy("convert");
    const png = await toPng(file);
    setBusy("");
    if (!png) return setError(errorText("E_UNSUPPORTED_FORMAT"));
    setError(null);
    setDraft({ png, preview: URL.createObjectURL(png) });
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

  return (
    <div className="card-image" data-testid="card-image">
      <h3>{t("cardImage")}</h3>
      {image ? <img src={image} alt="" data-testid="card-image-now" /> : <p className="muted">{t("cardImageNone")}</p>}
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
        {image && <button type="button" className="ghost" disabled={!!busy} onClick={() => setDraft(null)}>{t("cardImageRemove")}</button>}
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
            <button type="button" disabled={!!busy || !!preview || !draft.name.trim()} onClick={() => void generate()}>
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
            <button type="button" onClick={() => (setDraft(preview), setPreview(null))}>{t("cardGenUse")}</button>
            <button type="button" className="ghost" onClick={() => setPreview(null)}>{t("cardGenDiscard")}</button>
          </div>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
