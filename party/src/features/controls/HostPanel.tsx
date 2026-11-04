// 房主面板（協定 §5.1 H 欄）：邀請新朋友／換新連結、房間設定、換幕（自己是出額度者可同時定下一幕路線／模型）、離席處置、
// 到場整理（AI 代管、轉讓、移除、封鎖、指派座位，在 Attendance）、匯出、手機房主的暫停房間。回合級的結算／中止在回合列。
// 結束房間只在桌面版桌卡（房間要能重開，room-host-desktop-play）。
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { Attendance } from "./Attendance";
import { InviteControls, type InviteSource } from "./InviteControls";
import { PauseRoom } from "./PauseRoom";
import { RoutePicker } from "./RoutePicker";

const TIMEOUTS = [null, 60, 120, 300, 900];

export function HostPanel({ view, act, invite, exportRoom, phone, focusAttendance }: {
  view: RoomView; act: Act; invite: InviteSource; exportRoom: (() => Promise<boolean>) | null; phone: boolean; focusAttendance?: boolean;
}) {
  const { run, error, setError } = useRun(act);
  const r = view.round;
  const s = view.room.settings;
  const canAdvance = view.room.status === "playing" && r && (r.phase === "acting" || (r.phase === "held" && r.hold?.kind === "scene_full"));
  const card = (id: string | null) => view.cards.find((c) => c.cardId === id)?.name ?? "";
  return (
    <div className="panel" data-testid="host-panel">
      <h2>{t("hostTitle")}</h2>
      <InviteControls view={view} act={act} src={invite} />
      {exportRoom && (
        <div className="panel-actions">
          <button type="button" onClick={async () => setError((await exportRoom()) ? null : t("errInternal"))}>{t("hostExport")}</button>
        </div>
      )}
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
      <Attendance view={view} act={act} focus={focusAttendance} />
      {phone && view.room.status !== "closed" && <PauseRoom view={view} act={act} />}
      {error && <div className="field-error" data-testid="host-error">{error}</div>}
    </div>
  );
}
