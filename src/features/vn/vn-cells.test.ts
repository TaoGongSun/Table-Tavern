import { describe, expect, it } from "vitest";
import type { VnCell, VnEventCells } from "../../shared/contracts/backend-contracts";
import {
  autoDelay,
  entryKey,
  flatten,
  isPlayable,
  pendingCells,
  realignCursor,
  rerollError,
  stageAt,
  type FlatCell,
} from "./vn-cells";

const n = (text: string): VnCell => ({ type: "narration", text });
const d = (id: string | null, text: string, extra: Partial<VnCell> = {}): VnCell =>
  ({
    type: "dialogue",
    speaker_id: id,
    speaker_name: id ?? "Extra",
    expression: "neutral",
    variant: null,
    thought: false,
    directive: null,
    text,
    ...extra,
  }) as VnCell;
const bg = (location: string, time: string | null = null): VnCell => ({ type: "bg", location, time });
const choices = (...options: string[]): VnCell => ({ type: "choices", options });
const ev = (id: string | null, kind: VnEventCells["kind"], cells: VnCell[]): VnEventCells => ({
  event_id: id,
  kind,
  cells,
});

describe("flatten / playable / entry", () => {
  const events = [
    ev("g1", "narration", [n("a"), choices("x", "y")]),
    ev("p1", "player", [n("x")]),
    ev(null, "narration", [bg("Port"), n("b"), choices("p", "q")]),
  ];
  const cells = flatten(events);

  it("keys cells by event id (position when missing) and cell index", () => {
    expect(cells.map((cell) => cell.key)).toEqual(["g1#0", "g1#1", "p1#0", "@2#0", "@2#1", "@2#2"]);
  });

  it("drops player cells, bg and consumed choices; keeps choices of the last event only", () => {
    const playable = cells.filter((cell) => isPlayable(cell, "@2")).map((cell) => cell.key);
    expect(playable).toEqual(["g1#0", "@2#1", "@2#2"]);
    const pending = pendingCells(3, [n("c"), choices("r", "s")]);
    expect(pending.map((cell) => [cell.key, isPlayable(cell, "@2")])).toEqual([
      ["pending:3#0", true],
      ["pending:3#1", false],
    ]);
  });

  it("enters at the first playable cell of the last non-player event", () => {
    const playable = cells.filter((cell) => isPlayable(cell, "@2"));
    expect(entryKey(cells, playable)).toBe("@2#1");
    expect(entryKey([], [])).toBeNull();
    // 還沒有正式事件（空桌第一回合）：從第一個暫定格開始，不是最新那格
    const pending = pendingCells(1, [n("a"), n("b")]);
    expect(entryKey(pending, pending)).toBe("pending:1#0");
  });
});

describe("realignCursor", () => {
  const cases: [string, VnCell[], { index: number; shown: number }, VnCell[], { index: number; shown: number }][] = [
    ["same cells", [n("abc"), n("def")], { index: 1, shown: 2 }, [n("abc"), n("def")], { index: 1, shown: 2 }],
    ["formal gains a leading [bg]", [n("abc"), n("def")], { index: 1, shown: 1 }, [bg("Port"), n("abc"), n("def")], { index: 2, shown: 1 }],
    ["formal strips extra choices", [n("abc"), choices("a", "b", "c", "d", "e")], { index: 0, shown: 3 }, [n("abc"), choices("a", "b", "c", "d")], { index: 0, shown: 3 }],
    ["directive cap stripped, text same", [d("c1", "hey", { directive: "shake" }), d("c1", "yo", { directive: "shake" })], { index: 1, shown: 1 }, [d("c1", "hey"), d("c1", "yo")], { index: 1, shown: 1 }],
    ["cursor mid last tentative cell", [n("abc"), n("de")], { index: 1, shown: 1 }, [n("abc"), n("defgh")], { index: 1, shown: 1 }],
    ["boundary stays at end of previous text cell", [n("abc"), n("def")], { index: 0, shown: 3 }, [n("abc"), bg("Inn"), n("def")], { index: 0, shown: 3 }],
    ["text done then [bg] and choices", [n("abc"), bg("Inn"), choices("a", "b")], { index: 0, shown: 3 }, [n("abc"), bg("Inn"), choices("a", "b")], { index: 0, shown: 3 }],
    ["formal shorter than shown: last text end", [n("abcdef")], { index: 0, shown: Infinity }, [n("abc")], { index: 0, shown: 3 }],
    ["start of event", [n("abc")], { index: 0, shown: 0 }, [bg("Inn"), n("abc")], { index: 1, shown: 0 }],
  ];
  it.each(cases)("%s", (_, tentative, cursor, formal, expected) => {
    expect(realignCursor(tentative, cursor, formal)).toEqual(expected);
  });
});

