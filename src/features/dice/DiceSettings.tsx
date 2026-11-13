import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RulesGuide } from "./RulesGuide";
import { diceCopy, diceError } from "./dice-ui";
import { RuleBookList } from "./rules/RuleBookRow";

export function DiceSettings({
  worldId,
  diceEnabled,
  entries = [],
  onAddRulesBook,
  onRemoveEntries,
  isTurnRunning,
  onChanged,
}: {
  worldId: string;
  diceEnabled: boolean;
  /** 這桌世界書條目：判斷內建書加過沒 */
  entries?: readonly { uid: number; title: string; content: string }[];
  onAddRulesBook?: (bookJson: string, label: string) => Promise<void>;
  onRemoveEntries?: (uids: number[]) => Promise<boolean>;
  isTurnRunning: () => boolean;
  onChanged?: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  useEffect(() => {
    generation.current++;
    setError("");
    setBusy(false);
    return () => {
      generation.current++;
    };
  }, [worldId]);
  const c = diceCopy();
  async function toggle(on: boolean) {
    if (busy || isTurnRunning()) return;
    const gen = generation.current;
    setBusy(true);
    setError("");
    try {
      await invoke("set_dice_enabled", { worldId, enabled: on });
      if (generation.current !== gen) return;
      await onChanged?.();
    } catch (e) {
      if (generation.current === gen) setError(diceError(e));
    } finally {
      if (generation.current === gen) setBusy(false);
    }
  }
  return (
    <section className="dice-settings">
      <label>
        <input
          type="checkbox"
          checked={diceEnabled}
          disabled={busy || isTurnRunning()}
          onChange={(e) => void toggle(e.target.checked)}
        />
        {c.roll}
      </label>
      <RulesGuide entries={entries} onAddRulesBook={onAddRulesBook} onRemoveEntries={onRemoveEntries} />
      {error && <p role="alert">{error}</p>}
      {onAddRulesBook && <RuleBookList entries={entries} onAddRulesBook={onAddRulesBook} onRemoveEntries={onRemoveEntries} />}
    </section>
  );
}
