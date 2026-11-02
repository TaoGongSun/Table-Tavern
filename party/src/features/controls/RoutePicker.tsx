// 路線＋模型選擇（協定 §7.3 RouteOffer）：有執行端清單就用下拉；伺服器還沒把清單轉給手機時退回手填 id
// （填錯由伺服器回 E_MODEL_NOT_OFFERED）。只有出額度者看得到這個元件（裁決 1：其他玩家看不到供應商與模型）。
import { useState } from "react";
import type { RouteOffer, SceneSettings } from "../../../server/core/types";
import { t } from "../../i18n";

export interface RouteChoice {
  route: string;
  model: string;
}

export function RoutePicker({ routes, current, label, onPick }: { routes: RouteOffer[] | null; current: SceneSettings | null; label: string; onPick: (c: RouteChoice) => void }) {
  const [c, setC] = useState<RouteChoice>({ route: current?.route ?? routes?.[0]?.route ?? "", model: current?.model ?? "" });
  const offer = routes?.find((r) => r.route === c.route);
  const laxity = (n: number | null) => (n ? ` · ${t("laxity", { n })}` : "");
  return (
    <div className="route-picker">
      {routes ? (
        <>
          <select aria-label={t("payerRoute")} value={c.route} onChange={(e) => setC({ route: e.target.value, model: "" })}>
            {routes.map((r) => <option key={r.route} value={r.route}>{r.label}</option>)}
          </select>
          <select aria-label={t("payerModel")} value={c.model} onChange={(e) => setC({ ...c, model: e.target.value })}>
            <option value="">—</option>
            {offer?.models.map((m) => <option key={m.id} value={m.id}>{m.label + laxity(m.laxity)}</option>)}
          </select>
        </>
      ) : (
        <>
          <input aria-label={t("payerRoute")} value={c.route} onChange={(e) => setC({ ...c, route: e.target.value })} />
          <input aria-label={t("payerModel")} value={c.model} onChange={(e) => setC({ ...c, model: e.target.value })} />
        </>
      )}
      <button type="button" disabled={!c.route || !c.model} onClick={() => onPick(c)}>{label}</button>
    </div>
  );
}
