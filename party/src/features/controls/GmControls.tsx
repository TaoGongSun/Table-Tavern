// 房主的 GM 鈕（room-host-actions §2）：GM 旁白、GM 推進、請某張 NPC 發言、「讓 X 接話」。照單人版放在故事串與行動格之間，
// 沒座位的房主也看得到。這一回合已送出的行動會退回草稿；行動中且 GM 主機在線才能按，否則停用並寫原因。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function GmControls({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [cardId, setCardId] = useState("");
  const [help, setHelp] = useState(false);
  const r = view.round;
  const npcs = view.cards.filter((c) => c.playable && !c.hidden && c.claimedBy === null && !c.reserved);
  const offline = !view.executor.online;
  const blocked = offline ? t("startNeedExecutor") : r?.phase !== "acting" ? t("gmBusy") : null;
  const gm = (mode: string, extra: Record<string, unknown> = {}) => run("host.gm", { round: r?.round ?? 0, mode, ...extra });
  const called = r?.called ? npcs.find((c) => c.cardId === r.called) : undefined;
  const picked = npcs.find((c) => c.cardId === cardId) ?? npcs[0];
  return (
    <>
      <span className="panel-actions" data-testid="gm-controls">
        {called && (
          <button type="button" className="primary" disabled={blocked !== null} title={blocked ?? t("callNominatedHint", { name: called.name })} onClick={() => gm("call", { cardId: called.cardId })} data-testid="gm-call">
            {t("callNominated", { name: called.name })}
          </button>
        )}
        <button type="button" disabled={blocked !== null} title={blocked ?? t("gmNarrateNote")} onClick={() => gm("narrate")} data-testid="gm-narrate">{t("gmNarrate")}</button>
        <button type="button" disabled={blocked !== null} title={blocked ?? t("gmAdvanceNote")} onClick={() => gm("advance")} data-testid="gm-advance">{t("gmAdvance")}</button>
        {picked && (
          <>
            <select aria-label={t("requestReplyPick")} value={picked.cardId} onChange={(e) => setCardId(e.target.value)} data-testid="gm-reply-card">
              {npcs.map((c) => <option key={c.cardId} value={c.cardId}>{c.name}</option>)}
            </select>
            <button type="button" disabled={blocked !== null} title={blocked ?? t("requestReplyHint")} onClick={() => gm("call", { cardId: picked.cardId })} data-testid="gm-reply">{t("requestReply")}</button>
          </>
        )}
      </span>
      {/* 整個操作區最右邊：整列的說明 */}
      <button type="button" className="ghost gm-help" aria-label={t("gmHelp")} aria-expanded={help} title={t("gmHelp")} onClick={() => setHelp(!help)} data-testid="gm-help">?</button>
      {/* 各鈕的 tooltip 之外，點「?」才列出全部說明（不常駐，故事串留空間） */}
      {help && (
        <div className="action-hint" data-testid="gm-notes">
          {[t("gmNarrateNote"), t("gmAdvanceNote"), t("requestReplyHint"), t("gmNoResolveNote")].map((line) => <div key={line}>{line}</div>)}
        </div>
      )}
      {offline && <div className="action-hint">{blocked}</div>}
      {error && <div className="field-error" data-testid="gm-error">{error}</div>}
    </>
  );
}
