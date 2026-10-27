import { describe, expect, it } from "vitest";
import type { TranscriptEvent } from "../../../shared/contracts/backend-contracts";
import { consumeFrom, pushUndone, restorableIndex, restorePayload, tailMatches } from "./undo-stack";

function line(text: string, id?: string): TranscriptEvent {
  return { ...(id ? { id } : {}), ts: `ts-${text}`, speaker_id: "", speaker_name: "GM", kind: "narration", text };
}

describe("undo stack", () => {
  it("skips blank groups on top and consumes them with the restored one", () => {
    let stack = pushUndone(null, "w", 0, [line("旁白")]);
    stack = pushUndone(stack, "w", 0, [line("  ")]);
    expect(restorableIndex(stack, "w", 0)).toBe(0);
    expect(restorableIndex(stack, "w", 1)).toBe(-1);
    expect(consumeFrom(stack, "w", 0, 0)).toBeNull();
    // 疊已是別桌的：不動
    expect(consumeFrom(stack, "x", 0, 0)).toBe(stack);
    // 換桌換幕就重開一疊
    expect(pushUndone(stack, "x", 0, [line("別桌")]).groups).toHaveLength(1);
  });

  it("drops blank lines from the payload and sends tables as JSON text", () => {
    const withVars = { ...line("正文"), message_vars: { z: 1, a: 2 } } as unknown as TranscriptEvent;
    const payload = restorePayload([withVars, line(" ")]);
    expect(payload).toHaveLength(1);
    expect(payload[0].message_vars).toBe('{"z":1,"a":2}');
  });

  it("matches the shown tail by id, or by ts/kind/text for legacy lines", () => {
    const shown = [line("玩家", "p"), line("旁白", "g"), line("舊行")];
    expect(tailMatches(shown, [line("旁白", "g"), line("舊行")])).toBe(true);
    expect(tailMatches(shown, [line("別的", "x"), line("舊行")])).toBe(false);
    expect(tailMatches(shown.slice(0, 1), shown)).toBe(false);
  });
});
