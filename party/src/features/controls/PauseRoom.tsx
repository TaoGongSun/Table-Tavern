// 手機房主的「暫停房間」（room-seat-delegate §2.3）：先確認，按下就停用（免得連按看到錯誤）；電腦走跟桌面「關房（暫停）」同一條流程。
// 電腦離線時房間本來就停著，鈕停用並說明。桌面版不放（資訊列已有「關房（暫停）」）。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Confirm } from "../room/Confirm";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function PauseRoom({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [asking, setAsking] = useState(false);
  const [sent, setSent] = useState(false);
  const offline = !view.executor.online;
  useEffect(() => {
    if (view.executor.online) setSent(false); // 電腦重開了：下次又能暫停
  }, [view.executor.online]);
  const pause = async () => {
    setAsking(false);
    setSent(true);
    if (!(await run("ctl", { ctlId: crypto.randomUUID(), cmd: { kind: "pause" } }))) setSent(false);
  };
  return (
    <div className="panel-actions pause-room">
      <button type="button" className="danger" disabled={offline || sent} data-testid="pause-room" onClick={() => setAsking(true)}>{t("hostPause")}</button>
      {offline && <span className="muted">{t("hostPauseOffline")}</span>}
      {asking && (
        <Confirm title={t("hostPauseTitle")} testId="pause-confirm" onCancel={() => setAsking(false)} actions={[{ label: t("hostPause"), onClick: () => void pause(), danger: true }]}>
          <p>{t("hostPauseBody", { label: view.executor.label })}</p>
        </Confirm>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
