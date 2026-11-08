import { useMemo } from "react";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { eventDisplayText } from "../../shared/ui/event-text";
import { DicePicker } from "./DicePicker";
import { suggestions } from "./dice-suggestions";
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
  const gm = [...events].reverse().find((e) => e.kind === "narration");
  const latest = gm ? eventDisplayText(gm) : "";
  const options = useMemo(() => suggestions(latest, playerName), [latest, playerName]);
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
