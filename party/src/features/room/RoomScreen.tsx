// 房間畫面：回合列在上、分頁（故事／名單／房主／出額度）、故事頁底部固定行動格（沒座位時換成選角）。
import { useState } from "react";
import { t, type MsgKey } from "../../i18n";
import { HostPanel } from "../controls/HostPanel";
import { PairForm } from "../controls/PairForm";
import { PayerPanel } from "../controls/PayerPanel";
import { SeatPicker } from "../join/SeatPicker";
import { ActionBox } from "./ActionBox";
import type { RoomView } from "./room-state";
import { RoundBar } from "./RoundBar";
import { StoryView } from "./StoryView";
import { useRun, type Act } from "./useRoom";

const STATUS: Record<string, MsgKey> = { pending: "statusPending", drafting: "statusDrafting", submitted: "statusSubmitted", skipped: "statusSkipped" };

function MemberList({ view, act, onLeave }: { view: RoomView; act: Act; onLeave: () => void }) {
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
      {view.room.payerId !== view.me.memberId && view.room.status !== "closed" && <PairForm act={act} />}
      {error && <div className="field-error">{error}</div>}
      <div className="panel-actions">
        {view.me.cardId && view.room.status !== "closed" && <button type="button" onClick={() => run("seat.release")}>{t("seatRelease")}</button>}
        <button type="button" className="ghost danger" onClick={async () => (await run("member.leave")) && onLeave()}>{t("leaveRoom")}</button>
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
  exportRoom: () => Promise<boolean>;
  onLeave: () => void;
  origin: string;
}

export function RoomScreen({ view, c }: { view: RoomView; c: RoomControls }) {
  const [tab, setTab] = useState<"story" | "members" | "host" | "payer">("story");
  const isHost = view.room.hostId === view.me.memberId;
  const isPayer = view.room.payerId === view.me.memberId;
  const tabs: Array<typeof tab> = ["story", "members", ...(isHost ? ["host" as const] : []), ...(isPayer ? ["payer" as const] : [])];
  const canSit = view.room.status === "lobby" || view.room.status === "playing";
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
          {view.me.cardId ? <ActionBox view={view} act={c.act} sendDraft={c.sendDraft} cancelDraft={c.cancelDraft} /> : canSit && <SeatPicker view={view} act={c.act} />}
        </>
      )}
      {tab === "members" && <MemberList view={view} act={c.act} onLeave={c.onLeave} />}
      {tab === "host" && isHost && <HostPanel view={view} act={c.act} origin={c.origin} exportRoom={c.exportRoom} />}
      {tab === "payer" && isPayer && <PayerPanel view={view} act={c.act} resumed={c.resumed} />}
    </>
  );
}
