// 行動格（協定 §3.3）：草稿節流上傳（不公開）、送出即公開、收回成草稿、跳過。GM 生成中可打字不可送；
// 被拒（回合已往下走、GM 還在說）時草稿原樣保留。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

export function ActionBox({ view, act, sendDraft, cancelDraft }: { view: RoomView; act: Act; sendDraft: (text: string) => void; cancelDraft: () => void }) {
  const [text, setText] = useState(view.me.draft ?? "");
  const { run: send, error } = useRun(act);
  useEffect(() => {
    if (view.me.draft !== null) setText(view.me.draft); // 收回、換幕退回的原文
  }, [view.me.draft]);
  const r = view.round;
  const entry = r?.roster.find((e) => e.memberId === view.me.memberId);
  const acting = r?.phase === "acting" && !!entry;
  const done = entry?.status === "submitted" || entry?.status === "skipped";
  const live = view.room.status === "playing" || view.room.status === "transition";
  const run = async (tt: string, p: Record<string, unknown> = {}) => {
    await send(tt, { round: r?.round ?? 0, ...p });
  };
  /** 送出／跳過：先取消待送草稿；送出成功清空輸入框（伺服器也清了草稿），否則把框裡的字重新存成草稿 */
  const commit = async (tt: string, p: Record<string, unknown> = {}) => {
    cancelDraft();
    const ok = await send(tt, { round: r?.round ?? 0, ...p });
    if (ok && tt === "action.submit") setText("");
    else if (text) sendDraft(text);
  };
  const hint = !live ? t("actionWaitStart") : r?.phase !== "acting" ? t("errRoundNotActing") : !entry ? t("actionNextRound") : done ? t("actionDone") : null;
  return (
    <div className="action-box" data-testid="action-box">
      {hint && <div className="action-hint">{hint}</div>}
      <textarea
        value={text}
        rows={3}
        maxLength={2000}
        placeholder={t("actionPlaceholder")}
        data-testid="action-text"
        onChange={(e) => {
          setText(e.target.value);
          if (live && !done) sendDraft(e.target.value);
        }}
      />
      {error && <div className="field-error" data-testid="action-error">{error}</div>}
      <div className="action-buttons">
        {done ? (
          <button type="button" disabled={!acting} onClick={() => run("action.retract")}>{t("actionRetract")}</button>
        ) : (
          <>
            <button type="button" className="ghost" disabled={!acting} onClick={() => commit("action.skip")}>{t("actionSkip")}</button>
            <button type="button" className="primary" disabled={!acting || !text.trim()} onClick={() => commit("action.submit", { text })}>
              {t("actionSubmit")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
