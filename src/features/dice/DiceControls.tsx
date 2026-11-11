import { useMemo } from "react";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { eventDisplayText } from "../../shared/ui/event-text";
import { DicePicker } from "./DicePicker";
import { rankSuggestions, rolledFormula } from "./dice-suggestions";
import { diceCopy, playerTurn } from "./dice-ui";

export function DiceControls({
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
  const narrations = events.flatMap((e, i) => (e.kind === "narration" ? [i] : []));
  const last = narrations.length ? narrations[narrations.length - 1] : -1;
  const prev = narrations.length > 1 ? narrations[narrations.length - 2] : -1;
  const latest = last >= 0 ? eventDisplayText(events[last]) : "";
  // 這則旁白回應的擲骰（上一則旁白之後到這則之前）
  const answered = events
    .slice(prev + 1, Math.max(last, 0))
    .flatMap((e) => (e.kind === "system" && rolledFormula(e.text)) || [])
    .join("\n");
  const options = useMemo(
    () => rankSuggestions(latest, playerName, [], answered.split("\n")).map((x) => x.value),
    [latest, playerName, answered],
  );
  const reason = locked
    ? c.locked
    : room
      ? c.inRoom
      : busy
        ? c.busy
        : !diceEnabled
          ? c.off
          : !playerTurn(events, playerName)
            ? c.turn
            : !hasTarget
              ? c.target
              : "";
  return (
    <DicePicker
      copy={c}
      options={options}
      reason={reason}
      buttonClass="btn btn-sm"
      onRoll={onRoll}
      onExpand={onExpand}
    />
  );
}
