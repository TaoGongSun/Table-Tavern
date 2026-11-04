// 房間畫面：回合列在上、代管中的常駐列、分頁（故事／名單／房主／出額度）、故事頁底部固定行動格（沒座位時換成選角）。
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { t, type MsgKey } from "../../i18n";
import { GmControls } from "../controls/GmControls";
import { HostPanel } from "../controls/HostPanel";
import { UndoControls } from "../controls/UndoControls";
import { PairForm } from "../controls/PairForm";
import { PayerPanel } from "../controls/PayerPanel";
import { SeatPicker } from "../join/SeatPicker";
import type { Upload } from "../my-card/card-upload";
import { CardSheet } from "../my-card/CardSheet";
import { ActionBox } from "./ActionBox";
import { Confirm } from "./Confirm";
import { DelegateButton } from "./DelegateButton";
import { DelegatedBanner } from "./DelegatedBanner";
import { canDelegate, sentThisRound } from "./delegation";
import type { RoomView } from "./room-state";
import { RoundBar } from "./RoundBar";
import { SceneStatusLine } from "./scene-status";
import { StoryView } from "./StoryView";
import { useRun, type Act } from "./useRoom";

const STATUS: Record<string, MsgKey> = { pending: "statusPending", drafting: "statusDrafting", submitted: "statusSubmitted", skipped: "statusSkipped" };

function MemberList({ view, act, c }: { view: RoomView; act: Act; c: RoomControls }) {
  const { run, error } = useRun(act);
  const [leaving, setLeaving] = useState(false);
  const [releasing, setReleasing] = useState(false);
  const desktop = c.pairForm === false;
  const seatedPlaying = !!view.me.cardId && canDelegate(view);
  // 已入座、遊戲中、還沒代管：離開前問要不要先交給 AI 代管；其他情況照舊一按就走（桌面版的離開＝關房，不問）
  const leave = () => (!desktop && seatedPlaying && !view.me.delegated ? setLeaving(true) : c.onLeave());
  const delegateAndLeave = async () => {
    if (await run("seat.delegate", { memberId: view.me.memberId, on: true })) c.onLeave();
    else setLeaving(false);
  };
  const roster = new Map(view.round?.roster.map((e) => [e.memberId, e.status]));
  return (
    <div className="panel" data-testid="members">
      {view.members.map((m) => (
        <div key={m.memberId} className="row member" data-testid={`member-${m.memberId}`}>
          <span className={m.connected ? "dot on" : "dot"} title={t(m.connected ? "online" : "offline")} />
          <span className="grow">
            {m.nickname}
            {m.cardId && ` · ${view.cards.find((c) => c.cardId === m.cardId)?.name ?? ""}`}
            {view.room.hostId === m.memberId && <span className="badge">{t("badgeHost")}</span>}
            {view.room.payerId === m.memberId && <span className="badge" title={t("badgePayerHint")}>{t(m.memberId === view.me.memberId ? "badgePayerSelf" : "badgePayer")}</span>}
          </span>
          {m.delegated ? <span className="chip">{t("delegatedBadge")}</span> : roster.has(m.memberId) && <span className="chip">{t(STATUS[roster.get(m.memberId)!])}</span>}
        </div>
      ))}
      <p className="muted">{t("idleNotice", { days: view.room.idleDays })}</p>
      {c.pairForm !== false && view.room.payerId !== view.me.memberId && view.room.status !== "closed" && <PairForm act={act} />}
      {error && <div className="field-error">{error}</div>}
      <div className="panel-actions member-actions">
        {seatedPlaying && <DelegateButton view={view} act={act} memberIds={[view.me.memberId]} on={!view.me.delegated} label={t(view.me.delegated ? "delegateTakeBack" : "delegateOn")} testId="self-delegate" />}
        {/* 暫時離開＝只斷線（座位與角色保留），可以回來所以不用紅框；桌面版換成「關房（暫停）」照舊紅框 */}
        <button type="button" className={desktop ? "ghost danger" : "ghost"} data-testid="leave" onClick={leave}>{c.leaveLabel ?? t("leaveRoom")}</button>
        {/* 離座不可逆：跟另外兩顆分開，靠右紅字 */}
        {view.me.cardId && view.room.status !== "closed" && <button type="button" className="ghost danger seat-release" onClick={() => setReleasing(true)}>{t("seatRelease")}</button>}
      </div>
      {releasing && (
        <Confirm title={t("seatRelease")} testId="seat-release-confirm" onCancel={() => setReleasing(false)} actions={[{ label: t("seatRelease"), danger: true, onClick: async () => { await run("seat.release"); setReleasing(false); } }]}>
          <p>{t("seatReleaseConfirm")}</p>
        </Confirm>
      )}
      {leaving && (
        <Confirm
          title={t("leaveTitle")}
          testId="leave-confirm"
          onCancel={() => setLeaving(false)}
          actions={[{ label: t("leaveDelegate"), onClick: () => void delegateAndLeave() }, { label: t("leaveKeep"), onClick: () => (setLeaving(false), c.onLeave()) }]}
        >
          <p>{t("leaveAsk")}</p>
          {sentThisRound(view, view.me.memberId) && <p>{t("delegateRetractSelf")}</p>}
        </Confirm>
      )}
    </div>
  );
}

