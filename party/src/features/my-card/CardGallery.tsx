// 自己座位那張卡的圖庫（room-phone-gallery §3.6）：照桌面版生圖窗的圖庫，新的在前、一頁一頁載入；
// 點一張先向 GM 主機取較大的預覽給裁切窗。列頁與取單張各自同時只送一件，撞到限流就等到期自動重送一次。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { errorText } from "../room/error-text";
import type { Act } from "../room/useRoom";

type Item = { file: string; thumb: string };

/** 撞到限流就等到期自動重送一次 */
export async function ask(act: Act, p: object) {
  const reply = await act("card.gallery", p);
  if (reply.ok || reply.code !== "E_RATE_LIMITED") return reply;
  await new Promise((r) => setTimeout(r, reply.retryAfterMs ?? 1000));
  return act("card.gallery", p);
}

interface Props {
  act: Act;
  /** 目前角色圖從哪張圖庫圖來（存檔後的） */
  inUse: string | null;
  /** 草稿裡選的那張 */
  picked: string | null;
  onPick: (file: string, preview: string) => void;
}

export function CardGallery({ act, inUse, picked, onPick }: Props) {
  const [items, setItems] = useState<Item[]>([]);
  const [more, setMore] = useState(0);
  const [busy, setBusy] = useState<"" | "list" | "file">("list");
  const [error, setError] = useState<string | null>(null);

  async function load(before?: string) {
    setBusy("list");
    const reply = await ask(act, before ? { before } : {});
    setBusy("");
    if (!reply.ok) return setError(errorText(reply.code, reply.retryAfterMs));
    setError(null);
    setItems((now) => (before ? [...now, ...reply.result.items] : reply.result.items));
    setMore(reply.result.more);
  }

  async function pick(file: string) {
    setBusy("file");
    const reply = await ask(act, { file });
    setBusy("");
    if (!reply.ok) return setError(errorText(reply.code, reply.retryAfterMs));
    setError(null);
    onPick(file, reply.result.items[0].thumb);
  }

  useEffect(() => void load(), []); // 打開才撈；要重撈由外層換 key

  return (
    <div className="card-gallery" data-testid="card-gallery">
      <div className="card-gallery-grid">
        {items.map((it) => (
          <button
            key={it.file}
            type="button"
            className={`card-gallery-thumb${it.file === picked ? " picked" : ""}`}
            disabled={!!busy}
            onClick={() => void pick(it.file)}
          >
            <img src={it.thumb} alt="" />
            {it.file === inUse && (
              <span className="card-gallery-badge">{t("cardGalleryInUse")}</span>
            )}
          </button>
        ))}
      </div>
      {busy === "list" && <p className="muted">{t("cardLoading")}</p>}
      {more > 0 && busy !== "list" && (
        <button
          type="button"
          className="ghost"
          disabled={!!busy}
          onClick={() => void load(items[items.length - 1]?.file)}
        >
          {t("cardGalleryMore", { n: more })}
        </button>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
