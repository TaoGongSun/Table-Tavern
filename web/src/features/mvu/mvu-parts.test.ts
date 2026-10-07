import { describe, expect, it, vi } from "vitest";
import { checkWorldInfo, MVU_WI_SETTINGS, ST_WI_SETTINGS } from "../sillytavern/world-info-scan";
import type { WiEntry } from "../sillytavern/world-info-book";
import { correctlyMerge, parseStructured } from "./initvar";
import { replaceMacroLike, withoutDollarKeys } from "./macro-like";
import { cardMvuBase, cardWrite, finishReplyText, isMvuData, lastValidVars, layerTables, Revisions, withoutPlaceholder, EMPTY_LAYERS } from "./tables";

const stat = { 角色: { 名字: "莫拉", 年齡: 30, $meta: { extensible: true }, 道具: ["劍", "盾"] }, 說明: "第一行\n第二行" };
const read = (layer: string) => (layer === "message" ? { stat_data: stat } : layer === "chat" ? { 心情: "好" } : {});

describe("TavernHelper macro-like replacements", () => {
  it("get_*_variable: strings as they are, everything else as JSON, `$` keys dropped, missing path is null", () => {
    expect(replaceMacroLike("名字={{get_message_variable::stat_data.角色.名字}}", read)).toBe("名字=莫拉");
    expect(replaceMacroLike("{{get_message_variable::stat_data.角色}}", read)).toBe('{"名字":"莫拉","年齡":30,"道具":["劍","盾"]}');
    expect(replaceMacroLike("{{GET_CHAT_VARIABLE::心情}}／{{get_global_variable::沒有}}", read)).toBe("好／null");
  });

  it("format_*_variable: YAML with every later line indented to the text before the macro", () => {
    expect(replaceMacroLike("狀態：{{format_message_variable::stat_data.角色}}", read)).toBe(
      "狀態：名字: 莫拉\n   年齡: 30\n   道具:\n     - 劍\n     - 盾",
    );
    expect(replaceMacroLike("{{format_message_variable::stat_data.說明}}", read)).toBe("第一行\n第二行");
    // 同一行兩個 format：由左而右，第二個的縮排算到換完第一個之後
    expect(replaceMacroLike("{{format_chat_variable::心情}} {{format_message_variable::stat_data.角色.名字}}", read)).toBe("好 莫拉");
    expect(withoutDollarKeys([{ $x: 1, y: { $z: 2 } }])).toEqual([{ y: {} }]);
  });
});

describe("MVU structured parsing (parseString) and merging", () => {
  it("YAML first, JSON-looking text through JSON5 then jsonrepair; arrays are replaced when merging", () => {
    expect(parseStructured("a: 1\nb: [x, y]")).toEqual({ a: 1, b: ["x", "y"] });
    expect(parseStructured("{a: 1, // 註解\n b: 'x'}")).toEqual({ a: 1, b: "x" });
    expect(parseStructured('{"a": 1, "b": [1, 2')).toEqual({ a: 1, b: [1, 2] });
    expect(() => parseStructured("a: [1, *沒有")).toThrow();
    expect(correctlyMerge({ a: [1, 2, 3], b: { c: 1 } }, { a: [9], b: { d: 2 } })).toEqual({ a: [9], b: { c: 1, d: 2 } });
  });
});

