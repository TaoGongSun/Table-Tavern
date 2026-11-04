// 房主頁「到場整理」（room-seat-delegate §2.2、§5.1）：入座的人一人一列（連線、AI 代管、轉讓、移除時一起決定角色處置、封鎖），
// 沒入座的另一段照舊（接手座位、轉讓、移出、封鎖）；有入座又斷線的人時給一鍵代管。從回合列「到場整理」切過來時捲到這一段。
import { useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { PartyMenu } from "./PartyMenu";
import { Confirm } from "../room/Confirm";
import { DelegateButton } from "../room/DelegateButton";
import { canDelegate, cardName, offlineSeated } from "../room/delegation";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function Attendance({ view, act, focus }: { view: RoomView; act: Act; focus?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) ref.current?.scrollIntoView({ block: "start" });
  }, [focus]);
  const { run, error } = useRun(act);
  const [removing, setRemoving] = useState<string | null>(null);
  const [mode, setMode] = useState<"absent" | "npc">("absent"); // 預設離席，同現行
  const me = view.me.memberId;
  const seated = view.members.filter((m) => m.cardId);
  const unseated = view.members.filter((m) => !m.cardId && m.memberId !== me);
  const offline = canDelegate(view) ? offlineSeated(view) : [];
  const target = view.members.find((m) => m.memberId === removing);
  const remove = async () => {
    if (await run("member.kick", { memberId: removing, ban: false, mode })) setRemoving(null);
  };
  return (
    <div ref={ref} data-testid="attendance">
      <h3>{t("hostAttendance")}</h3>
      {offline.length > 0 && (
        <DelegateButton view={view} act={act} memberIds={offline.map((m) => m.memberId)} on label={t("hostDelegateOffline", { n: offline.length })} testId="delegate-offline" />
      )}
      {seated.map((m) => (
        <div key={m.memberId} className="row attend" data-testid={`host-${m.memberId}`}>
          <span className={m.connected ? "dot on" : "dot"} title={t(m.connected ? "online" : "offline")} />
          <span className="grow">
            {m.nickname} · {cardName(view, m.cardId)}
            {m.memberId === me && t("hostYou")}
            {m.delegated && <span className="chip">{t("delegatedBadge")}</span>}
          </span>
          {canDelegate(view) && (
            <DelegateButton view={view} act={act} memberIds={[m.memberId]} on={!m.delegated} label={t(m.delegated ? "delegateOff" : "delegateOnOther")} className="ghost" testId={`attend-delegate-${m.memberId}`} />
          )}
          {m.memberId !== me && (
            <PartyMenu trigger="⋯" className="ghost" label={t("hostMemberOptions", { name: m.nickname })} items={[
              { key: "transfer", label: t("hostTransfer"), onSelect: () => void run("host.transfer", { memberId: m.memberId }) },
              { key: "remove", label: t("hostRemove"), onSelect: () => (setMode("absent"), setRemoving(m.memberId)) },
              { key: "ban", label: t("hostBan"), danger: true, onSelect: () => void run("member.kick", { memberId: m.memberId, ban: true }) },
            ]} />
          )}
        </div>
      ))}
      {unseated.length > 0 && <h3>{t("hostUnseated")}</h3>}
      {unseated.map((m) => (
        <div key={m.memberId} className="row" data-testid={`host-${m.memberId}`}>
          <span className={m.connected ? "dot on" : "dot"} title={t(m.connected ? "online" : "offline")} />
          <span className="grow">{m.nickname}</span>
          {seated.length > 0 && (
            <select aria-label={t("hostReassign")} value="" onChange={(e) => run("member.reassign", { from: e.target.value, to: m.memberId })}>
              <option value="">{t("hostReassign")}</option>
              {seated.map((x) => <option key={x.memberId} value={x.memberId}>{`${cardName(view, x.cardId)}（${x.nickname}）`}</option>)}
            </select>
          )}
          <PartyMenu trigger="⋯" className="ghost" label={t("hostMemberOptions", { name: m.nickname })} items={[
            { key: "transfer", label: t("hostTransfer"), onSelect: () => void run("host.transfer", { memberId: m.memberId }) },
            { key: "remove", label: t("hostKick"), onSelect: () => void run("member.kick", { memberId: m.memberId, ban: false }) },
            { key: "ban", label: t("hostBan"), danger: true, onSelect: () => void run("member.kick", { memberId: m.memberId, ban: true }) },
          ]} />
        </div>
      ))}
      {target && (
        <Confirm title={t("hostRemoveTitle", { name: target.nickname })} testId="remove-confirm" onCancel={() => setRemoving(null)} actions={[{ label: t("hostRemoveOk"), onClick: () => void remove(), danger: true }]}>
          <p>{t("hostRemoveBody")}</p>
          <fieldset className="remove-mode">
            <legend>{t("hostRemoveRole")}</legend>
            {(["absent", "npc"] as const).map((k) => (
              <label key={k} className="check">
                <input type="radio" name="remove-mode" checked={mode === k} onChange={() => setMode(k)} />
                {t(k === "absent" ? "hostRemoveAbsent" : "hostRemoveNpc")}
              </label>
            ))}
          </fieldset>
        </Confirm>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
