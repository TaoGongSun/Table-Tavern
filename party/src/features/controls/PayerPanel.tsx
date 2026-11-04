// 出額度者面板（協定 §7）：執行端狀態、額度細節、中止與下一幕設定。卡住時的詳情、重試與換模型在回合列（HeldActions）（重試、換模型、切路線、中止）、下一幕設定。
import type { ReactNode } from "react";
import type { RemoteCommand } from "../../../server/core/types";
import { modelName, t } from "../../i18n";
import { clock } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { RoutePicker } from "./RoutePicker";

const yesNo = (v: boolean | null) => t(v === null ? "unknown" : v ? "yes" : "no");

/** `desktop`：桌面版自己就是執行端，不給「更新額度」「解除配對」（額度在資訊列、交出去走「交給手機」） */
/** `extra`：桌面版塞進來的生圖開關（花這台電腦的額度），放在下一幕設定之上 */
export function PayerPanel({ view, act, desktop = false, extra }: { view: RoomView; act: Act; desktop?: boolean; extra?: ReactNode }) {
  const { run, error, setError } = useRun(act);
  const x = view.executor;
  const q = view.quotaDetail;
  const r = view.round;
  // 遠端控制的回覆是 ok { result: { ok, error? } }（§14-8）；執行端拒絕時顯示它在地化的說明
  const ctl = async (cmd: RemoteCommand) => {
    const res = await run("ctl", { ctlId: crypto.randomUUID(), cmd });
    if (res && !res.ok) setError(res.error?.message ?? t("errExecutorRefused"));
  };
  return (
    <div className="panel" data-testid="payer-panel">
      <h2>{t("payerTitle")}</h2>
      <dl className="facts">
        <dt>{t("payerExecutor")}</dt>
        <dd>{x.label} · {t(x.online ? "online" : "offline")}</dd>
        <dt>{t("payerKeepAwake")}</dt>
        <dd>{yesNo(x.keepAwake)}</dd>
        <dt>{t("payerNextModel")}</dt>
        <dd>{x.modelLabel ? modelName(x.modelLabel) : "—"}</dd>
        {q && (
          <>
            <dt>{t("payerQuota")}</dt>
            <dd data-testid="quota">
              {q.routeLabel} · {t(`quota_${q.state}`)}
              {q.usedRatio !== null && ` · ${Math.round(q.usedRatio * 100)}%`}
              {q.creditsLeft && ` · ${q.creditsLeft}`}
              {q.resetsAt && ` · ${t("quotaResets", { time: clock(q.resetsAt) })}`}
            </dd>
          </>
        )}
      </dl>
      <div className="panel-actions">
        {r?.phase === "generating" && <button type="button" onClick={() => ctl({ kind: "abort" })}>{t("roundAbort")}</button>}
        {!desktop && <button type="button" className="ghost" onClick={() => ctl({ kind: "refresh_quota" })}>{t("payerRefreshQuota")}</button>}
      </div>
      {extra}
      <h3>{t("payerSetNext")}</h3>
      <RoutePicker routes={view.routes} current={view.room.sceneSettings} label={t("payerSetNextApply")} onPick={(c) => ctl({ kind: "set_next", ...c })} />
      {error && <div className="field-error" data-testid="payer-error">{error}</div>}
      {!desktop && <button type="button" className="ghost danger" onClick={() => run("pair.leave")}>{t("payerUnpair")}</button>}
    </div>
  );
}