export interface RoomControls {
  act: Act;
  sendDraft: (text: string) => void;
  cancelDraft: () => void;
  loadOlder: (beforeSeq: number) => Promise<number>;
  resumed: (round: number) => void;
  /** null＝不提供匯出（桌面版：那張桌本機就有） */
  exportRoom: (() => Promise<boolean>) | null;
  /** null＝桌面版：不給匯入與換圖上傳 */
  upload: Upload | null;
  onLeave: () => void;
  /** 桌面版換掉「暫時離開」的字樣 */
  leaveLabel?: string;
  /** false＝桌面版：不顯示輸入配對碼（接回房主在資訊列）、選角看得到保留給電腦的卡 */
  pairForm?: boolean;
  origin: string;
  /** 桌面版：資訊列那條邀請連結；手機：自己網址上的密語（邀請新朋友時給伺服器驗） */
  inviteUrl?: string | null;
  joinSecret?: string | null;
  payerExtra?: ReactNode;
  heldDetail?: ReactNode;
  /** 桌面版選角區的入口（建立／用我的角色卡、匯入角色卡），取代手機的建立與匯入 */
  seatEntry?: ReactNode;
}

export function RoomScreen({ view, c: controls }: { view: RoomView; c: RoomControls }) {
  const [takenBack, setTakenBack] = useState(false);
  // 只認本人接回成功；房主替別人取消代管、讓座或轉移座位都不算。
  const actRoom: Act = useCallback(async (kind, params, waitMs) => {
    setTakenBack(false);
    const reply = await controls.act(kind, params, waitMs);
    const target = params as { memberId?: string; on?: boolean } | undefined;
    if (reply.ok && kind === "seat.delegate" && target?.on === false && target.memberId === view.me.memberId && view.me.delegated && view.me.cardId) setTakenBack(true);
    return reply;
  }, [controls.act, view.me.memberId, view.me.delegated, view.me.cardId]);
  useEffect(() => {
    if (!takenBack) return;
    const timer = setTimeout(() => setTakenBack(false), 5000);
    return () => clearTimeout(timer);
  }, [takenBack]);
  const c = { ...controls, act: actRoom, sendDraft: (text: string) => { setTakenBack(false); controls.sendDraft(text); } };
  const [tab, setTab] = useState<"story" | "members" | "host" | "payer">("story");
  const isHost = view.room.hostId === view.me.memberId;
  const isPayer = view.room.payerId === view.me.memberId;
  const tabs: Array<typeof tab> = ["story", "members", ...(isHost ? ["host" as const] : []), ...(isPayer ? ["payer" as const] : [])];
  const canSit = view.room.status === "lobby" || view.room.status === "playing";
  // 遊戲中沒座位的房主多半只當主持：選角收成一行，要入座再展開
  const [picking, setPicking] = useState(false);
  const folded = isHost && view.room.status === "playing" && !picking;
  const [sheet, setSheet] = useState<"create" | "edit" | null>(null);
  const [imported, setImported] = useState(false); // 剛匯入卡：提示只帶入了哪些，開編卡畫面就收起
  const open = view.room.status !== "closed";
  // 房主剛把房間從暫停接回（電腦重開）：提醒誰還沒連上；只記在記憶體
  const wasPaused = useRef(view.executor.paused);
  // 桌面版每次（重）開房間都是新掛上的畫面：一開始就算剛接回，沒人缺席就不會出現
  const [reopened, setReopened] = useState(c.pairForm === false);
  const [attend, setAttend] = useState(0); // 從回合列「到場整理」切過來：每按一次捲一次
  useEffect(() => {
    if (wasPaused.current && !view.executor.paused && view.executor.online) setReopened(true);
    wasPaused.current = view.executor.paused;
  }, [view.executor.paused, view.executor.online]);
  const bar = <RoundBar view={view} act={c.act} heldDetail={c.heldDetail} resumed={c.resumed} reopen={isHost ? { shown: reopened, dismiss: () => setReopened(false), openAttendance: () => (setTab("host"), setAttend((n) => n + 1)) } : undefined} />;
  const hostPlaying = isHost && view.room.status === "playing";
  const seated = !!view.me.cardId && open;
  const gm = <GmControls view={view} act={c.act} />;
  const nav = (
    <nav className="tabs">
      {tabs.map((k) => (
        <button key={k} type="button" className={tab === k ? "tab on" : "tab"} onClick={() => setTab(k)} data-testid={`tab-${k}`}>
          {t(`tab_${k}`)}
        </button>
      ))}
    </nav>
  );
  if (sheet && open) return <CardSheet view={view} act={c.act} mode={sheet} upload={c.upload} onClose={() => setSheet(null)} />;
  return (
    <>
      {c.pairForm === false ? (
        // 桌面版：頁籤與回合列同一列（放不下自動換行），省一整列給故事；手機照原本的上下順序
        <div className="room-head">
          {nav}
          {bar}
        </div>
      ) : (
        bar
      )}
      {takenBack ? <div className="delegated-banner" role="status" data-testid="delegate-returned">{t("delegatedReturned")}</div> : <DelegatedBanner view={view} act={c.act} />}
      {c.pairForm !== false && nav}
      {tab === "story" && (
        <>
          <SceneStatusLine parts={view.story} />
          <StoryView view={view} loadOlder={c.loadOlder} tail={hostPlaying && <UndoControls view={view} act={c.act} />} />
          {/* 故事區下方整塊：桌面版高度有上限、自己捲（room-desktop-layout §2-9）；手機是 display: contents，版面照舊 */}
          <div className="room-dock">
            {hostPlaying && !seated && (
              // 沒入座的房主：GM 鈕單獨一列，入座是列尾的小鈕
              <div className="host-controls">
                {gm}
                {folded && <button type="button" className="ghost small seat-unfold" data-testid="seat-unfold" onClick={() => setPicking(true)}>{t("seatShowPicker")}</button>}
              </div>
            )}
            {/* 電腦自己的座位照舊用 CastRail 的卡片編輯 */}
            {seated && c.pairForm !== false && (
              <button type="button" className="ghost" data-testid="card-edit" onClick={() => (setImported(false), setSheet("edit"))}>{t("cardEdit")}</button>
            )}
            {imported && view.me.cardId && <p className="muted" data-testid="import-note">{t("seatImportedNote")}</p>}
            {seated ? (
              <ActionBox view={view} act={c.act} sendDraft={c.sendDraft} cancelDraft={c.cancelDraft} leading={hostPlaying ? gm : undefined} />
            ) : (
              canSit && !folded && <SeatPicker view={view} act={c.act} desktop={c.pairForm === false} upload={c.upload} entry={c.seatEntry} onCreate={() => setSheet("create")} onImported={() => setImported(true)} />
            )}
          </div>
        </>
      )}
      {tab === "members" && <MemberList view={view} act={c.act} c={c} />}
      {tab === "host" && isHost && (
        <HostPanel view={view} act={c.act} invite={{ origin: c.origin, inviteUrl: c.inviteUrl, joinSecret: c.joinSecret }} exportRoom={c.exportRoom} phone={c.pairForm !== false} focusAttendance={attend > 0} key={attend} />
      )}
      {tab === "payer" && isPayer && <PayerPanel view={view} act={c.act} desktop={c.pairForm === false} extra={c.payerExtra} />}
    </>
  );
}
