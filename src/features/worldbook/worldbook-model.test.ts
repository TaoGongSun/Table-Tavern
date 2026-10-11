import { describe, expect, it } from "vitest";
import { draftVisibility, pruneDraftCharacters, type WorldbookDraft } from "./worldbook-model";

const draft = (patch: Partial<WorldbookDraft>): WorldbookDraft => ({
  uid: 1,
  title: "設定",
  keys: "",
  content: "內容",
  constant: true,
  enabled: true,
  order: 0,
  visibility: "characters",
  characters: [],
  ...patch,
});

describe("draftVisibility", () => {
  it("名單裡不在清單上的 id（已封存的角色）照留，不被悄悄濾掉", () => {
    expect(draftVisibility(draft({ characters: ["在場的", "已封存的"] }))).toEqual({
      type: "characters",
      characters: ["在場的", "已封存的"],
    });
  });

  it("GM／公開不帶名單", () => {
    expect(draftVisibility(draft({ visibility: "gm", characters: ["甲"] }))).toEqual({ type: "gm" });
    expect(draftVisibility(draft({ visibility: "public" }))).toEqual({ type: "public" });
  });
});

describe("pruneDraftCharacters", () => {
  const base = draft({ visibility: "characters", characters: ["A", "B"] });

  it("drops deleted ids and keeps the rest", () => {
    expect(pruneDraftCharacters(base, ["B"])).toEqual({ ...base, characters: ["A"] });
  });

  it("hands an emptied character list to the GM", () => {
    expect(pruneDraftCharacters(draft({ visibility: "characters", characters: ["B"] }), ["B"])).toEqual(
      draft({ visibility: "gm", characters: [] }),
    );
  });

  it("returns the same object when nothing matches", () => {
    expect(pruneDraftCharacters(base, ["Z"])).toBe(base);
    const empty = draft({ visibility: "characters", characters: [] });
    expect(pruneDraftCharacters(empty, ["B"])).toBe(empty);
  });

  it("filters the list under other visibilities without changing them", () => {
    for (const visibility of ["gm", "public"] as const) {
      expect(pruneDraftCharacters(draft({ visibility, characters: ["B"] }), ["B"])).toEqual(
        draft({ visibility, characters: [] }),
      );
    }
  });
});
