// 房主收回／重生上一回合（room-host-actions §3.2）：故事串尾的小鈕（照單人版 .story-actions，跟著故事捲）。兩者都先確認「GM 這段會作廢、大家都看得到它消失」，
// 收回就定案、不能復原。出額度者重生時可同時換路線／模型。只收一層：上一回合的段落還在故事串最後才給按（伺服器為準）。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { modelBroken } from "../room/gm-error";
import { RoutePicker } from "./RoutePicker";

export function UndoControls({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [ask, setAsk] = useState<"regenerate" | "edit" | null>(null);
  const r = view.round;
  const last = view.story[view.story.length - 1];
  const target = r?.phase === "held" ? r.round : r?.phase === "acting" ? r.round - 1 : null;
  // held＝這回合本身就是目標（可能一段都還沒寫出來）；acting 要上一回合的段落還在故事串最後
  const acting =
    r?.phase === "acting" && last && last.round === target && last.round > 0 && last.part !== 33;
  if (target === null || (r?.phase !== "held" && !acting)) return null;
  const undo = async (mode: "regenerate" | "edit", next?: { route: string; model: string }) => {
    if (await run("round.undo", { round: target, mode, ...(next ? { next } : {}) })) setAsk(null);
  };
  const offline = !view.executor.online
    ? t("startNeedExecutor")
    : view.executor.syncing
      ? t("roundHostSyncing")
      : null;
  // 卡住（held）時看上次失敗的種類：模型本身用不了就灰掉它；暫時性錯誤原模型照選、提示可以直接重試
  const failed = r?.phase === "held" && r.hold ? r.hold.kind : null;
  const cur = view.room.sceneSettings;
  const broken =
    failed && modelBroken(failed) && cur ? { route: cur.route, model: cur.model } : null;
  // 卡住且 GM 這回合一段都沒寫出來：沒有東西會消失，警語改講會重跑
  const empty =
    r?.phase === "held" &&
    !view.story.some((p) => p.round === r.round && p.part >= 0 && p.part !== 33);
  const warn = !empty
    ? "undoConfirm"
    : ask === "regenerate"
      ? "undoConfirmHeldRegen"
      : "undoConfirmHeldEdit";
  const regen = ask === "regenerate";
  const payer = view.room.payerId === view.me.memberId;
  // 確認區：主按鈕排第一。原模型用不了時主按鈕是換模型重生，停用的原模型鈕排最後
  const same = (
    <button
      type="button"
      className="danger"
      disabled={regen && !!broken}
      onClick={() => undo(ask!)}
    >
      {regen && broken
        ? t("regenSameBroken", { why: t(failed === "model_unavailable" ? "whyGone" : "whyAuth") })
        : t(regen ? "undoRegenerate" : "undoEdit")}
    </button>
  );
  const cancel = (
    <button type="button" className="ghost" onClick={() => setAsk(null)}>
      {t("cancel")}
    </button>
  );
  const picker = regen && payer && (
    <>
      {failed && !broken && <p className="muted">{t("regenTransientNote")}</p>}
      <RoutePicker
        routes={view.routes}
        current={cur}
        broken={broken}
        label={t("undoRegenerateSwitch")}
        onPick={(next) => undo("regenerate", next)}
      />
    </>
  );
  return (
    <div className="story-actions">
      {ask === null ? (
        <span className="panel-actions" data-testid="undo-controls">
          <button
            type="button"
            className="ghost small undo-regen"
            disabled={offline !== null}
            title={offline ?? t("undoRegenerateHint")}
            onClick={() => setAsk("regenerate")}
          >
            {t("undoRegenerate")}
          </button>
          <button
            type="button"
            className="ghost small undo-edit"
            disabled={offline !== null}
            title={offline ?? t("undoEditHint")}
            onClick={() => setAsk("edit")}
          >
            {t("undoEdit")}
          </button>
        </span>
      ) : (
        <div className="undo-confirm" data-testid="undo-confirm">
          <p>{t(warn)}</p>
          {regen && broken ? (
            <>
              {picker}
              <div className="panel-actions">
                {cancel}
                {same}
              </div>
            </>
          ) : (
            <>
              <div className="panel-actions">
                {same}
                {cancel}
              </div>
              {picker}
            </>
          )}
        </div>
      )}
      {error && (
        <div className="field-error" data-testid="undo-error">
          {error}
        </div>
      )}
    </div>
  );
}
