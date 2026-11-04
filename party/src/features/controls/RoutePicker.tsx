// 路線＋模型選擇（協定 §7.3 RouteOffer）：有執行端清單就用下拉；伺服器還沒把清單轉給手機時退回手填 id
// （填錯由伺服器回 E_MODEL_NOT_OFFERED）。只有出額度者看得到這個元件（裁決 1：其他玩家看不到供應商與模型）。
import { useState } from "react";
import type { RouteOffer, SceneSettings } from "../../../server/core/types";
import { modelName, t } from "../../i18n";

export interface RouteChoice {
  route: string;
  model: string;
}

/** `broken`＝上次失敗、這個模型本身用不了（重生時）：灰掉它，預設改選清單裡第一個能用的 */
export function RoutePicker({ routes, current, label, onPick, broken }: { routes: RouteOffer[] | null; current: SceneSettings | null; label: string; onPick: (c: RouteChoice) => void; broken?: RouteChoice | null }) {
  const bad = (route: string, model: string) => !!broken && broken.route === route && broken.model === model;
  const [c, setC] = useState<RouteChoice>(() => {
    const start = { route: current?.route ?? routes?.[0]?.route ?? "", model: current?.model ?? "" };
    if (!bad(start.route, start.model)) return start;
    for (const r of routes ?? []) for (const m of r.models) if (!bad(r.route, m.id)) return { route: r.route, model: m.id };
    return { route: start.route, model: "" };
  });
  const offer = routes?.find((r) => r.route === c.route);
  const laxity = (n: number | null) => (n ? ` · ${t("laxity", { n })}` : "");
  return (
    <div className="route-picker">
      {routes ? (
        <>
          <label className="picker-label">{t("payerRoute")}</label>
          <select aria-label={t("payerRoute")} value={c.route} onChange={(e) => setC({ route: e.target.value, model: "" })}>
            {routes.map((r) => <option key={r.route} value={r.route}>{r.label}</option>)}
          </select>
          <label className="picker-label">{t("payerModel")}</label>
          <select aria-label={t("payerModel")} value={c.model} onChange={(e) => setC({ ...c, model: e.target.value })}>
            <option value="">—</option>
            {offer?.models.map((m) => (
              <option key={m.id} value={m.id} disabled={bad(c.route, m.id)}>
                {modelName(m.label) + laxity(m.laxity) + (bad(c.route, m.id) ? `（${t("modelFailedLast")}）` : "")}
              </option>
            ))}
          </select>
        </>
      ) : (
        <>
          <input aria-label={t("payerRoute")} value={c.route} onChange={(e) => setC({ ...c, route: e.target.value })} />
          <input aria-label={t("payerModel")} value={c.model} onChange={(e) => setC({ ...c, model: e.target.value })} />
        </>
      )}
      <button type="button" disabled={!c.route || !c.model || bad(c.route, c.model)} onClick={() => onPick(c)}>{label}</button>
    </div>
  );
}
