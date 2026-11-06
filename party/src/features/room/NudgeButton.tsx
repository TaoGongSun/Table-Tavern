// 房主催促一位還沒送出的玩家（room-speak-request §1）：伺服器同一對象 30 秒一次，按過或被擋都顯示「已提醒」到冷卻結束。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { Act } from "./useRoom";

export function NudgeButton({
  act,
  round,
  memberId,
}: {
  act: Act;
  round: number;
  memberId: string;
}) {
  const [until, setUntil] = useState(0);
  useEffect(() => {
    if (!until) return;
    const id = setTimeout(() => setUntil(0), Math.max(0, until - Date.now()));
    return () => clearTimeout(id);
  }, [until]);
  const go = async () => {
    const reply = await act("host.nudge", { round, memberId });
    if (reply.ok) setUntil(Date.now() + 30_000);
    else if (reply.code === "E_RATE_LIMITED") setUntil(Date.now() + (reply.retryAfterMs ?? 30_000));
  };
  return (
    <button
      type="button"
      className="ghost small"
      disabled={until > 0}
      onClick={() => void go()}
      data-testid={`nudge-${memberId}`}
    >
      {t(until ? "nudgeDone" : "nudge")}
    </button>
  );
}
