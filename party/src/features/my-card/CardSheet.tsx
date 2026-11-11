// 編卡畫面（room-player-cards §2、§3.2）：建立與編輯共用。欄位照桌面版玩家卡：角色圖（只有編輯時）、名字、圖示、公開描述、秘密情報。
// 圖示、秘密情報、放不進的公開描述不經 seat.create：建立時先入座，再接著 card.save；這步失敗畫面不關、原文保留可重試。
// 圖示先在這裡驗，免得已入座才被伺服器擋。
import { useEffect, useState } from "react";
import { cleanIcon, cleanText } from "../../../server/core/validate";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { errorText } from "../room/error-text";
import { CardImage, type ImageDraft } from "./CardImage";
import type { Upload } from "./card-upload";

interface Fields {
  name: string;
  avatar: string;
  publicMd: string;
  privateMd: string;
}
const EMPTY: Fields = { name: "", avatar: "", publicMd: "", privateMd: "" };

interface Props {
  view: RoomView;
  act: Act;
  mode: "create" | "edit";
  upload: Upload | null;
  onClose: () => void;
}

export function CardSheet({ view, act, mode, upload, onClose }: Props) {
  const { run, error, setError } = useRun(act);
  const [saved, setSaved] = useState<Fields | null>(
    mode === "create" ? { ...EMPTY, name: view.me.nickname } : null,
  );
  const [f, setF] = useState<Fields>(saved ?? EMPTY);
  const [image, setImage] = useState<string | null>(null);
  const [gallery, setGallery] = useState({ count: 0, inUse: null as string | null });
  // 圖一律暫存，按存檔才寫入、按返回就撤回（同桌面版 CardEditor）；undefined＝沒動過
  const [draftImage, setDraftImage] = useState<ImageDraft | undefined>(undefined);
  const [cardId, setCardId] = useState<string | null>(mode === "edit" ? view.me.cardId : null);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState(false); // 已建卡入座、其餘欄位還沒存上
  const [leaving, setLeaving] = useState(false); // 有未存修改時按返回：先確認

  useEffect(() => {
    if (mode !== "edit") return;
    void act("card.mine").then((reply) => {
      if (!reply.ok)
        return setError(reply.code === "E_TEXT_INVALID" ? t("cardTooLong") : t("cardLoadFailed"));
      const c = reply.result.card;
      const loaded = {
        name: c.name,
        avatar: c.avatar,
        publicMd: c.publicMd,
        privateMd: c.privateMd,
      };
      setSaved(loaded);
      setF(loaded);
      setImage(c.image);
      setGallery({ count: c.gallery ?? 0, inUse: c.galleryFile ?? null });
      setCardId(c.cardId);
    });
  }, [mode, act, setError]);

  const set = (k: keyof Fields) => (e: { target: { value: string } }) =>
    setF({ ...f, [k]: e.target.value });

  /** 只送改過的欄位；圖示空白不送（伺服器要 1–16 字元） */
  function changed(base: Fields): Record<string, string> {
    const out: Record<string, string> = {};
    if (f.name.trim() !== base.name) out.name = f.name.trim();
    if (f.avatar.trim() && f.avatar.trim() !== base.avatar) out.avatar = f.avatar.trim();
    if (f.publicMd !== base.publicMd) out.publicMd = f.publicMd;
    if (f.privateMd !== base.privateMd) out.privateMd = f.privateMd;
    return out;
  }

  async function saveTo(id: string, fields: Record<string, string>) {
    const d = draftImage;
    const patch: Record<string, unknown> = { ...fields };
    if (d && "png" in d) {
      // 新圖先上傳（執行端換好圖才回），成功就算已存，之後存欄位失敗重試也不用再傳
      const r = upload
        ? await upload("image", d.png)
        : { ok: false as const, code: "E_INTERNAL" as const };
      if (!r.ok) return (setError(errorText(r.code ?? "E_INTERNAL")), false);
      setImage(d.preview);
      setDraftImage(undefined);
    } else if (d === null) patch.removeImage = true;
    else if (d) Object.assign(patch, { imageFromGallery: d.file }, d.crop && { crop: d.crop });
    if (Object.keys(patch).length && !(await run("card.save", { cardId: id, ...patch })))
      return false;
    onClose();
    return true;
  }

  async function submit() {
    if (!saved) return;
    if (f.avatar.trim() && !cleanIcon(f.avatar.trim())) return setError(t("cardAvatarInvalid"));
    setBusy(true);
    if (cardId) {
      const patch = changed(saved);
      if (
        !patch.name ||
        mode === "create" ||
        window.confirm(t("cardRenameConfirm", { name: patch.name }))
      )
        await saveTo(cardId, patch);
      return setBusy(false);
    }
    // 建立：seat.create 只收名字與公開描述（≤1000 字、30 行）；放不進的公開描述、圖示、秘密段入座後再存
    const blurbFits = cleanText(f.publicMd, 0, 1000) !== null;
    const made = await run("seat.create", {
      name: f.name.trim(),
      blurb: blurbFits ? f.publicMd : "",
    });
    if (made) {
      const base = { ...EMPTY, name: f.name.trim(), publicMd: blurbFits ? f.publicMd : "" };
      setCardId(made.cardId);
      setSaved(base);
      if (!(await saveTo(made.cardId, changed(base)))) setPending(true);
    }
    setBusy(false);
  }

  const loading = mode === "edit" && !saved;
  // 未存修改照桌面版 CardEditor 算：欄位一項、圖的草稿（上傳、移除、用這張選進的生圖）一項
  const unsaved = saved
    ? (Object.keys(changed(saved)).length ? 1 : 0) + (draftImage !== undefined ? 1 : 0)
    : 0;
  const back = () => (unsaved ? setLeaving(true) : onClose());
  return (
    <form
      className="panel card-sheet"
      data-testid="card-sheet"
      onSubmit={(e) => (e.preventDefault(), void submit())}
    >
      <h2>{t(mode === "create" ? "seatCreate" : "cardEdit")}</h2>
      {/* 動作鈕置頂（標題正下方） */}
      <div className="panel-actions">
        <button type="submit" disabled={busy || loading || !f.name.trim()}>
          {t(pending ? "cardRetry" : "cardSave")}
        </button>
        <button type="button" className="ghost" disabled={busy} onClick={back}>
          {t("cardCancel")}
        </button>
      </div>
      {leaving && (
        // 照桌面版的離開確認：兩選，預設焦點在「繼續編輯」，誤按 Enter 不會捨棄
        <div
          className="leave-confirm"
          role="alertdialog"
          aria-labelledby="leave-title"
          data-testid="leave-confirm"
          onKeyDown={(e) => e.key === "Escape" && setLeaving(false)}
        >
          <div className="leave-box">
            <h3 id="leave-title">{t("unsavedLeaveTitle")}</h3>
            <p>{t("unsavedLeaveConfirm", { n: unsaved })}</p>
            <div className="panel-actions">
              <button type="button" autoFocus onClick={() => setLeaving(false)}>
                {t("unsavedLeaveCancel")}
              </button>
              <button type="button" className="danger" onClick={onClose}>
                {t("unsavedLeaveOk")}
              </button>
            </div>
          </div>
        </div>
      )}
      {pending && (
        <div className="field-error" data-testid="card-pending">
          {t("cardSecretPending")}
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
      {loading ? (
        !error && <p className="muted">{t("cardLoading")}</p>
      ) : (
        // 存檔進行中整區停用：存好就關畫面，期間補打的字會不見
        <fieldset disabled={busy} data-testid="card-fields">
          {mode === "edit" && cardId && (
            <CardImage
              view={view}
              act={act}
              cardId={cardId}
              image={draftImage === undefined ? image : (draftImage?.preview ?? null)}
              setDraft={setDraftImage}
              draft={{ name: f.name, description: f.publicMd }}
              canUpload={!!upload}
              gallery={gallery}
              hold={setBusy}
            />
          )}
          <label>
            {t("seatCreateName")}
            <input value={f.name} maxLength={40} onChange={set("name")} data-testid="card-name" />
          </label>
          <label>
            {t("cardAvatar")}
            <input
              value={f.avatar}
              maxLength={16}
              onChange={set("avatar")}
              data-testid="card-avatar"
            />
          </label>
          <label>
            {t("seatCreateBlurb")}
            <textarea
              value={f.publicMd}
              maxLength={64000}
              rows={5}
              onChange={set("publicMd")}
              data-testid="card-public"
            />
          </label>
          <label>
            {t("cardPrivate")}
            <span className="muted">{t("cardPrivateHint")}</span>
            <textarea
              value={f.privateMd}
              maxLength={64000}
              rows={5}
              placeholder={t("cardPrivatePlaceholder")}
              onChange={set("privateMd")}
              data-testid="card-private"
            />
          </label>
        </fieldset>
      )}
    </form>
  );
}
