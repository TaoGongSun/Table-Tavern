// 「交給 AI 代管／取消代管」鈕：對象這回合已送出時先確認（原文會退回草稿、不計入），否則一按就生效。
// 一次可以帶好幾位（房主的一鍵代管，只用在交給代管）；本人、房主各處入口共用。
import { useState } from "react";
import { t } from "../../i18n";
import { Confirm } from "./Confirm";
import { cardName, sentThisRound } from "./delegation";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

export function DelegateButton({ view, act, memberIds, on, label, className, testId, onDone }: {
  view: RoomView; act: Act; memberIds: string[]; on: boolean; label: string; className?: string; testId?: string; onDone?: () => void;
}) {
  const { run, error } = useRun(act);
  const [asking, setAsking] = useState(false);
  const sent = on ? memberIds.filter((id) => sentThisRound(view, id)) : [];
  const go = async () => {
    setAsking(false);
    // 好幾位一次送：同一筆交易做完才結算，已送出的人不會先被算進這回合
    const ok = memberIds.length > 1 ? await run("seat.delegate_many", { memberIds }) : await run("seat.delegate", { memberId: memberIds[0], on });
    if (ok) onDone?.();
  };
  const who = (id: string) => (id === view.me.memberId ? null : cardName(view, view.members.find((m) => m.memberId === id)?.cardId ?? null));
  return (
    <>
      <button type="button" className={className} data-testid={testId} onClick={() => (sent.length ? setAsking(true) : void go())}>{label}</button>
      {asking && (
        <Confirm title={t("delegateConfirmTitle")} testId="delegate-confirm" onCancel={() => setAsking(false)} actions={[{ label, onClick: () => void go() }]}>
          {sent.map((id) => <p key={id}>{who(id) === null ? t("delegateRetractSelf") : t("delegateRetractOther", { name: who(id)! })}</p>)}
        </Confirm>
      )}
      {error && <span className="field-error">{error}</span>}
    </>
  );
}
