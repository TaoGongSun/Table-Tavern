// 「交給 AI 代管／取消代管」：對象這回合已送出時先確認（原文會退回草稿、不計入），否則一按就生效。
// 送出與確認框在 useDelegate，狀態列的選單與各處的鈕共用；一次可以帶好幾位（房主的一鍵代管）。
import { useRef, useState, type ReactNode } from "react";
import { t } from "../../i18n";
import { Confirm } from "./Confirm";
import { cardName, sentThisRound } from "./delegation";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

export function useDelegate(view: RoomView, act: Act) {
  const { run, error } = useRun(act);
  const [asking, setAsking] = useState(false);
  const req = useRef<{
    memberIds: string[];
    on: boolean;
    label: string;
    onDone?: () => void;
  } | null>(null);

  const go = async () => {
    const current = req.current;
    if (!current) return;
    setAsking(false);
    // 好幾位一次送：同一筆交易做完才結算，已送出的人不會先被算進這回合
    const ok =
      current.memberIds.length > 1
        ? await run("seat.delegate_many", { memberIds: current.memberIds })
        : await run("seat.delegate", { memberId: current.memberIds[0], on: current.on });
    if (ok) current.onDone?.();
  };

  const start = (memberIds: string[], on: boolean, label?: string, onDone?: () => void) => {
    req.current = { memberIds, on, label: label ?? t(on ? "delegateOn" : "delegateOff"), onDone };
    const sent = on ? memberIds.filter((id) => sentThisRound(view, id)) : [];
    if (sent.length) setAsking(true);
    else void go();
  };

  const who = (id: string) =>
    id === view.me.memberId
      ? null
      : cardName(view, view.members.find((m) => m.memberId === id)?.cardId ?? null);
  const sent =
    asking && req.current?.on ? req.current.memberIds.filter((id) => sentThisRound(view, id)) : [];
  const confirm: ReactNode = asking && req.current && (
    <Confirm
      title={t("delegateConfirmTitle")}
      testId="delegate-confirm"
      onCancel={() => setAsking(false)}
      actions={[{ label: req.current.label, onClick: () => void go() }]}
    >
      {sent.map((id) => (
        <p key={id}>
          {who(id) === null
            ? t("delegateRetractSelf")
            : t("delegateRetractOther", { name: who(id)! })}
        </p>
      ))}
    </Confirm>
  );

  return { start, confirm, error };
}

export function DelegateButton({
  view,
  act,
  memberIds,
  on,
  label,
  className,
  testId,
  onDone,
}: {
  view: RoomView;
  act: Act;
  memberIds: string[];
  on: boolean;
  label: string;
  className?: string;
  testId?: string;
  onDone?: () => void;
}) {
  const { start, confirm, error } = useDelegate(view, act);
  return (
    <>
      <button
        type="button"
        className={className}
        data-testid={testId}
        onClick={() => start(memberIds, on, label, onDone)}
      >
        {label}
      </button>
      {confirm}
      {error && <span className="field-error">{error}</span>}
    </>
  );
}
