// 出額度者面板（協定 §7）：執行端狀態、額度細節、最近錯誤與可做的遠端動作（重試、換模型、切路線、中止）、下一幕設定。
import type { RemoteCommand } from "../../../server/core/types";
import { t } from "../../i18n";
import { clock } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { RoutePicker } from "./RoutePicker";

const yesNo = (v: boolean | null) => t(v === null ? "unknown" : v ? "yes" : "no");

export function PayerPanel({ view, act, resumed }: { view: RoomView; act: Act; resumed: (round: number) => void }) {
  const { run, error, setError } = useRun(act);
  const x = view.executor;
  const q = view.quotaDetail;
  const r = view.round;
  const held = r?.phase === "held" ? r : null;
  const detail = held && view.errorDetail?.round === held.round ? view.errorDetail.error : null;
  const actions = held?.hold?.actions ?? [];
  // 遠端控制的回覆是 ok { result: { ok, error? } }（§14-8）；執行端拒絕時顯示它在地化的說明
  const ctl = async (cmd: RemoteCommand) => {
    const res = await run("ctl", { ctlId: crypto.randomUUID(), cmd });
    if (res && !res.ok) setError(res.error?.message ?? t("errExecutorRefused"));
    if (res?.ok && held && cmd.kind !== "set_next" && cmd.kind !== "refresh_quota") resumed(held.round);
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
        <dd>{x.modelLabel ?? "—"}</dd>
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
      {detail && (
        <div className="error-detail" data-testid="error-detail">
          <strong>{detail.code}</strong> {detail.message}
        </div>
      )}
      <div className="panel-actions">
        {held && actions.includes("retry") && <button type="button" className="primary" onClick={() => ctl({ kind: "retry" })}>{t("payerRetry")}</button>}
        {r?.phase === "generating" && <button type="button" onClick={() => ctl({ kind: "abort" })}>{t("roundAbort")}</button>}
        <button type="button" className="ghost" onClick={() => ctl({ kind: "refresh_quota" })}>{t("payerRefreshQuota")}</button>
      </div>
      {held && (actions.includes("switch_route") || actions.includes("switch_model")) && (
        <>
          <h3>{t("payerSwitchNow")}</h3>
          <RoutePicker
            routes={view.routes}
            current={view.room.sceneSettings}
            label={t("payerSwitchApply")}
            onPick={(c) => ctl(c.route === view.room.sceneSettings?.route ? { kind: "switch_model", model: c.model } : { kind: "switch_route", ...c })}
          />
        </>
      )}
      <h3>{t("payerSetNext")}</h3>
      <RoutePicker routes={view.routes} current={view.room.sceneSettings} label={t("payerSetNextApply")} onPick={(c) => ctl({ kind: "set_next", ...c })} />
      {error && <div className="field-error" data-testid="payer-error">{error}</div>}
      <button type="button" className="ghost danger" onClick={() => run("pair.leave")}>{t("payerUnpair")}</button>
    </div>
  );
}
