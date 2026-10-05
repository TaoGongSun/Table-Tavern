import { describe, expect, it } from "vitest";
import {
  REFACTOR_CARD_FORMAT,
  REFACTOR_IMPORT_NEWER,
  hasRefactorCardChunk,
  parseRefactorCard,
} from "./refactor-card";
import { REFACTOR_IMPORT_INVALID, defaultRefactorSelection } from "./refactor-review";

const character = (name: string, suspected = false) => ({
  name,
  emoji: "🙂",
  public_md: "",
  private_md: "",
  source_uids: ["1"],
  solo_entry_md: `${name} 的條目`,
  suspected_player: suspected,
});

const outcome = { characters: [character("甲"), character("乙"), character("丙", true)] };

const envelope = (applied: unknown, extra: Record<string, unknown> = {}) =>
  JSON.stringify({ format: REFACTOR_CARD_FORMAT, version: 1, outcome, applied, ...extra });

const goodApplied = {
  characters: [
    { outcome_index: 0, character_id: "a" },
    { outcome_index: 1, character_id: null },
    { outcome_index: 2, character_id: "c" },
  ],
  player_index: 0,
};

describe("parseRefactorCard", () => {
  it("舊版裸產物照收，applied 為 null", () => {
    const card = parseRefactorCard(JSON.stringify(outcome));
    expect(card.outcome.characters).toHaveLength(3);
    expect(card.applied).toBeNull();
  });

  it("封套帶 applied：解析映射，預設勾選重現來源桌", () => {
    const card = parseRefactorCard(envelope(goodApplied));
    expect(card.applied?.player_index).toBe(0);
    const selection = defaultRefactorSelection(card.outcome, card.applied);
    expect(selection.character_indices).toEqual([0, 2]);
    expect(selection.player_index).toBe(0);
  });

  it("封套沒 applied：預設勾選照舊全勾、疑似玩家當玩家", () => {
    const card = parseRefactorCard(envelope(undefined));
    expect(card.applied).toBeNull();
    const selection = defaultRefactorSelection(card.outcome, card.applied);
    expect(selection.character_indices).toEqual([0, 1, 2]);
    expect(selection.player_index).toBe(2);
  });

  it("applied 完整性違規整份拒收", () => {
    const bads: unknown[] = [
      { characters: goodApplied.characters.slice(0, 2), player_index: null },
      {
        characters: [
          { outcome_index: 0, character_id: "a" },
          { outcome_index: 0, character_id: "b" },
          { outcome_index: 2, character_id: null },
        ],
        player_index: null,
      },
      {
        characters: [
          { outcome_index: 0, character_id: "a" },
          { outcome_index: 1, character_id: null },
          { outcome_index: 3, character_id: null },
        ],
        player_index: null,
      },
      {
        characters: [
          { outcome_index: 0, character_id: " " },
          { outcome_index: 1, character_id: null },
          { outcome_index: 2, character_id: null },
        ],
        player_index: null,
      },
      {
        characters: [
          { outcome_index: 0, character_id: "a" },
          { outcome_index: 1, character_id: "a" },
          { outcome_index: 2, character_id: null },
        ],
        player_index: null,
      },
      { ...goodApplied, player_index: 1 },
      {
        characters: [
          { outcome_index: 0, character_id: 7 },
          { outcome_index: 1, character_id: null },
          { outcome_index: 2, character_id: null },
        ],
        player_index: null,
      },
      { ...goodApplied, player_index: 0.5 },
    ];
    for (const bad of bads) {
      expect(() => parseRefactorCard(envelope(bad))).toThrow(REFACTOR_IMPORT_INVALID);
    }
  });

  it("format／version 檢查：新版本給更新提示", () => {
    expect(() => parseRefactorCard(envelope(null, { format: "other" }))).toThrow(
      REFACTOR_IMPORT_INVALID,
    );
    expect(() => parseRefactorCard(envelope(null, { version: 0 }))).toThrow(REFACTOR_IMPORT_INVALID);
    expect(() => parseRefactorCard(envelope(null, { version: "1" }))).toThrow(
      REFACTOR_IMPORT_INVALID,
    );
    expect(() => parseRefactorCard(envelope(null, { version: 2 }))).toThrow(REFACTOR_IMPORT_NEWER);
  });
});

describe("hasRefactorCardChunk", () => {
  const magic = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  const chunk = (type: string, length: number) => [
    ...[length >>> 24, (length >>> 16) & 255, (length >>> 8) & 255, length & 255],
    ...[...type].map((c) => c.charCodeAt(0)),
    ...new Array<number>(length).fill(0),
    0,
    0,
    0,
    0,
  ];
  const png = (...chunks: number[][]) => new Uint8Array([...magic, ...chunks.flat()]);

  it("走 chunk 表找 ttRd，不論位置", () => {
    expect(
      hasRefactorCardChunk(png(chunk("IHDR", 13), chunk("IDAT", 4), chunk("ttRd", 3), chunk("IEND", 0))),
    ).toBe(true);
    expect(hasRefactorCardChunk(png(chunk("IHDR", 13), chunk("tEXt", 5), chunk("IEND", 0)))).toBe(false);
  });

  it("不是 PNG、長度越界：不認", () => {
    expect(hasRefactorCardChunk(new TextEncoder().encode('{"format":"ttRd"}'))).toBe(false);
    const broken = png(chunk("IHDR", 13));
    broken[10] = 200;
    expect(hasRefactorCardChunk(broken)).toBe(false);
  });
});
