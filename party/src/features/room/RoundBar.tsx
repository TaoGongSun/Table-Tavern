// 回合列：回合號與幕、倒數、GM 生成中／主機離線等待中、held 的公開錯誤；房主（與出額度者）的回合級按鈕也在這裡。
// AI 代管（room-seat-delegate §5.1）：倒數說明、全員代管停住的原因、「還在等」裡離線者的代管鈕、重開後還沒連上的提醒。
import { useEffect, useState } from "react";
import { modelName, t } from "../../i18n";
import { DelegateButton } from "./DelegateButton";
import { allDelegated, offlineSeated } from "./delegation";
import { clock, HOLD_TEXT, HOLD_TEXT_PAYER } from "./error-text";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

const mmss = (ms: number) => `${Math.floor(ms / 60_000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;

function status(v: RoomView, now: number): string {
  const r = v.round;
  if (v.room.status === "closed") return t("errRoomClosed");
  if (!v.executor.online && v.executor.paused) return t("roundHostPaused");
  if (v.executor.syncing) return t("roundHostSyncing");
  if (v.room.status === "lobby") return t("roundLobby");
  if (v.room.status === "transition") return t(v.room.transition?.stalled ? "roundStalled" : "roundTransition");
  if (!r) return "";
  if (r.phase === "held" && r.hold) return r.hold.kind === "quota" && r.hold.resetsAt ? t("holdQuotaAt", { time: clock(r.hold.resetsAt) }) : t((v.room.payerId === v.me.memberId && HOLD_TEXT_PAYER[r.hold.kind]) || HOLD_TEXT[r.hold.kind]);
  if (r.phase !== "acting") return t(v.executor.online ? "roundGenerating" : v.executor.paused ? "roundHostPaused" : "roundExecutorOffline");
  if (r.roster.length === 0 && allDelegated(v)) return t(v.room.hostId === v.me.memberId ? "roundAllDelegatedHost" : "roundAllDelegated");
  if (r.deadline !== null) return t("roundCountdown", { time: mmss(Math.max(0, r.deadline - now)) });
  return r.pausedRemainingMs !== null ? t("roundPaused", { time: mmss(r.pausedRemainingMs) }) : t("roundActing");
}

/** 重開後的提醒：房主剛把房間從暫停接回（客戶端記憶體，重新整理後就不再提醒） */
export interface Reopen {
  shown: boolean;
  dismiss: () => void;
  openAttendance: () => void;
}

export function RoundBar({ view, act, reopen }: { view: RoomView; act: Act; reopen?: Reopen }) {
  const [now, setNow] = useState(Date.now());
  const { run, error } = useRun(act);
  const r = view.round;
  useEffect(() => {
    if (r?.deadline == null) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [r?.deadline]);
  const host = view.room.hostId === view.me.memberId;
  const boss = host || view.room.payerId === view.me.memberId;
  // 還沒送的人（純顯示，取自 roster）
  const waiting = r?.phase === "acting" ? r.roster.filter((e) => e.status === "pending" || e.status === "drafting") : [];
  const nameOf = (cardId: string) => view.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
  const offline = host ? offlineSeated(view) : [];
  const missing = reopen?.shown && view.room.status === "playing" ? offline : [];
  const blocked = !view.members.some((m) => m.cardId && !m.delegated) ? t("startNeedPlayer") : !view.executor.online ? t("startNeedExecutor") : view.executor.syncing ? t("roundHostSyncing") : null;
  return (
    <div className="round-bar" data-testid="round-bar">
      <div className="round-line">
        {r && <span className="round-no">{t("roundNumber", { round: r.round, scene: view.room.scene })}</span>}
        <span className="round-status">{status(view, now)}</span>
        {view.room.sceneSettings && <span className="round-gm">{t("gmModel", { model: modelName(view.room.sceneSettings.modelLabel) })}</span>}
      </div>
      <div className="round-actions">
        {host && view.room.status === "lobby" && <button type="button" className="primary" disabled={blocked !== null} title={blocked ?? undefined} onClick={() => run("room.start")}>{t("hostStart")}</button>}
        {host && view.room.status === "lobby" && blocked && <span className="action-hint" data-testid="start-blocked">{blocked}</span>}
        {host && r?.phase === "acting" && r.roster.length > 0 && <button type="button" onClick={() => run("round.resolve", { round: r.round })}>{waiting.length > 0 ? t("hostResolvePending", { n: waiting.length }) : t("hostResolve")}</button>}
        {boss && r?.phase === "generating" && <button type="button" onClick={() => run("gen.abort", { round: r.round })}>{t("roundAbort")}</button>}
        {boss && r?.phase === "held" && r.hold?.actions.includes("continue") && <button type="button" onClick={() => run("round.continue", { round: r.round })}>{t("roundContinue")}</button>}
        {host && view.room.transition?.stalled && <button type="button" onClick={() => run("scene.advance_cancel")}>{t("hostAdvanceCancel")}</button>}
      </div>
      {r?.phase === "acting" && r.deadline !== null && <div className="round-waiting" data-testid="countdown-note">{t("roundCountdownNote")}</div>}
      {/* 房主：還在等的人裡沒連線的，名字旁直接給代管鈕，不必切到房主頁 */}
      {waiting.length > 0 && (
        <div className="round-waiting" data-testid="round-waiting">
          {t("roundWaiting", { names: "" })}
          {waiting.map((e, i) => (
            <span key={e.memberId} className="waiting-name">
              {i > 0 && "、"}
              {nameOf(e.cardId)}
              {offline.some((m) => m.memberId === e.memberId) && (
                <>
                  {" "}<span className="dot" title={t("offline")} />{" "}
                  <DelegateButton view={view} act={act} memberIds={[e.memberId]} on label={t("delegateOnOther")} className="ghost small" testId={`waiting-delegate-${e.memberId}`} />
                </>
              )}
            </span>
          ))}
        </div>
      )}
      {missing.length > 0 && (
        <div className="reopen-note" data-testid="reopen-note">
          <span className="grow">{t("reopenMissing", { names: missing.map((m) => nameOf(m.cardId!)).join("、") })}</span>
          <DelegateButton view={view} act={act} memberIds={missing.map((m) => m.memberId)} on label={t("reopenDelegateAll")} onDone={reopen!.dismiss} />
          <button type="button" className="ghost" onClick={() => (reopen!.dismiss(), reopen!.openAttendance())}>{t("hostAttendance")}</button>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
