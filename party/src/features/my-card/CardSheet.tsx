// 編卡畫面（room-player-cards §2、§3.2）：建立與編輯共用。欄位照桌面版玩家卡：角色圖（只有編輯時）、名字、圖示、公開描述、秘密情報。
// 秘密情報不經 seat.create：建立時先入座，再接著 card.save 秘密段；這步失敗畫面不關、原文保留可重試。
import { useEffect, useState } from "react";
import { cleanText } from "../../../server/core/validate";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { CardImage } from "./CardImage";
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
  const [saved, setSaved] = useState<Fields | null>(mode === "create" ? { ...EMPTY, name: view.me.nickname } : null);
  const [f, setF] = useState<Fields>(saved ?? EMPTY);
  const [image, setImage] = useState<string | null>(null);
  const [cardId, setCardId] = useState<string | null>(mode === "edit" ? view.me.cardId : null);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState(false); // 已建卡入座、秘密段還沒存上

  useEffect(() => {
    if (mode !== "edit") return;
    void act("card.mine").then((reply) => {
      if (!reply.ok) return setError(reply.code === "E_TEXT_INVALID" ? t("cardTooLong") : t("cardLoadFailed"));
      const c = reply.result.card;
      const loaded = { name: c.name, avatar: c.avatar, publicMd: c.publicMd, privateMd: c.privateMd };
      setSaved(loaded);
      setF(loaded);
      setImage(c.image);
      setCardId(c.cardId);
    });
  }, [mode, act, setError]);

  const set = (k: keyof Fields) => (e: { target: { value: string } }) => setF({ ...f, [k]: e.target.value });

  /** 只送改過的欄位；圖示空白不送（伺服器要 1–16 字元） */
  function changed(base: Fields): Record<string, string> {
    const out: Record<string, string> = {};
    if (f.name.trim() !== base.name) out.name = f.name.trim();
    if (f.avatar.trim() && f.avatar.trim() !== base.avatar) out.avatar = f.avatar.trim();
    if (f.publicMd !== base.publicMd) out.publicMd = f.publicMd;
    if (f.privateMd !== base.privateMd) out.privateMd = f.privateMd;
    return out;
  }

  async function saveTo(id: string, patch: Record<string, string>) {
    if (Object.keys(patch).length && !(await run("card.save", { cardId: id, ...patch }))) return false;
    onClose();
    return true;
  }

  async function submit() {
    if (!saved) return;
    setBusy(true);
    if (cardId) {
      const patch = changed(saved);
      if (!patch.name || mode === "create" || window.confirm(t("cardRenameConfirm", { name: patch.name }))) await saveTo(cardId, patch);
      return setBusy(false);
    }
    // 建立：seat.create 只收名字與公開描述（≤1000 字、30 行）；放不進的公開描述、圖示、秘密段入座後再存
    const blurbFits = cleanText(f.publicMd, 0, 1000) !== null;
    const made = await run("seat.create", { name: f.name.trim(), blurb: blurbFits ? f.publicMd : "" });
    if (made) {
      const base = { ...EMPTY, name: f.name.trim(), publicMd: blurbFits ? f.publicMd : "" };
      setCardId(made.cardId);
      setSaved(base);
      if (!(await saveTo(made.cardId, changed(base)))) setPending(true);
    }
    setBusy(false);
  }

  const loading = mode === "edit" && !saved;
  return (
    <form className="panel card-sheet" data-testid="card-sheet" onSubmit={(e) => (e.preventDefault(), void submit())}>
      <h2>{t(mode === "create" ? "seatCreate" : "cardEdit")}</h2>
      {/* 動作鈕置頂（標題正下方） */}
      <div className="panel-actions">
        <button type="submit" disabled={busy || loading || !f.name.trim()}>{t(pending ? "cardRetry" : "cardSave")}</button>
        <button type="button" className="ghost" onClick={onClose}>{t("cardCancel")}</button>
      </div>
      {pending && <div className="field-error" data-testid="card-pending">{t("cardSecretPending")}</div>}
      {error && <div className="field-error">{error}</div>}
      {loading ? (
        !error && <p className="muted">{t("cardLoading")}</p>
      ) : (
        <>
          {mode === "edit" && cardId && (
            <CardImage view={view} act={act} cardId={cardId} image={image} setImage={setImage} draft={{ name: f.name, description: f.publicMd }} upload={upload} />
          )}
          <label>
            {t("seatCreateName")}
            <input value={f.name} maxLength={40} onChange={set("name")} data-testid="card-name" />
          </label>
          <label>
            {t("cardAvatar")}
            <input value={f.avatar} maxLength={16} onChange={set("avatar")} />
          </label>
          <label>
            {t("seatCreateBlurb")}
            <textarea value={f.publicMd} maxLength={64000} rows={5} onChange={set("publicMd")} data-testid="card-public" />
          </label>
          <label>
            {t("cardPrivate")}
            <span className="muted">{t("cardPrivateHint")}</span>
            <textarea value={f.privateMd} maxLength={64000} rows={5} placeholder={t("cardPrivatePlaceholder")} onChange={set("privateMd")} data-testid="card-private" />
          </label>
        </>
      )}
    </form>
  );
}
