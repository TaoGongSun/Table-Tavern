// 只看已落段的故事；三鍵逐鍵合併，面板整份取代。
import { useEffect, useRef } from "react";
import type { SceneStatus, StoryPart } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";
import "./scene-status.css";

type StatusPart = Pick<StoryPart, "state" | "panel">;
const KEYS = ["time", "place", "present"] as const;
const LABELS: Record<(typeof KEYS)[number], MsgKey> = {
  time: "statusTime",
  place: "statusPlace",
  present: "statusPresent",
};

const TRACKING_LABELS = {
  goals: "statusGoals",
  promises: "statusPromises",
  clues: "statusClues",
} as const;

function PanelList({ label, items }: { label: MsgKey; items: string[] }) {
  return (
    <div>
      <strong>{t(label)}</strong>
      <ul>
        {items.map((item, i) => (
          <li key={i}>{item}</li>
        ))}
      </ul>
    </div>
  );
}

/** 戰鬥中的一句：輪到誰，或還沒設定時「戰鬥中」；沒在戰鬥回 null */
export function combatLine(panel: StoryPart["panel"]): string | null {
  const combat = panel?.combat;
  if (!combat?.order.length) return null;
  return combat.turn ? t("statusTurn", { name: combat.turn }) : t("statusInCombat");
}

/** 有鍵就換（含空字串）；缺省沿用上一筆。panel 是整份快照。 */
export function mergedSceneStatus(
  parts: readonly StatusPart[],
): SceneStatus & Pick<StoryPart, "panel"> {
  const out: SceneStatus & Pick<StoryPart, "panel"> = {};
  for (const part of parts) {
    if (part.panel !== undefined) out.panel = part.panel;
    const state = part.state;
    if (!state) continue;
    for (const key of KEYS) if (state[key] !== undefined) out[key] = state[key];
  }
  return out;
}

/** 缺的鍵、空字串、只剩空白都不列 */
export function visibleSceneStatus(parts: readonly StatusPart[]) {
  const status = mergedSceneStatus(parts);
  return KEYS.flatMap((key) => {
    const text = status[key]?.trim();
    return text ? [{ key, label: t(LABELS[key]), text }] : [];
  });
}

export function SceneStatusLine({
  parts,
  open,
  onOpen,
}: {
  parts: readonly StatusPart[];
  open: boolean;
  onOpen: () => void;
}) {
  const items = visibleSceneStatus(parts);
  const panel = mergedSceneStatus(parts).panel;
  const fight = combatLine(panel);
  if (!items.length && !panel) return null;
  return (
    <button
      type="button"
      className="scene-status"
      data-testid="scene-status"
      aria-expanded={open}
      onClick={onOpen}
      aria-label={t("statusPanel")}
    >
      <span className="scene-status-text">
        {[...(fight ? [{ key: "combat", label: "", text: fight }] : []), ...items].map(
          (item, index) => (
            <span
              key={item.key}
              className={`scene-status-item${item.key === "combat" ? " scene-status-combat" : ""}`}
              title={`${item.label} ${item.text}`.trim()}
            >
              {index > 0 && " · "}
              {item.label && <span className="k">{item.label} </span>}
              {item.text}
            </span>
          ),
        )}
        {!items.length && !fight && t("statusPanel")}
      </span>
      <span className="scene-status-caret" aria-hidden="true">
        {open ? "▴" : "▾"}
      </span>
    </button>
  );
}

export function SceneStatusSheet({
  parts,
  onClose,
}: {
  parts: readonly StatusPart[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const outside = (e: PointerEvent) => {
      const target = e.target;
      if (
        target instanceof Element &&
        !ref.current?.contains(target) &&
        !target.closest(".scene-status")
      )
        onClose();
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [onClose]);
  const status = mergedSceneStatus(parts);
  return (
    <section
      ref={ref}
      className="panel scene-status-sheet"
      data-testid="status-sheet"
      role="dialog"
      aria-label={t("statusPanel")}
    >
      <div className="panel-actions">
        <strong>{t("statusPanel")}</strong>
        <button type="button" className="ghost" onClick={onClose}>
          {t("statusClose")}
        </button>
      </div>
      <dl>
        {KEYS.map((key) => (
          <div key={key}>
            <dt>{t(LABELS[key])}</dt>
            <dd>{status[key] || "—"}</dd>
          </div>
        ))}
      </dl>
      {status.panel?.combat && (
        <section className="scene-panel-combat">
          <h3>{t("statusInitiative")}</h3>
          <ol>
            {status.panel.combat.order.map((name) => (
              <li
                key={name}
                className={name === status.panel?.combat?.turn ? "is-turn" : undefined}
              >
                {name}
              </li>
            ))}
          </ol>
          {Object.keys(status.panel.combat.foes).length > 0 && (
            <>
              <h3>{t("statusFoes")}</h3>
              <ul className="scene-panel-foes">
                {Object.entries(status.panel.combat.foes).map(([name, foe]) => (
                  <li key={name}>
                    <strong>{name}</strong>
                    {foe.hp && <span>{foe.hp}</span>}
                    {foe.conditions?.map((item, i) => (
                      <span key={i} className="scene-panel-chip">
                        {item}
                      </span>
                    ))}
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      )}
      {status.panel?.characters && (
        <div className="scene-panel-characters">
          {Object.entries(status.panel.characters).map(([name, cell]) => (
            <section key={name}>
              <h3>{name}</h3>
              {cell.status && <p>{cell.status}</p>}
              {cell.hp && (
                <p>
                  <strong>{t("statusHp")}</strong> {cell.hp}
                </p>
              )}
              {cell.items && <PanelList label="statusItems" items={cell.items} />}
              {cell.conditions && cell.conditions.length > 0 && (
                <PanelList label="statusConditions" items={cell.conditions} />
              )}
            </section>
          ))}
        </div>
      )}
      {status.panel?.tracking && (
        <section>
          <h3>{t("statusTracking")}</h3>
          <div className="scene-panel-tracking">
            {(["goals", "promises", "clues"] as const).map((key) => (
              <PanelList
                key={key}
                label={TRACKING_LABELS[key]}
                items={status.panel?.tracking?.[key] ?? []}
              />
            ))}
          </div>
        </section>
      )}
    </section>
  );
}
