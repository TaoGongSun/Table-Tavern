// 行動格（協定 §3.3）：草稿節流上傳（不公開）、送出即公開、收回成草稿、跳過。GM 生成中可打字不可送；
// 被拒（回合已往下走、GM 還在說）時草稿原樣保留。交給 AI 代管中照樣能打字存草稿，只是送不出（不在名單上）。
import { useEffect, useState, type ReactNode } from "react";
import { t } from "../../i18n";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

/** `leading`：動作列左側（房主的 GM 鈕，照單人版跟送出同一列） */
export function ActionBox({ view, act, sendDraft, cancelDraft, leading }: { view: RoomView; act: Act; sendDraft: (text: string) => void; cancelDraft: () => void; leading?: ReactNode }) {
  const [text, setText] = useState(view.me.draft ?? "");
  const [taken, setTaken] = useState(false); // 房主收回、原文已直接放回輸入框：送出前提示一行
  const [tooLong, setTooLong] = useState(false);
  const [stuck, setStuck] = useState(false); // 自動放回沒存成：原文照留在下面那格
  const { run: send, error } = useRun(act);
  useEffect(() => {
    if (view.me.draft !== null) setText(view.me.draft); // 收回、換幕退回的原文
  }, [view.me.draft]);
  const returned = view.me.returned;
  useEffect(() => {
    // 房主收回的原文：輸入框空著就直接放回去（重連、重開頁面同一條規則）；有草稿就留在下面那格讓他自己帶回或丟掉
    if (!returned || text.trim()) return;
    void bring(returned, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [returned]);
  /** 帶回：草稿跟清原文同一筆交易，伺服器存好才換輸入框；存不進去就原文照留在下面那格 */
  const bring = async (next: string, auto = false) => {
    cancelDraft();
    if (!(await send("action.returned_dismiss", { draft: next }))) {
      if (text && live && !done) sendDraft(text); // 上面取消了還沒上傳的草稿：沒放回就照原樣補送，免得丟掉最後打的字
      if (auto) setStuck(true);
      else setTooLong(true);
      return;
    }
    setText(next);
    setTooLong(false);
    setStuck(false);
    if (auto) setTaken(true);
  };
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
    if (ok && tt === "action.submit") {
      setText("");
      setTaken(false);
    }
    else if (text) sendDraft(text);
  };
  const hint = !live ? t("actionWaitStart") : view.me.delegated ? t("actionDelegated") : r?.phase === "held" ? t("actionHeld") : r?.phase !== "acting" ? t("errRoundNotActing") : !entry ? t("actionNextRound") : done && !(entry?.status === "submitted" && entry.text) ? t("actionDone") : null;
  return (
    <div className="action-box" data-testid="action-box">
      {hint && <div className="action-hint">{hint}</div>}
      {/* 送出後輸入框清空：寫出送了什麼，免得以為沒送出又打一次（原文就在名單上，結算後消失） */}
      {r?.phase === "acting" && entry?.status === "submitted" && entry.text && (
        <div className="action-hint" data-testid="submitted-note">{t("actionSubmittedNote", { text: [...entry.text].length > 20 ? `${[...entry.text].slice(0, 20).join("")}…` : entry.text })}</div>
      )}
      {taken && <div className="action-hint" data-testid="returned-notice">{t("returnedNotice")}</div>}
      {returned && (text.trim() || stuck) && (
        // 房主收回的行動原文：只給本人；已有草稿時不覆蓋，帶回接到草稿尾端
        <div className="returned" data-testid="returned">
          <div className="part-name">{t("returnedTitle")}</div>
          <div className="part-text">{returned}</div>
          <div className="action-buttons">
            <button
              type="button"
              onClick={() => void bring(text ? `${text}\n${returned}` : returned)}
            >
              {t("returnedBring")}
            </button>
            <button type="button" className="ghost" onClick={() => void send("action.returned_dismiss")}>{t("returnedDrop")}</button>
          </div>
          {tooLong && <div className="field-error" data-testid="returned-too-long">{t("returnedTooLong")}</div>}
        </div>
      )}
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
        {leading && <span className="action-leading">{leading}</span>}
        {/* 「收回修改」只在行動中：結算後（GM 生成中、卡住）收不回，照常給停用的跳過／送出，不留一顆一直灰著的收回 */}
        {done && acting ? (
          <button type="button" onClick={() => run("action.retract")}>{t("actionRetract")}</button>
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
