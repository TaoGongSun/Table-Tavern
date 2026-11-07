import { describe, expect, it } from "vitest";
import type { VnCell, VnEventCells } from "../../shared/contracts/backend-contracts";
import {
  AUTO_WAITS,
  TEXT_SPEEDS,
  autoDelay,
  pickLevel,
  cellHash,
  entryKey,
  flatten,
  isPlayable,
  liveChoice,
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
const bg = (
  location: string,
  time: string | null = null,
  period: string | null = time,
  weather: string | null = null,
): VnCell => ({
  type: "bg",
  location,
  time,
  slug: `slug-${location}`,
  period,
  weather,
});
const choices = (...options: string[]): VnCell => ({ type: "choices", options });
const ev = (id: string | null, kind: VnEventCells["kind"], cells: VnCell[]): VnEventCells => ({
  event_id: id,
  kind,
  cells,
});

describe("flatten / playable / entry", () => {
  const events = [
    { ...ev("g1", "narration", [n("a"), choices("x", "y")]), choice: { answered: "x" } },
    ev("p1", "player", [n("x")]),
    { ...ev(null, "narration", [bg("Port"), n("b"), choices("p", "q")]), choice: { answered: null } },
  ];
  const cells = flatten(events);

  it("keys cells by event id (position when missing) and cell index", () => {
    expect(cells.map((cell) => cell.key)).toEqual(["g1#0", "g1#1", "p1#0", "@2#0", "@2#1", "@2#2"]);
  });

  it("drops player cells and bg; stops at answered (past) choices and the open choice of the last event", () => {
    const playable = cells.filter((cell) => isPlayable(cell, "@2")).map((cell) => cell.key);
    expect(playable).toEqual(["g1#0", "g1#1", "@2#1", "@2#2"]);
    expect(liveChoice(cells)?.key).toBe("@2#2");
    const pending = pendingCells(3, [n("c"), choices("r", "s")]);
    expect(pending.map((cell) => [cell.key, isPlayable(cell, "@2")])).toEqual([
      ["pending:3#0", true],
      ["pending:3#1", false],
    ]);
  });

  it("an open choice followed by character lines waits for them; a superseded choice never stops", () => {
    const later = flatten([
      ev("g0", "narration", [choices("old", "older")]),
      { ...ev("g1", "narration", [n("a"), choices("x", "y")]), choice: { answered: null } },
      ev("c1", "dialogue", [d("c", "who?")]),
    ]);
    expect(later.filter((cell) => isPlayable(cell, "c1")).map((cell) => cell.key)).toEqual(["g1#0", "c1#0"]);
    expect(liveChoice(later)?.key).toBe("g1#1");
  });

  it("enters at the scene's first playable cell, not the newest event", () => {
    const playable = cells.filter((cell) => isPlayable(cell, "@2"));
    expect(entryKey(playable)).toBe("g1#0");
    expect(entryKey([])).toBeNull();
    // 還沒有正式事件（空桌第一回合）：從第一個暫定格開始，不是最新那格
    const pending = pendingCells(1, [n("a"), n("b")]);
    expect(entryKey(pending)).toBe("pending:1#0");
  });

  it("never plays or enters at the scene recap; it stays in the log only", () => {
    const recap = flatten([{ ...ev("s0", "narration", [n("前情")]), recap: true }]);
    expect(recap.filter((cell) => isPlayable(cell, "s0"))).toEqual([]);
    expect(entryKey([])).toBeNull();
    const withBeat = flatten([
      { ...ev("s0", "narration", [n("前情")]), recap: true },
      ev("g1", "narration", [n("a")]),
    ]);
    const playable = withBeat.filter((cell) => isPlayable(cell, "g1"));
    expect(entryKey(playable)).toBe("g1#0");
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

  it("backdrop keys follow [bg]; a pending cell without a period keeps the current one", () => {
    expect(at([n("x")]).backdrop).toBeNull();
    expect(at([bg("Port", "黃昏", "dusk"), d("a", "1")]).backdrop).toEqual({
      location: "Port",
      slug: "slug-Port",
      period: "dusk",
    });
    expect(at([bg("Port", "黃昏", "dusk"), bg("Inn", "不久", null)]).backdrop?.period).toBe("dusk");
    expect(at([bg("Inn", null, null)]).backdrop?.period).toBe("day");
  });

  it("bgm sticks across a location change and off clears it", () => {
    const bgm = (moods: string[]): VnCell => ({ type: "bgm", moods });
    expect(at([n("x")]).bgm).toBeNull();
    expect(at([bgm(["Mysterious", "Dark"]), n("a")]).bgm).toEqual(["Mysterious", "Dark"]);
    expect(at([bgm(["Dark"]), bg("Inn"), n("a")]).bgm).toEqual(["Dark"]);
    expect(at([bgm(["Dark"]), bgm([])]).bgm).toEqual([]);
  });

  it("weather: a pending cell without one keeps it at the same place and clears at a new place", () => {
    expect(at([n("x")]).weather).toBe("clear");
    expect(at([bg("Port", "dusk", "dusk", "rain"), bg("Port", "night")]).weather).toBe("rain");
    expect(at([bg("Port", "dusk", "dusk", "rain"), bg("Inn")]).weather).toBe("clear");
    expect(at([bg("Port", "dusk", "dusk", "rain"), bg("Port", null, null, "fog")]).weather).toBe("fog");
    expect(at([bg("Port", "dusk", "dusk", "rain"), bg("Port", null, null, "clear"), bg("Port", "night")]).weather).toBe("clear");
  });

  it("streamed cells fold to the same weather as the formal cells that replace them", () => {
    // 串流暫定格只有明寫的天氣；正式格由後端補好（與 vn_backgrounds::resolve_scene 同規則）
    const places = ["Port", "Port", "Inn", "Port", "Port", "Port"];
    const written = ["rain", null, null, "fog", "clear", null];
    const resolved = ["rain", "rain", "clear", "fog", "clear", "clear"];
    const streamed = places.map((place, i) => bg(place, null, null, written[i]));
    const formal = places.map((place, i) => bg(place, null, "day", resolved[i]));
    for (let i = 0; i < places.length; i++) expect(at(streamed, i).weather).toBe(at(formal, i).weather);
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

  it("autoDelay scales by the auto-wait factor", () => {
    expect(autoDelay("abcde", AUTO_WAITS.short)).toBe(700);
    expect(autoDelay("x".repeat(500), AUTO_WAITS.long)).toBe(12000);
  });

  it.each([
    [undefined, "normal"],
    ["fast", "fast"],
    ["bogus", "normal"],
    ["toString", "normal"],
    [3, "normal"],
  ])("pickLevel(%j) = %s; normal keeps the old fixed 25 ms", (value, level) => {
    expect(pickLevel(TEXT_SPEEDS, value)).toBe(level);
    expect(TEXT_SPEEDS.normal).toBe(25);
  });

  it.each([
    ["VN_REROLL_BUSY: a turn is in progress", "busy"],
    ["Error: VN_REROLL_STALE: changed", "stale"],
    ["VN_REROLL_FAILED: no choices", "failed"],
    ["VN_REROLL_UNAVAILABLE: not a VN table", "unavailable"],
    ["AI_INCOMPLETE_RESPONSE: VN_REROLL_FAILED:", null],
  ])("rerollError(%j)", (raw, code) => expect(rerollError(raw)).toBe(code));
});

describe("cellHash", () => {
  it("is stable per content and tells apart speaker, kind and text", () => {
    expect(cellHash(n("a"))).toBe(cellHash(n("a")));
    expect(cellHash(n("a"))).toMatch(/^[0-9a-f]{16}$/);
    const keys = new Set([n("a"), n("b"), d("x", "a"), d("y", "a"), choices("a", "b")].map(cellHash));
    expect(keys.size).toBe(5);
  });
});
