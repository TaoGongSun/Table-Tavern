import { describe, expect, it } from "vitest";
import type { VnCell } from "../../../shared/contracts/backend-contracts";
import { flatten, type FlatCell } from "../vn-cells";
import { canStartSfx, desiredBgmKey, effectiveVolume, filePath, prefVolume, reachedSfx } from "./audio-policy";

const narration = (text: string): VnCell => ({ type: "narration", text });
const sfx = (key: string): VnCell => ({ type: "sfx", query: "door", key });

function formal(cells: VnCell[]): FlatCell[] {
  return flatten([{ event_id: "e1", kind: "narration", cells }]);
}

describe("desiredBgmKey", () => {
  it("maps a missing marker to the theme and off to silence", () => {
    expect(desiredBgmKey(null)).toBe("theme");
    expect(desiredBgmKey([])).toBeNull();
    expect(desiredBgmKey(["Mysterious", "Dark"])).toBe("mysterious+dark");
  });
});

describe("prefVolume", () => {
  it("keeps a stored number and falls back when nothing is stored", () => {
    expect(prefVolume(undefined, 70)).toBe(70);
    expect(prefVolume(0, 70)).toBe(0);
    expect(prefVolume(140, 70)).toBe(100);
  });
});

describe("effectiveVolume", () => {
  it("drops to zero while muted and keeps the slider level otherwise", () => {
    expect(effectiveVolume(true, 70)).toBe(0);
    expect(effectiveVolume(false, 70)).toBe(0.7);
    expect(effectiveVolume(false, 0)).toBe(0);
  });
});

describe("canStartSfx", () => {
  it("drops a third overlap and a start closer than 250ms", () => {
    expect(canStartSfx(0, null, 1000)).toBe(true);
    expect(canStartSfx(1, 1000, 1249)).toBe(false);
    expect(canStartSfx(1, 1000, 1250)).toBe(true);
    expect(canStartSfx(2, null, 1000)).toBe(false);
  });
});

describe("reachedSfx", () => {
  it("plays formal effects with this line, not the pending copy", () => {
    const cells = formal([sfx("creak"), narration("a"), sfx("door"), narration("b")]);
    expect(reachedSfx(cells, 1, "e1").map((cell) => cell.key)).toEqual(["e1#0", "e1#2"]);
    expect(reachedSfx(cells, 3, "e1").map((cell) => cell.key)).toEqual(["e1#2"]);
    const pending: FlatCell = {
      key: "pending:1#0",
      eventKey: null,
      index: 0,
      kind: "pending",
      cell: sfx("later"),
    };
    const line = formal([narration("a")])[0];
    expect(reachedSfx([line, pending], 0, "e1")).toEqual([]);
  });
});

describe("filePath", () => {
  it("matches kind and key", () => {
    const files = [{ kind: "bgm", key: "theme", path: "bgm-theme.mp3" }];
    expect(filePath(files, "bgm", "theme")).toBe("bgm-theme.mp3");
    expect(filePath(files, "sfx", "theme")).toBeNull();
  });
});
