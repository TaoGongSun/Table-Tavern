// 劇透遮罩（plans/vn-mode-pkg4.md §2.5，君子鎖）：鎖住時蓋住內容，給「不看就重骰」與「解鎖」（二次確認）；
// 解鎖後在原位置留「重新上鎖」。鎖存在 vn.json，同一桌所有遮罩一起開關。
import { confirm } from "@tauri-apps/plugin-dialog";
import { useState, type ReactNode } from "react";
import { t } from "../../../i18n";
import { backendText } from "../../../shared/ui/backend-text";
import { bibleErrorKey, useBible } from "./useBible";
import "./vn-bible.css";

export function SpoilerMask({
  worldId,
  onReroll,
  title = "vnSpoilerLocked",
  rerollLabel = "vnSpoilerReroll",
  children,
}: {
  worldId: string;
  /** 遮罩標題與重骰鈕文案（劇本分頁講清楚蓋住的是什麼） */
  title?: "vnSpoilerLocked" | "vnBibleMaskTitle";
  rerollLabel?: "vnSpoilerReroll" | "vnBibleMaskReroll";
  /** 「不看就重骰」；沒給（GM 世界書條目：匯入的內容沒得重骰）就只有解鎖 */
  onReroll?: () => Promise<void>;
  children: ReactNode;
}) {
  const bible = useBible(worldId);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function act(work: () => Promise<void>, done?: string) {
    setBusy(true);
    setMessage(null);
    try {
      await work();
      if (done) setMessage(done);
    } catch (reason) {
      const key = bibleErrorKey(reason);
      setMessage(key ? t(key) : backendText(reason));
    } finally {
      setBusy(false);
    }
  }

  async function unlock() {
    const ok = await confirm(t("vnSpoilerUnlockConfirm"), {
      title: t("vnSpoilerUnlockTitle"),
      kind: "warning",
      okLabel: t("vnSpoilerUnlock"),
      cancelLabel: t("dialogCancel"),
    });
    if (ok) await act(() => bible.setLocked(false));
  }

  if (!bible.locked) {
    return (
      <div className="vn-spoiler-open">
        {children}
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={() => void act(() => bible.setLocked(true))}
        >
          {t("vnSpoilerRelock")}
        </button>
      </div>
    );
  }
  const played = bible.view?.played ?? false;
  return (
    <div className="vn-spoiler-mask" role="group" aria-label={t(title)}>
      <strong>{t(title)}</strong>
      <div className="vn-spoiler-actions">
        {onReroll && (
          <button
            type="button"
            className="btn btn-sm"
            disabled={busy}
            onClick={() =>
              played ? setMessage(t("vnBiblePlayed")) : void act(onReroll, t("vnSpoilerRerolled"))
            }
          >
            {busy ? t("vnSpoilerRerolling") : t(rerollLabel)}
          </button>
        )}
        <button type="button" className="btn btn-sm" disabled={busy} onClick={() => void unlock()}>
          {t("vnSpoilerUnlock")}
        </button>
      </div>
      {message && <p className="vn-spoiler-message">{message}</p>}
    </div>
  );
}
