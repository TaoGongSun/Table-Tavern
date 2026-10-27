import { describe, expect, it } from "vitest";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import {
  draftFromEntry,
  draftVisibility,
  mergePendingRebase,
  parseKeys,
  pruneDraftCharacters,
  rebaseDraft,
  type DraftGroup,
  type DraftState,
  type WorldbookDraft,
} from "./worldbook-model";

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

describe("rebaseDraft", () => {
  const stored = (patch: Partial<WorldbookEntry> = {}): WorldbookEntry => ({
    uid: 1,
    title: "設定",
    keys: ["鎮"],
    content: "內容",
    constant: true,
    order: 0,
    disabled: false,
    locked: false,
    visibility: { type: "characters", characters: ["X", "Y"] },
    ...patch,
  });
  const opened = (entry: WorldbookEntry, edit: Partial<WorldbookDraft> = {}): DraftState => {
    const origin = draftFromEntry(entry, "、");
    return { draft: { ...origin, ...edit }, origin: JSON.stringify(origin), seq: 3 };
  };
  const pending = (groups: DraftGroup[], deleted: number[] = []) =>
    mergePendingRebase(null, { changed: groups.length ? [{ uid: 1, groups }] : [], deleted });
  const dirty = (state: DraftState) => JSON.stringify(state.draft) !== state.origin;

  it("follows the undo on an untouched draft and stays clean", () => {
    const fresh = stored({ content: "還原" });
    const next = rebaseDraft(opened(stored()), pending(["content"]), [fresh], "、");
    expect(next.draft).toEqual(draftFromEntry(fresh, "、"));
    expect(dirty(next)).toBe(false);
    expect(next.origin).toBe(JSON.stringify(draftFromEntry(fresh, "、")));
  });

  it("takes the undo's value on reported groups only, even if other fields differ on disk", () => {
    const state = opened(stored(), { title: "玩家標題", content: "玩家內文" });
    const fresh = stored({ content: "還原", keys: ["別處改的"] });
    const next = rebaseDraft(state, pending(["content"]), [fresh], "、");
    expect(next.draft).toMatchObject({ title: "玩家標題", content: "還原", keys: "鎮" });
    expect(dirty(next)).toBe(true);
  });

  it("treats visibility and its list as one group", () => {
    const state = opened(stored(), { characters: ["X", "Y", "Z"] });
    const fresh = stored({ visibility: { type: "characters", characters: ["X"] } });
    expect(rebaseDraft(state, pending(["visibility"]), [fresh], "、").draft).toMatchObject({
      visibility: "characters",
      characters: ["X"],
    });
    // 沒報 visibility（撤銷只剪名單）：草稿照留，交給 removedIds 修剪
    expect(rebaseDraft(state, pending(["content"]), [fresh], "、").draft?.characters).toEqual(["X", "Y", "Z"]);
  });

  it("closes a deleted entry's draft, and a reported one the reread no longer has", () => {
    const state = opened(stored(), { content: "打到一半" });
    expect(rebaseDraft(state, pending([], [1]), [stored()], "、").draft).toBeNull();
    expect(rebaseDraft(state, pending(["content"]), [], "、").draft).toBeNull();
  });

  it("leaves new-entry drafts and unreported entries alone", () => {
    const fresh = { ...opened(stored()), draft: { ...draftFromEntry(stored(), "、"), uid: null } };
    expect(rebaseDraft(fresh, pending(["content"], [1]), [], "、")).toBe(fresh);
    const other = opened(stored({ uid: 2 }));
    expect(rebaseDraft(other, pending(["content"], [1]), [], "、")).toBe(other);
  });

  it("keeps the key order of draftFromEntry", () => {
    const next = rebaseDraft(opened(stored()), pending(["visibility", "title"]), [
      stored({ title: "新", visibility: { type: "gm" } }),
    ], "、");
    expect(Object.keys(next.draft!)).toEqual(Object.keys(draftFromEntry(stored(), "、")));
    expect(Object.keys(JSON.parse(next.origin))).toEqual(Object.keys(draftFromEntry(stored(), "、")));
  });
});

describe("關鍵字欄往返", () => {
  it.each([", ", "、"])("以 %j 分隔產出的欄位解析後與原陣列相同（含帶空白的關鍵字）", (separator) => {
    const keys = ["bell tower", "bell", "鐘樓"];
    const entry = {
      uid: 1,
      title: "t",
      keys,
      content: "",
      constant: false,
      order: 0,
      disabled: false,
      locked: false,
      visibility: { type: "gm" },
    } as WorldbookEntry;
    expect(parseKeys(draftFromEntry(entry, separator).keys)).toEqual(keys);
  });
});
