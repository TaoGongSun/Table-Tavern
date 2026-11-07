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
  if (!items.length && !mergedSceneStatus(parts).panel) return null;
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
        {items.length
          ? items.map((item, index) => (
              <span
                key={item.key}
                className="scene-status-item"
                title={`${item.label} ${item.text}`}
              >
                {index > 0 && " · "}
                <span className="k">{item.label} </span>
                {item.text}
              </span>
            ))
          : t("statusPanel")}
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
      {status.panel?.characters && (
        <div className="scene-panel-characters">
          {Object.entries(status.panel.characters).map(([name, cell]) => (
            <section key={name}>
              <h3>{name}</h3>
              {cell.status && <p>{cell.status}</p>}
              {cell.items && <PanelList label="statusItems" items={cell.items} />}
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
