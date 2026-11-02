// 房主面板（協定 §5.1 H 欄）：邀請重生、房間設定、換幕（自己是出額度者可同時定下一幕路線／模型）、成員管理
// （移出、封鎖、轉讓房主、把座位指派給換了裝置的人）、離席處置、匯出、關房。回合級的結算／中止在回合列。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { RoutePicker } from "./RoutePicker";

const TIMEOUTS = [null, 60, 120, 300, 900];

export function HostPanel({ view, act, origin, exportRoom }: { view: RoomView; act: Act; origin: string; exportRoom: () => Promise<boolean> }) {
  const { run, error, setError } = useRun(act);
  const [invite, setInvite] = useState<string | null>(null);
  const r = view.round;
  const s = view.room.settings;
  const canAdvance = view.room.status === "playing" && r && (r.phase === "acting" || (r.phase === "held" && r.hold?.kind === "scene_full"));
  const seated = view.members.filter((m) => m.cardId);
  const card = (id: string | null) => view.cards.find((c) => c.cardId === id)?.name ?? "";
  const others = view.members.filter((m) => m.memberId !== view.me.memberId);
  return (
    <div className="panel" data-testid="host-panel">
      <h2>{t("hostTitle")}</h2>
      <div className="panel-actions">
        <button
          type="button"
          onClick={async () => {
            const res = await run("invite.regenerate");
            if (res) setInvite(`${origin}/r/${view.room.code}#k=${res.joinSecret}`);
          }}
        >
          {t("hostInvite")}
        </button>
        <button type="button" onClick={async () => setError((await exportRoom()) ? null : t("errInternal"))}>{t("hostExport")}</button>
      </div>
      {invite && <input className="invite" readOnly value={invite} onFocus={(e) => e.currentTarget.select()} data-testid="invite" />}
      <label>
        {t("hostTimeout")}
        <select value={String(s.actionTimeoutSec)} onChange={(e) => run("room.settings", { patch: { actionTimeoutSec: e.target.value === "null" ? null : Number(e.target.value) } })}>
          {TIMEOUTS.map((n) => <option key={String(n)} value={String(n)}>{n === null ? t("hostTimeoutOff") : t("seconds", { n })}</option>)}
        </select>
      </label>
      <label className="check">
        <input type="checkbox" checked={s.shuffleOrder} onChange={(e) => run("room.settings", { patch: { shuffleOrder: e.target.checked } })} />
        {t("hostShuffle")}
      </label>
      {canAdvance && (
        <>
          <h3>{t("hostAdvance")}</h3>
          {view.room.payerId === view.me.memberId ? (
            <RoutePicker routes={view.routes} current={view.room.sceneSettings} label={t("hostAdvance")} onPick={(next) => run("scene.advance", { round: r.round, next })} />
          ) : (
            <button type="button" onClick={() => run("scene.advance", { round: r.round })}>{t("hostAdvance")}</button>
          )}
        </>
      )}
      {view.departed.map((cardId) => (
        <div key={cardId} className="row">
          <span>{t("hostDeparted", { name: card(cardId) })}</span>
          <button type="button" onClick={() => run("member.departure", { cardId, mode: "npc" })}>{t("hostDepartNpc")}</button>
          <button type="button" onClick={() => run("member.departure", { cardId, mode: "absent" })}>{t("hostDepartAbsent")}</button>
        </div>
      ))}
      <h3>{t("hostMembers")}</h3>
      {others.map((m) => (
        <div key={m.memberId} className="row" data-testid={`host-${m.memberId}`}>
          <span>{m.nickname}{m.cardId && ` · ${card(m.cardId)}`}</span>
          {!m.cardId && seated.length > 0 && (
            <select aria-label={t("hostReassign")} value="" onChange={(e) => run("member.reassign", { from: e.target.value, to: m.memberId })}>
              <option value="">{t("hostReassign")}</option>
              {seated.map((x) => <option key={x.memberId} value={x.memberId}>{`${card(x.cardId)}（${x.nickname}）`}</option>)}
            </select>
          )}
          <button type="button" className="ghost" onClick={() => run("host.transfer", { memberId: m.memberId })}>{t("hostTransfer")}</button>
          <button type="button" className="ghost" onClick={() => run("member.kick", { memberId: m.memberId, ban: false })}>{t("hostKick")}</button>
          <button type="button" className="ghost danger" onClick={() => run("member.kick", { memberId: m.memberId, ban: true })}>{t("hostBan")}</button>
        </div>
      ))}
      {error && <div className="field-error" data-testid="host-error">{error}</div>}
      {view.room.status !== "closed" && (
        <button type="button" className="danger" onClick={() => confirm(t("hostCloseConfirm")) && run("room.close")}>{t("hostClose")}</button>
      )}
    </div>
  );
}