describe("message tables", () => {
  it("a valid table needs stat_data and schema; the last valid one before a message is used", () => {
    const entries = [
      { id: "a", role: "char" as const, text: "開場", vars: { stat_data: { x: 1 }, schema: {} } },
      { id: "b", role: "user" as const, text: "嗨" },
      { id: "c", role: "char" as const, text: "回", vars: { stat_data: { x: 2 } } },
    ];
    expect(isMvuData(entries[2].vars)).toBe(false);
    expect(lastValidVars(entries, 3)).toEqual({ stat_data: { x: 1 }, schema: {} });
    expect(lastValidVars(entries, 0)).toBeUndefined();
  });

  it("the placeholder is added once, status_current_variable blocks go, and prompts drop the placeholder line", () => {
    expect(finishReplyText("正文")).toBe("正文\n\n<StatusPlaceHolderImpl/>");
    expect(finishReplyText("正文<StatusPlaceHolderImpl/>")).toBe("正文<StatusPlaceHolderImpl/>");
    expect(finishReplyText("正文<status_current_variable>舊\n值</status_current_variable>")).toBe("正文\n\n<StatusPlaceHolderImpl/>");
    expect(withoutPlaceholder("正文\n\n<StatusPlaceHolderImpl/>")).toBe("正文\n");
  });

  it("versions are counters per target: content that hashes alike still gets different versions, unchanged content keeps its version", () => {
    const revisions = new Revisions();
    const old = revisions.of("chat", { x: "Aa" });
    const current = revisions.of("chat", { x: "B@" });
    expect(current).not.toBe(old);
    expect(revisions.of("chat", { x: "B@" })).toBe(current);
    expect(revisions.of("chat", null)).toBeNull();
    // 以舊版本為底的寫入一律被拒，帶回現在的值
    const write = vi.fn();
    const settle = cardWrite(
      { requestId: "w", target: "chat", payload: '{"x":"lost"}', base: old, generation: 0, scene: 0 },
      { read: () => ({ found: true, table: { x: "B@" } }), write },
      revisions,
    );
    expect(write).not.toHaveBeenCalled();
    expect(settle).toEqual({
      kind: "mvu-settle",
      results: [{ requestId: "w", ok: false, error: "stale" }],
      authority: { key: "chat", table: { x: "B@" }, rev: current },
    });
  });

  it("the iframe snapshot shares identical tables, targets carry each message's version, layers use the sandbox keys", () => {
    const vars = { stat_data: { x: 1 } };
    const entries = [
      { id: "a", role: "char" as const, text: "1", vars },
      { id: "b", role: "user" as const, text: "2" },
      { id: "c", role: "char" as const, text: "3", vars: { stat_data: { x: 1 } } },
    ];
    const revisions = new Revisions();
    const mvu = cardMvuBase(entries, layerTables({ 心情: "好" }, {}, { ...EMPTY_LAYERS, script: { s1: { a: 1 } } }), { user: "旅人", char: "莫拉" }, revisions);
    expect(mvu.states).toEqual([vars, {}]);
    expect(mvu.floorState).toEqual([0, 1, 0]);
    expect(mvu.targets).toEqual([
      { key: "a", rev: revisions.of("a", vars) },
      { key: "b", rev: null },
      { key: "c", rev: revisions.of("c", vars) },
    ]);
    expect(Object.keys(mvu.layers)).toEqual(["chat", "global", "character:card", "preset", "script:s1"]);
    expect(mvu.layers.chat).toEqual({ rev: revisions.of("chat", { 心情: "好" }), vars: { 心情: "好" } });
    expect(mvu.latestId).toBe(2);
  });
});

describe("MVU's recommended world info settings", () => {
  const entry = (key: string): WiEntry =>
    ({
      id: "e", bookKey: "0", key: [key], keysecondary: [], comment: "", content: "命中", constant: false, selective: false, order: 1,
      position: 0, excludeRecursion: false, preventRecursion: false, delayUntilRecursion: false, disable: false, probability: 100,
      useProbability: false, depth: 4, selectiveLogic: 0, outletName: "", group: "", groupOverride: false, groupWeight: 100,
      scanDepth: null, caseSensitive: null, matchWholeWords: null, useGroupScoring: null, role: 0, sticky: null, cooldown: null,
      delay: null, matchPersonaDescription: false, matchCharacterDescription: false, matchCharacterPersonality: false,
      matchCharacterDepthPrompt: false, matchScenario: false, matchCreatorNotes: false, triggers: [], ignoreBudget: false, decorators: [],
    }) as WiEntry;
  const scan = (settings: typeof ST_WI_SETTINGS) =>
    checkWorldInfo([entry("cat")], {
      chat: ["concatenate"],
      maxContext: 1000,
      globalScan: { personaDescription: "", characterDescription: "", characterPersonality: "", characterDepthPrompt: "", scenario: "", creatorNotes: "" },
      trigger: "normal",
      timed: { sticky: {}, cooldown: {} },
      substitute: (text) => text,
      regex: (text) => text,
      countTokens: () => 1,
      random: () => 0,
      settings,
    }).activated;

  it("whole-word matching is off for MVU tables, on for ST defaults", () => {
    expect(scan(ST_WI_SETTINGS)).toEqual([]);
    expect(scan(MVU_WI_SETTINGS)).toEqual(["e"]);
  });
});
