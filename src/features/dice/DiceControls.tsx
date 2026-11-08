import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  TranscriptEvent,
} from "../../shared/contracts/backend-contracts";
import { eventDisplayText } from "../../shared/ui/event-text";
import { diceCopy, playerTurn } from "./dice-ui";
import "./dice-pool.css";

export function DiceControls({
  worldId,
  events,
  diceEnabled,
  busy,
  locked,
  room,
  hasTarget,
  playerName,
  onRoll,
  onExpand,
}: {
  worldId: string;
  events: TranscriptEvent[];
  diceEnabled: boolean;
  busy: boolean;
  locked: boolean;
  room: boolean;
  playerName?: string;
  hasTarget: boolean;
  onRoll: (formula: string) => Promise<void>;
  onExpand?: () => void;
}) {
  const c = diceCopy();
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => { if (open) onExpand?.(); }, [open, onExpand]);
  const [options, setOptions] = useState<string[]>([]);
  const [count, setCount] = useState("1");
  const [sides, setSides] = useState("20");
  const [modifier, setModifier] = useState("0");
  const pending = useRef(false);
  const [rolling, setRolling] = useState(false);
  const gm = [...events].reverse().find((e) => e.kind === "narration");
  const latest = gm ? eventDisplayText(gm) : "";
  useEffect(() => {
    let stale = false;
    setOptions([]);
    void invoke<string[]>("dice_suggestions", { text: latest })
      .then((value) => {
        if (!stale) setOptions(value);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [latest, worldId]);
  const reason = locked
    ? c.locked
    : room
      ? c.room
      : busy || rolling
        ? c.busy
        : !diceEnabled
          ? c.off
          : !playerTurn(events, playerName)
            ? c.turn
            : !hasTarget
              ? c.target
              : "";
  async function roll(formula: string) {
    if (reason || pending.current) return;
    pending.current = true;
    setRolling(true);
    setOpen(false);
    try {
      await onRoll(formula);
    } finally {
      pending.current = false;
      setRolling(false);
    }
  }
  return (
    <div className="dice-controls">
      <button
        type="button"
        className="btn btn-sm"
        disabled={Boolean(reason)}
        title={reason || c.roll}
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        ⚄ {c.roll}
      </button>
      {reason && <small role="status">{reason}</small>}
      {open && (
        <div className="dice-options">
          {options.map((value) => (
            <button
              type="button"
              className="btn btn-sm dice-option"
              disabled={Boolean(reason)}
              key={value}
              onClick={() => void roll(value)}
            >
              {c.rollOption.replace("{formula}", value)}
            </button>
          ))}
          <div className="dice-custom" onKeyDown={(event) => {
            if (event.key === "Enter" && event.target instanceof HTMLInputElement) event.preventDefault();
          }}>
            {(
              [
                [c.count, count, setCount, 1, 100],
                [c.sides, sides, setSides, 2, 1000],
                [c.modifier, modifier, setModifier, -9999, 9999],
              ] as const
            ).map(([label, value, change, min, max]) => (
              <label key={label}>
                {label}
                <input
                  type="number"
                  min={min}
                  max={max}
                  step="1"
                  value={value}
                  disabled={Boolean(reason)}
                  onChange={(e) => change(e.target.value)}
                />
              </label>
            ))}
            <button
              type="button"
              className="btn btn-sm"
              disabled={Boolean(reason)}
              onClick={() =>
                void roll(
                  `${+count}d${+sides}${+modifier ? `${+modifier > 0 ? "+" : ""}${+modifier}` : ""}`,
                )
              }
            >
              {c.rollCustom}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