describe("stageAt", () => {
  const at = (cells: VnCell[], index = cells.length - 1) =>
    stageAt(pendingCells(0, cells) as FlatCell[], index);

  it("seats by entry order: left, right, centre; extras and the player stay off stage", () => {
    const stage = at([d("a", "1"), d("b", "2"), d(null, "3"), d("__PLAYER__", "4"), d("c", "5")]);
    expect(stage.slots).toEqual(["a", "b", "c"]);
    expect(stage.speaker).toBe("c");
  });

  it("a fourth entrant replaces whoever spoke least recently", () => {
    const stage = at([d("a", "1"), d("b", "2"), d("c", "3"), d("a", "4"), d("d", "5")]);
    expect(stage.slots).toEqual(["a", "d", "c"]);
  });

  it("a new location clears the stage; only a new time does not", () => {
    expect(at([bg("Port", "dusk"), d("a", "1"), bg("Port", "night")]).slots).toEqual(["a", null, null]);
    const moved = at([d("a", "1"), bg("Inn"), d("b", "2")]);
    expect(moved.slots).toEqual(["b", null, null]);
    expect(moved.location).toBe("Inn");
  });

  it("each character keeps their last expression until they speak again", () => {
    const stage = at([
      d("a", "1", { expression: "happy" }),
      d("b", "2", { expression: "angry" }),
      n("x"),
    ]);
    expect(stage.expressions).toEqual({ a: "happy", b: "angry" });
    expect(at([d("a", "1", { expression: "sad" }), d("a", "2")]).expressions.a).toBe("neutral");
  });

  it("@variant sticks until switched, across a location change", () => {
    const stage = at([d("a", "1", { variant: "armor" }), bg("Inn"), d("a", "2"), d("b", "3")]);
    expect(stage.variants).toEqual({ a: "armor" });
    expect(at([d("a", "1", { variant: "armor" }), d("a", "2", { variant: "base" })]).variants.a).toBe("base");
  });

  it("narration has no speaker; directives belong to the current cell only", () => {
    expect(at([d("a", "1"), n("x")]).speaker).toBeNull();
    const cells = [d("a", "1", { directive: "closeup" }), d("b", "2")];
    expect(at(cells, 0).directive).toBe("closeup");
    expect(at(cells, 1).directive).toBeNull();
    expect(at(cells, 1).slots).toEqual(["a", "b", null]);
  });
});

describe("autoDelay / rerollError", () => {
  it.each([
    ["", 1200],
    ["abcde", 1400],
    ["x".repeat(500), 6000],
  ])("autoDelay(%j) = %i", (text, ms) => expect(autoDelay(text)).toBe(ms));

  it.each([
    ["VN_REROLL_BUSY: a turn is in progress", "busy"],
    ["Error: VN_REROLL_STALE: changed", "stale"],
    ["VN_REROLL_FAILED: no choices", "failed"],
    ["VN_REROLL_UNAVAILABLE: not a VN table", "unavailable"],
    ["AI_INCOMPLETE_RESPONSE: VN_REROLL_FAILED:", null],
  ])("rerollError(%j)", (raw, code) => expect(rerollError(raw)).toBe(code));
});
