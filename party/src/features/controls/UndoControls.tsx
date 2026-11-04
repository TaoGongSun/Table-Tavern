// 房主收回／重生上一回合（room-host-actions §3.2）：放在故事串最後一段下方。兩者都先確認「GM 這段會作廢、大家都看得到它消失」，
// 收回就定案、不能復原。出額度者重生時可同時換路線／模型。只收一層：上一回合的段落還在故事串最後才給按（伺服器為準）。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { holdKind, modelBroken } from "../room/gm-error";
import { RoutePicker } from "./RoutePicker";

export function UndoControls({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [ask, setAsk] = useState<"regenerate" | "edit" | null>(null);
  const r = view.round;
  const last = view.story[view.story.length - 1];
  const target = r?.phase === "held" ? r.round : r?.phase === "acting" ? r.round - 1 : null;
  // held＝這回合本身就是目標（可能一段都還沒寫出來）；acting 要上一回合的段落還在故事串最後
  const acting = r?.phase === "acting" && last && last.round === target && last.round > 0 && last.part !== 33;
  if (target === null || (r?.phase !== "held" && !acting)) return null;
  const undo = async (mode: "regenerate" | "edit", next?: { route: string; model: string }) => {
    if (await run("round.undo", { round: target, mode, ...(next ? { next } : {}) })) setAsk(null);
  };
  const offline = !view.executor.online;
  // 卡住（held）時看上次失敗的種類：模型本身用不了就灰掉它；暫時性錯誤原模型照選、提示可以直接重試
  const failed = r?.phase === "held" && r.hold ? holdKind(r.hold.kind) : null;
  const cur = view.room.sceneSettings;
  const broken = failed && modelBroken(failed) && cur ? { route: cur.route, model: cur.model } : null;
  // 卡住且 GM 這回合一段都沒寫出來：沒有東西會消失，警語改講會重跑
  const empty = r?.phase === "held" && !view.story.some((p) => p.round === r.round && p.part >= 0 && p.part !== 33);
  const warn = !empty ? "undoConfirm" : ask === "regenerate" ? "undoConfirmHeldRegen" : "undoConfirmHeldEdit";
  return (
    <>
      {ask === null ? (
        <span className="panel-actions" data-testid="undo-controls">
          <button type="button" className="ghost" disabled={offline} title={offline ? t("startNeedExecutor") : t("undoRegenerateHint")} onClick={() => setAsk("regenerate")}>{t("undoRegenerate")}</button>
          <button type="button" className="ghost" disabled={offline} title={offline ? t("startNeedExecutor") : t("undoEditHint")} onClick={() => setAsk("edit")}>{t("undoEdit")}</button>
        </span>
      ) : (
        <div className="undo-confirm" data-testid="undo-confirm">
          <p>{t(warn)}</p>
          <div className="panel-actions">
            <button type="button" className="danger" disabled={ask === "regenerate" && !!broken} onClick={() => undo(ask)}>{ask === "regenerate" && broken ? t("regenSameBroken", { why: t(failed === "gone" ? "whyGone" : "whyAuth") }) : t(ask === "regenerate" ? "undoRegenerate" : "undoEdit")}</button>
            <button type="button" className="ghost" onClick={() => setAsk(null)}>{t("cancel")}</button>
          </div>
          {ask === "regenerate" && view.room.payerId === view.me.memberId && (
            <>
              {failed && !broken && <p className="muted">{t("regenTransientNote")}</p>}
              <RoutePicker routes={view.routes} current={cur} broken={broken} label={t("undoRegenerateSwitch")} onPick={(next) => undo("regenerate", next)} />
            </>
          )}
        </div>
      )}
      {error && <div className="field-error" data-testid="undo-error">{error}</div>}
    </>
  );
}
