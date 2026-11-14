// 擲骰鈕（桌面與房間共用，不碰 Tauri）：有建議時主鈕一鍵擲第一個，小箭頭展開其他建議與自訂；沒建議就是「擲骰」展開。
import { useLayoutEffect, useRef, useState } from "react";
import type copy from "./dice-copy.json";
import "./dice-pool.css";

export function DicePicker({
  copy: c,
  options,
  primary = options[0] ?? null,
  rolled = [],
  reason,
  quiet = false,
  buttonClass,
  optionClass = buttonClass,
  onRoll,
  onExpand,
  onFixReason,
}: {
  copy: (typeof copy)["en"];
  options: string[];
  /** 主鈕一鍵擲的算式；null＝主鈕是通用「擲骰」展開。預設第一個建議 */
  primary?: string | null;
  /** 這回合擲過的算式：展開列標 ✓，還能再擲 */
  rolled?: string[];
  /** 不能擲的理由；空字串＝可以擲 */
  reason: string;
  /** 手機那條小鈕列放不下常駐理由：按了才顯示 */
  quiet?: boolean;
  buttonClass?: string;
  optionClass?: string;
  onRoll: (formula: string) => Promise<void>;
  onExpand?: () => void;
  /** 有就把不能擲的理由做成可點的連結，直接開到能解決它的地方 */
  onFixReason?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [tapped, setTapped] = useState("");
  const [count, setCount] = useState("1");
  const [sides, setSides] = useState("20");
  const [modifier, setModifier] = useState("0");
  const [rolling, setRolling] = useState(false);
  const pending = useRef(false);
  useLayoutEffect(() => {
    if (open) onExpand?.();
  }, [open, onExpand]);
  const blocked = reason || (rolling ? c.busy : "");
  const off = Boolean(blocked);
  async function roll(formula: string) {
    if (off || pending.current) return;
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
  /** quiet 時不用原生 disabled：要按得到才能告訴玩家為什麼不能擲 */
  const guard = (fn: () => void) => () => {
    if (quiet && reason) setTapped(reason);
    else fn();
  };
  const offProps = quiet ? { "aria-disabled": off || undefined } : { disabled: off };
  const first = primary;
  const rest = options.filter((value) => value !== first);
  return (
    <div className={`dice-controls${quiet && off ? " dice-off" : ""}`}>
      {first ? (
        <span className="dice-main">
          <button
            type="button"
            className={buttonClass}
            {...offProps}
            title={blocked || c.rollOption.replace("{formula}", first)}
            onClick={guard(() => void roll(first))}
          >
            ⚄ {c.rollOption.replace("{formula}", first)}
          </button>
          <button
            type="button"
            className={`${buttonClass ?? ""} dice-more`}
            {...offProps}
            aria-label={c.roll}
            aria-expanded={open}
            title={blocked || c.roll}
            onClick={guard(() => setOpen(!open))}
          >
            ▾
          </button>
        </span>
      ) : (
        <button
          type="button"
          className={buttonClass}
          {...offProps}
          title={blocked || c.roll}
          aria-expanded={open}
          onClick={guard(() => setOpen(!open))}
        >
          ⚄ {c.roll}
        </button>
      )}
      {onFixReason && reason && !quiet ? (
        <button type="button" className="dice-reason-link" onClick={onFixReason}>
          {reason}
        </button>
      ) : (
        (quiet ? tapped === reason && reason : blocked) && <small role="status">{blocked}</small>
      )}
      {open && (
        <div className="dice-options">
          {rest.map((value) => (
            <button
              type="button"
              className={`${optionClass ?? ""} dice-option`}
              disabled={off}
              key={value}
              onClick={() => void roll(value)}
            >
              {rolled.includes(value) && "✓ "}
              {c.rollOption.replace("{formula}", value)}
            </button>
          ))}
          <div
            className="dice-custom"
            onKeyDown={(event) => {
              if (event.key === "Enter" && event.target instanceof HTMLInputElement) event.preventDefault();
            }}
          >
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
                  disabled={off}
                  onChange={(e) => change(e.target.value)}
                />
              </label>
            ))}
            <button
              type="button"
              className={buttonClass}
              disabled={off}
              onClick={() =>
                void roll(`${+count}d${+sides}${+modifier ? `${+modifier > 0 ? "+" : ""}${+modifier}` : ""}`)
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
