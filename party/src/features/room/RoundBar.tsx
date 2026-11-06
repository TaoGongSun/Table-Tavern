// 回合列：回合號與幕、倒數、GM 生成中／主機離線等待中、held 的公開錯誤；房主（與出額度者）的回合級按鈕也在這裡。
// AI 代管（room-seat-delegate §5.1）：倒數說明、全員代管停住的原因、「還在等」裡離線者的代管鈕、重開後還沒連上的提醒。
import { useEffect, useState, type ReactNode } from "react";
import { modelName, t } from "../../i18n";
import { DelegateButton } from "./DelegateButton";
import { allDelegated, offlineSeated } from "./delegation";
import { clock, HOLD_TEXT } from "./error-text";
import { HeldActions } from "../controls/HeldActions";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

const mmss = (ms: number) =>
  `${Math.floor(ms / 60_000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;

function status(v: RoomView, now: number): string {
  const r = v.round;
  if (v.room.status === "closed") return t("errRoomClosed");
  if (!v.executor.online && v.executor.paused) return t("roundHostPaused");
  if (v.executor.syncing) return t("roundHostSyncing");
  if (v.room.status === "lobby") return t("roundLobby");
  if (v.room.status === "transition")
    return t(v.room.transition?.stalled ? "roundStalled" : "roundTransition");
  if (!r) return "";
  if (r.phase === "held" && r.hold) return held(v, r.hold);
  if (r.phase !== "acting")
    return t(
      v.executor.online
        ? "roundGenerating"
        : v.executor.paused
          ? "roundHostPaused"
          : "roundExecutorOffline",
    );
  if (r.roster.length === 0 && allDelegated(v))
    return t(v.room.hostId === v.me.memberId ? "roundAllDelegatedHost" : "roundAllDelegated");
  if (r.deadline !== null)
    return t("roundCountdown", { time: mmss(Math.max(0, r.deadline - now)) });
  return r.pausedRemainingMs !== null
    ? t("roundPaused", { time: mmss(r.pausedRemainingMs) })
    : t("roundActing");
}

/** 卡住：只講發生什麼；能處理的人（出額度者）鈕就在旁邊，其他人寫等誰（換幕的句子本來就點名房主） */
function held(v: RoomView, hold: NonNullable<NonNullable<RoomView["round"]>["hold"]>): string {
  const base =
    hold.kind === "quota" && hold.resetsAt
      ? t("holdQuotaAt", { time: clock(hold.resetsAt) })
      : t(HOLD_TEXT[hold.kind]);
  if (v.room.payerId === v.me.memberId || hold.kind === "scene_full") return base;
  const payer = v.members.find((m) => m.memberId === v.room.payerId)?.nickname;
  if (v.room.hostId !== v.me.memberId) return t("holdWaitHost", { status: base });
  return payer ? t("holdWaitPayer", { status: base, name: payer }) : base;
}

/** 重開後的提醒：房主剛把房間從暫停接回（客戶端記憶體，重新整理後就不再提醒） */
export interface Reopen {
  shown: boolean;
  dismiss: () => void;
  openAttendance: () => void;
}

/** `heldDetail`：桌面版本機遮罩過的錯誤原文；沒給就用房間的分類說明（只給出額度者，他才能處理）。
 *  `compact`（手機）：平時只一行摘要（狀態＋回合號，含倒數），點了展開 GM 模型、還在等誰、倒數說明；
 *  能按、要處理的東西（回合鈕、卡住處理與錯誤詳情、離線者的代管鈕、重開提醒）不收。 */
export function RoundBar({
  view,
  act,
  reopen,
  heldDetail,
  resumed,
  compact = false,
}: {
  view: RoomView;
  act: Act;
  reopen?: Reopen;
  heldDetail?: ReactNode;
  resumed: (round: number) => void;
  compact?: boolean;
}) {
  const [now, setNow] = useState(Date.now());
  const [more, setMore] = useState(false);
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
  const waiting =
    r?.phase === "acting"
      ? r.roster.filter((e) => e.status === "pending" || e.status === "drafting")
      : [];
  const nameOf = (cardId: string) => view.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
  const offline = host ? offlineSeated(view) : [];
  const missing = reopen?.shown && view.room.status === "playing" ? offline : [];
  const payer = view.room.payerId === view.me.memberId;
  const classified =
    r?.phase === "held" && view.errorDetail?.round === r.round ? view.errorDetail.error : null;
  const detail =
    payer && r?.phase === "held"
      ? (heldDetail ??
        (classified && (
          <>
            <strong>{classified.code}</strong> {classified.message}
          </>
        )))
      : null;
  const blocked = !view.members.some((m) => m.cardId && !m.delegated)
    ? t("startNeedPlayer")
    : !view.executor.online
      ? t("startNeedExecutor")
      : view.executor.syncing
        ? t("roundHostSyncing")
        : null;
  const roundNo = r && (
    <span className="round-no">{t("roundNumber", { round: r.round, scene: view.room.scene })}</span>
  );
  const statusText = <span className="round-status">{status(view, now)}</span>;
  const errDetail = detail && (
    <details className="round-detail" data-testid="error-detail">
      <summary>{t("gmErrDetail")}</summary>
      <small>{detail}</small>
    </details>
  );
  const gmModel = view.room.sceneSettings && (
    <span className="round-gm">
      {t("gmModel", { model: modelName(view.room.sceneSettings.modelLabel) })}
    </span>
  );
  const shown = !compact || more;
  // 沒有可展開的細節（大廳、生成中）：摘要不畫 ▾
  const expandable =
    !!gmModel || waiting.length > 0 || (r?.phase === "acting" && r.deadline !== null);
  // 手機收起時「還在等」只在有房主能按的代管鈕時露出
  const waitingShown =
    waiting.length > 0 &&
    (shown || waiting.some((e) => offline.some((m) => m.memberId === e.memberId)));
  return (
    <div className="round-bar" data-testid="round-bar">
      {compact ? (
        <button
          type="button"
          className="round-summary"
          aria-expanded={expandable ? more : undefined}
          data-testid="round-summary"
          onClick={() => setMore((m) => !m)}
        >
          {statusText}
          {roundNo && <> · {roundNo}</>}
        </button>
      ) : (
        <div className="round-line">
          {roundNo}
          {statusText}
          {errDetail}
          {gmModel}
        </div>
      )}
      {/* 卡住時給出額度者的錯誤詳情是要他處理的：不收 */}
      {compact && errDetail && <div className="round-line">{errDetail}</div>}
      {compact && more && gmModel && <div className="round-line round-more">{gmModel}</div>}
      <div className="round-actions">
        {host && view.room.status === "lobby" && (
          <button
            type="button"
            className="primary"
            disabled={blocked !== null}
            title={blocked ?? undefined}
            onClick={() => run("room.start")}
          >
            {t("hostStart")}
          </button>
        )}
        {host && view.room.status === "lobby" && blocked && (
          <span className="action-hint" data-testid="start-blocked">
            {blocked}
          </span>
        )}
        {host && r?.phase === "acting" && r.roster.length > 0 && (
          <button type="button" onClick={() => run("round.resolve", { round: r.round })}>
            {waiting.length > 0 ? t("hostResolvePending", { n: waiting.length }) : t("hostResolve")}
          </button>
        )}
        {boss && r?.phase === "generating" && (
          <button type="button" onClick={() => run("gen.abort", { round: r.round })}>
            {t("roundAbort")}
          </button>
        )}
        <HeldActions view={view} act={act} resumed={resumed} />
        {boss && r?.phase === "held" && r.hold?.actions.includes("continue") && (
          <button type="button" onClick={() => run("round.continue", { round: r.round })}>
            {t("roundContinue")}
          </button>
        )}
        {host && view.room.transition?.stalled && (
          <button type="button" onClick={() => run("scene.advance_cancel")}>
            {t("hostAdvanceCancel")}
          </button>
        )}
      </div>
      {shown && r?.phase === "acting" && r.deadline !== null && (
        <div className="round-waiting" data-testid="countdown-note">
          {t("roundCountdownNote")}
        </div>
      )}
      {/* 房主：還在等的人裡沒連線的，名字旁直接給代管鈕，不必切到房主頁 */}
      {waitingShown && (
        <div className="round-waiting" data-testid="round-waiting">
          {t("roundWaiting", { names: "" })}
          {waiting.map((e, i) => (
            <span key={e.memberId} className="waiting-name">
              {i > 0 && "、"}
              {nameOf(e.cardId)}
              {offline.some((m) => m.memberId === e.memberId) && (
                <>
                  {" "}
                  <span className="dot" title={t("offline")} />{" "}
                  <DelegateButton
                    view={view}
                    act={act}
                    memberIds={[e.memberId]}
                    on
                    label={t("delegateOnOther")}
                    className="ghost small"
                    testId={`waiting-delegate-${e.memberId}`}
                  />
                </>
              )}
            </span>
          ))}
        </div>
      )}
      {missing.length > 0 && (
        <div className="reopen-note" data-testid="reopen-note">
          <span className="grow">
            {t("reopenMissing", { names: missing.map((m) => nameOf(m.cardId!)).join("、") })}
          </span>
          <DelegateButton
            view={view}
            act={act}
            memberIds={missing.map((m) => m.memberId)}
            on
            label={t("reopenDelegateAll")}
            onDone={reopen!.dismiss}
          />
          <button
            type="button"
            className="ghost"
            onClick={() => (reopen!.dismiss(), reopen!.openAttendance())}
          >
            {t("hostAttendance")}
          </button>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
