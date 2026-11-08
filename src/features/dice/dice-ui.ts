import { parseMarker } from "../../shared/ui/event-text";
import copy from "./dice-copy.json";
import { currentLang } from "../../i18n";
export const diceCopy = () => copy[currentLang()] ?? copy.en;
export function diceError(reason: unknown): string {
  const message = String(reason);
  const c = diceCopy();
  if (message.includes("dice-scene-changed")) return c.sceneChanged;
  if (message.includes("dice-invalid-formula")) return c.badFormula;
  if (message.includes("dice-disabled")) return c.off;
  if (message.includes("dice-relay-active")) return c.busy;
  return message;
}

export function playerTurn(
  events: import("../../shared/contracts/backend-contracts").TranscriptEvent[],
  name = "",
): boolean {
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i];
    if (event.kind !== "system") break;
    const marker = parseMarker(event.marker);
    if (marker?.type === "gm_call") return marker.name === name;
  }
  return true;
}
