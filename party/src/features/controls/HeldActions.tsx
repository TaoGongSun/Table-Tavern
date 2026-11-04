// GM 卡住（held）時出額度者的遠端動作（協定 §7.3）：重試、換模型，放在回合列狀態句旁（room-desktop-layout §2-5）。
// 只出伺服器 hold.actions 允許的鈕；換模型是錨在鈕下的小彈層（不是選單：裡面有兩個下拉）。
import { useState } from "react";
import type { RemoteCommand } from "../../../server/core/types";
import { t } from "../../i18n";
import { modelBroken } from "../room/gm-error";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { RoutePicker } from "./RoutePicker";

export function HeldActions({ view, act, resumed }: { view: RoomView; act: Act; resumed: (round: number) => void }) {
  const { run, error, setError } = useRun(act);
  const [open, setOpen] = useState(false);
  const r = view.round;
  if (r?.phase !== "held" || !r.hold || view.room.payerId !== view.me.memberId) return null;
  const actions = r.hold.actions;
  const cur = view.room.sceneSettings;
  const broken = modelBroken(r.hold.kind) && cur ? { route: cur.route, model: cur.model } : null;
  // 遠端控制的回覆是 ok { result: { ok, error? } }（§14-8）；執行端拒絕時顯示它在地化的說明
  const ctl = async (cmd: RemoteCommand) => {
    const res = await run("ctl", { ctlId: crypto.randomUUID(), cmd });
    if (res && !res.ok) setError(res.error?.message ?? t("errExecutorRefused"));
    if (res?.ok) (setOpen(false), resumed(r.round));
  };
  // 送哪個命令照伺服器允許的動作：只允許換路線時同一條路線也送 switch_route；只允許換模型時只能選同一條路線
  const byRoute = actions.includes("switch_route");
  const byModel = actions.includes("switch_model");
  const canSwitch = byRoute || byModel;
  const routes = byRoute ? view.routes : (view.routes?.filter((x) => x.route === cur?.route) ?? null);
  return (
    <>
      {actions.includes("retry") && (
        // 模型本身用不了時重試沒用：主按鈕讓給換模型
        <button type="button" className={broken ? undefined : "primary"} onClick={() => void ctl({ kind: "retry" })}>{t("payerRetry")}</button>
      )}
      {canSwitch && (
        <span className="held-switch-anchor" onKeyDown={(e) => e.key === "Escape" && setOpen(false)}>
          <button type="button" className={broken ? "primary" : undefined} aria-expanded={open} onClick={() => setOpen(!open)} data-testid="held-switch">
            {t("heldSwitch")} ▾
          </button>
          {open && (
            <div className="menu held-switch">
              <RoutePicker
                routes={routes}
                current={cur}
                broken={broken}
                label={t("payerSwitchApply")}
                onPick={(c) => void ctl(byModel && c.route === cur?.route ? { kind: "switch_model", model: c.model } : { kind: "switch_route", ...c })}
              />
            </div>
          )}
        </span>
      )}
      {error && <span className="field-error" data-testid="held-error">{error}</span>}
    </>
  );
}
