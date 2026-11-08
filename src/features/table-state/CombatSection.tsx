// 戰鬥區（ttrpg-combat-tracker §3）：先攻一列（↑↓ 調順序、點名字設輪到誰）、無卡敵人小格、拒收標記與戰鬥結束提示。
// 寫入一律走既有指令：順序 set_panel_list、輪到誰與 HP set_state_path。
import { type ReactNode } from "react";
import { t } from "../../i18n";
import { type StateNode } from "../../shared/contracts/backend-contracts";
import { type RefusedMark, type RefusedMarks } from "./useTableStateController";
import "./combat.css";

type Branch = Record<string, StateNode>;
const branch = (node: StateNode | undefined): Branch =>
  typeof node === "object" && node !== null ? node : {};

/** 清單分支照索引排成字串 */
export function listValues(node: StateNode | undefined): string[] {
  return Object.entries(branch(node))
    .filter(([key, value]) => /^\d+$/.test(key) && typeof value === "string")
    .sort(([a], [b]) => Number(a) - Number(b))
    .map(([, value]) => value as string);
}

/** 收合列的戰鬥段：沒有先攻順序回 null；有順序沒輪到誰顯示「戰鬥中」 */
export function combatSummary(tree: Branch): string | null {
  const combat = branch(tree.combat);
  if (listValues(combat.order).length === 0) return null;
  const turn = typeof combat.turn === "string" ? combat.turn : "";
  return turn ? t("statePanelTurn", { name: turn }) : t("statePanelInCombat");
}

const WHY = {
  no_order: "statePanelWhyNoOrder",
  duplicate: "statePanelWhyDuplicate",
  too_many: "statePanelWhyTooMany",
  bad_hp: "statePanelWhyBadHp",
  bad_name: "statePanelWhyBadName",
  missing: "statePanelWhyMissing",
  not_in_order: "statePanelWhyNotInOrder",
} as const;
const ADD_REASONS = new Set(["no_order", "duplicate", "too_many", "bad_hp", "bad_name"]);

/** 拒收的白話句：敵人與輪到誰寫名字＋原因，其餘寫 GM 寫了什麼 */
function refusedText(path: string, mark: RefusedMark): string {
  const reason = t(mark.reason && mark.reason in WHY ? WHY[mark.reason as keyof typeof WHY] : "statePanelWhyBadOp");
  const foe = /^combat\.foes\.([^.]+)/.exec(path)?.[1];
  if (foe)
    return t(ADD_REASONS.has(mark.reason ?? "") ? "statePanelFoeRefused" : "statePanelFoeNotUpdated", {
      name: foe,
      reason,
    });
  if (path === "combat.turn" && mark.reason) return t("statePanelTurnRefused", { name: mark.wrote ?? "", reason });
  return mark.wrote ? t("statePanelRefused", { wrote: mark.wrote }) : t("statePanelRefusedUnknown");
}

/** 拒收標記：真的沒套上的帶 ⚠ 與警告色；依骰面改套的是灰色說明（已套用，不算錯）。`path` 給了就能寫成白話原因 */
export function RefusedBadge({
  mark,
  path = "",
  onClick,
}: {
  mark: RefusedMark | undefined;
  path?: string;
  onClick?: () => void;
}) {
  if (!mark) return null;
  if (mark.applied)
    return (
      <span className="state-dice-applied">
        {t("statePanelRefusedApplied", { wrote: mark.wrote ?? "", applied: mark.applied })}
      </span>
    );
  const text = "⚠ " + refusedText(path, mark);
  return onClick ? (
    <button type="button" className="state-refused" onClick={onClick}>
      {text}
    </button>
  ) : (
    <span className="state-refused">{text}</span>
  );
}

export function CombatSection({
  tree,
  ended,
  refused,
  renderLeaf,
  renderList,
  onSetOrder,
  onSetTurn,
}: {
  tree: Branch;
  /** 這一幕剛由 GM 收場 */
  ended: boolean;
  refused: RefusedMarks;
  renderLeaf: (path: string[], label: string) => ReactNode;
  renderList: (node: StateNode | undefined, path: string[], label: string) => ReactNode;
  onSetOrder?: (order: string[]) => void;
  onSetTurn?: (name: string) => void;
}) {
  const combat = branch(tree.combat);
  const order = listValues(combat.order);
  const turn = typeof combat.turn === "string" ? combat.turn : "";
  // 沒有格可掛的拒收（先攻、輪到誰、敵人整筆、不存在的敵人）：不論在不在戰鬥都要看得到；同一隻敵人同一句只留一行
  const foes = branch(combat.foes);
  const seen = new Set<string>();
  const loose = Object.entries(refused)
    .filter(([path]) => {
      const parts = path.split(".");
      if (parts[0] !== "combat") return false;
      if (parts[1] === "foes" && parts.length > 3 && parts[2] in foes) return false;
      const key = `${parts[2] ?? path}|${refused[path].reason ?? path}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .map(([path, mark]) => (
      <p key={path}>
        <RefusedBadge mark={mark} path={path} />
      </p>
    ));
  if (order.length === 0) {
    if (!ended && loose.length === 0) return null;
    return (
      <div className="state-combat-idle">
        {ended && <p className="state-combat-ended">{t("statePanelCombatEnded")}</p>}
        {loose}
      </div>
    );
  }
  const move = (index: number, step: -1 | 1) => {
    const next = [...order];
    [next[index], next[index + step]] = [next[index + step], next[index]];
    onSetOrder?.(next);
  };
  return (
    <section className="state-panel-cell state-combat" aria-label={t("statePanelCombat")}>
      <h3>{t("statePanelCombat")}</h3>
      <section aria-label={t("statePanelInitiative")}>
        <h5>{t("statePanelInitiative")}</h5>
        <ol className="state-combat-order">
          {order.map((name, index) => (
            <li key={name} className={name === turn ? "is-turn" : undefined} aria-current={name === turn ? "true" : undefined}>
              <span className="state-combat-index">{index + 1}.</span>
              <button type="button" className="state-combat-name" onClick={() => onSetTurn?.(name)}>
                {name}
              </button>
              <button
                type="button"
                aria-label={`${t("statePanelMoveUp")} ${name}`}
                disabled={index === 0 || !onSetOrder}
                onClick={() => move(index, -1)}
              >
                ↑
              </button>
              <button
                type="button"
                aria-label={`${t("statePanelMoveDown")} ${name}`}
                disabled={index === order.length - 1 || !onSetOrder}
                onClick={() => move(index, 1)}
              >
                ↓
              </button>
            </li>
          ))}
        </ol>
        {!turn && <p className="state-combat-hint">{t("statePanelSetTurnHint")}</p>}
      </section>
      {loose}
      {Object.keys(branch(combat.foes)).length > 0 && (
        <section aria-label={t("statePanelFoes")}>
          <h5>{t("statePanelFoes")}</h5>
          <div className="state-combat-foes">
            {Object.entries(branch(combat.foes)).map(([name, foe]) => (
              <section className="state-panel-cell" key={name} aria-label={name}>
                <h4>{name}</h4>
                {renderLeaf(["combat", "foes", name, "hp"], t("statePanelHp"))}
                {renderList(branch(foe).conditions, ["combat", "foes", name, "conditions"], t("statePanelConditions"))}
              </section>
            ))}
          </div>
        </section>
      )}
    </section>
  );
}
