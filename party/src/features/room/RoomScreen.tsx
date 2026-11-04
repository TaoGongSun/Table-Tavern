// 房間畫面：回合列在上、分頁（故事／名單／房主／出額度）、故事頁底部固定行動格（沒座位時換成選角）。
import { useState } from "react";
import { t, type MsgKey } from "../../i18n";
import { HostPanel } from "../controls/HostPanel";
import { PairForm } from "../controls/PairForm";
import { PayerPanel } from "../controls/PayerPanel";
import { SeatPicker } from "../join/SeatPicker";
import type { Upload } from "../my-card/card-upload";
import { CardSheet } from "../my-card/CardSheet";
import { ActionBox } from "./ActionBox";
import type { RoomView } from "./room-state";
import { RoundBar } from "./RoundBar";
import { StoryView } from "./StoryView";
import { useRun, type Act } from "./useRoom";

const STATUS: Record<string, MsgKey> = { pending: "statusPending", drafting: "statusDrafting", submitted: "statusSubmitted", skipped: "statusSkipped" };

function MemberList({ view, act, c }: { view: RoomView; act: Act; c: RoomControls }) {
  const { run, error } = useRun(act);
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
            {view.room.payerId === m.memberId && <span className="badge">{t("badgePayer")}</span>}
          </span>
          {roster.has(m.memberId) && <span className="chip">{t(STATUS[roster.get(m.memberId)!])}</span>}
        </div>
      ))}
      <p className="muted">{t("idleNotice", { days: view.room.idleDays })}</p>
      {c.pairForm !== false && view.room.payerId !== view.me.memberId && view.room.status !== "closed" && <PairForm act={act} />}
      {error && <div className="field-error">{error}</div>}
      <div className="panel-actions">
        {view.me.cardId && view.room.status !== "closed" && <button type="button" onClick={() => run("seat.release")}>{t("seatRelease")}</button>}
        {/* 暫時離開＝只斷線（座位與角色保留）；桌面版換成「關房（暫停）」 */}
        <button type="button" className="ghost danger" onClick={c.onLeave}>{c.leaveLabel ?? t("leaveRoom")}</button>
      </div>
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
}

export function RoomScreen({ view, c }: { view: RoomView; c: RoomControls }) {
  const [tab, setTab] = useState<"story" | "members" | "host" | "payer">("story");
  const isHost = view.room.hostId === view.me.memberId;
  const isPayer = view.room.payerId === view.me.memberId;
  const tabs: Array<typeof tab> = ["story", "members", ...(isHost ? ["host" as const] : []), ...(isPayer ? ["payer" as const] : [])];
  const canSit = view.room.status === "lobby" || view.room.status === "playing";
  // 遊戲中沒座位的房主多半只當主持：選角收成一行，要入座再展開
  const [picking, setPicking] = useState(false);
  const folded = isHost && view.room.status === "playing" && !picking;
  const [sheet, setSheet] = useState<"create" | "edit" | null>(null);
  const open = view.room.status !== "closed";
  if (sheet && open) return <CardSheet view={view} act={c.act} mode={sheet} upload={c.upload} onClose={() => setSheet(null)} />;
  return (
    <>
      <RoundBar view={view} act={c.act} />
      <nav className="tabs">
        {tabs.map((k) => (
          <button key={k} type="button" className={tab === k ? "tab on" : "tab"} onClick={() => setTab(k)} data-testid={`tab-${k}`}>
            {t(`tab_${k}`)}
          </button>
        ))}
      </nav>
      {tab === "story" && (
        <>
          <StoryView view={view} loadOlder={c.loadOlder} />
          {/* 電腦自己的座位照舊用 CastRail 的卡片編輯 */}
          {view.me.cardId && open && c.pairForm !== false && (
            <button type="button" className="ghost" data-testid="card-edit" onClick={() => setSheet("edit")}>{t("cardEdit")}</button>
          )}
          {view.me.cardId && open ? <ActionBox view={view} act={c.act} sendDraft={c.sendDraft} cancelDraft={c.cancelDraft} /> : canSit && (folded ? (
            <button type="button" className="ghost" data-testid="seat-unfold" onClick={() => setPicking(true)}>{t("seatShowPicker")}</button>
          ) : (
            <SeatPicker view={view} act={c.act} desktop={c.pairForm === false} upload={c.upload} onCreate={() => setSheet("create")} />
          ))}
        </>
      )}
      {tab === "members" && <MemberList view={view} act={c.act} c={c} />}
      {tab === "host" && isHost && <HostPanel view={view} act={c.act} origin={c.origin} exportRoom={c.exportRoom} />}
      {tab === "payer" && isPayer && <PayerPanel view={view} act={c.act} resumed={c.resumed} desktop={c.pairForm === false} />}
    </>
  );
}
