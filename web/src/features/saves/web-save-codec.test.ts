import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseWebSave, type WebSave } from "@desktop/shared/contracts/web-save/web-save";
import { importCardBytes, playCardFromValue } from "../cards/play-card";
import { SAMPLE_CARD, SAMPLE_PLAY_CARD } from "../cards/sample-card";
import type { ChatEntry } from "../chat/chat-turn";
import { VariableScope } from "../sillytavern/variables";
import {
  adoptGlobals,
  EMPTY_CARRY,
  restoreWebSave,
  restoreWebSaveText,
  toWebSave,
  type RestoredGame,
  type SnapshotInput,
} from "./web-save-codec";

const CONTRACT = new URL("../../../../src/shared/contracts/web-save/", import.meta.url);
const CARDS = new URL("../../../../src/shared/contracts/card-view/", import.meta.url);
const read = (name: string) => readFileSync(new URL(name, CONTRACT), "utf8");
const fixture = (name: string) => JSON.parse(read(name)) as WebSave;

const T0 = Date.parse("2026-10-07T21:00:00Z");
const entries: ChatEntry[] = [
  { id: "m0", role: "char", text: "開場 {{user}}", opening: true, sentAt: T0 },
  { id: "m1", role: "user", text: "你好", sentAt: T0 + 1000 },
  { id: "m2", role: "char", text: "整理後", raw: "原文", interrupted: true, sentAt: T0 + 2000 },
];

function snapshot(overrides: Partial<SnapshotInput> = {}): SnapshotInput {
  return {
    card: { ...SAMPLE_PLAY_CARD, regexAllowed: true },
    userName: "旅人",
    openingIndex: 0,
    entries,
    local: { 錢包: "15" },
    global: { 名聲: 3 },
    globalWritten: new Set(["名聲"]),
    carry: EMPTY_CARRY,
    exportedAt: T0 + 5000,
    ...overrides,
  };
}

describe("匯出網頁存檔", () => {
  it("四類欄位全帶，過得了契約檢查器", () => {
    const save = toWebSave(snapshot());
    const checked = parseWebSave(JSON.stringify(save));
    expect(checked.ok).toBe(true);
    expect(save).toMatchObject({
      format: "table-tavern-web-save",
      version: 1,
      exported_at: "2026-10-07T21:00:05.000Z",
      import_route: "character",
      regex_allowed: true,
      user_name: "旅人",
      opening_index: 0,
      card_storage: {},
      world_info: { entries: [], timed: {}, last_message_id: null, message_effects: {} },
      mvu: {
        macros: null,
        seed: null,
        layers: { chat: { 錢包: "15" }, character: {}, global: { 名聲: 3 }, preset: {}, script: {}, extension: {} },
      },
    });
    // 原卡外殼一個字都不改
    expect(save.card).toEqual(SAMPLE_CARD);
    expect(save.card_png).toBeUndefined();
    expect(save.messages).toEqual([
      { id: "m0", role: "char", text: "開場 {{user}}", ts: "2026-10-07T21:00:00.000Z", opening: true },
      { id: "m1", role: "user", text: "你好", ts: "2026-10-07T21:00:01.000Z" },
      { id: "m2", role: "char", text: "整理後", raw: "原文", ts: "2026-10-07T21:00:02.000Z", interrupted: true },
    ]);
  });

  it("從 PNG 匯入的卡帶原 PNG（標準 base64），讀回來還是同一張圖", () => {
    const png = new Uint8Array(readFileSync(new URL("composite.png", CARDS)));
    const imported = importCardBytes(png);
    if (!imported.ok) throw new Error(imported.error);
    const save = toWebSave(snapshot({ card: imported.card, entries: [] }));
    expect(parseWebSave(JSON.stringify(save)).ok).toBe(true);
    expect(save.card_png).toBe(Buffer.from(png).toString("base64"));
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    expect(restored.game.card.png).toEqual(png);
    expect(restored.game.card.source).toBe("png:chara");
  });

  it("PNG 讀出的卡與 card 不同：以 card 為準、不留 PNG", () => {
    const png = new Uint8Array(readFileSync(new URL("composite.png", CARDS)));
    const save = toWebSave(snapshot({ card: { ...SAMPLE_PLAY_CARD, png } }));
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    expect(restored.game.card.png).toBeUndefined();
    expect(restored.game.card.text.name).toBe("瑟拉");
  });

  it("計時算到的那則被刪掉：last_message_id 改回 null", () => {
    const carry = { ...EMPTY_CARRY, worldInfo: { ...EMPTY_CARRY.worldInfo, last_message_id: "gone" } };
    expect(toWebSave(snapshot({ carry })).world_info.last_message_id).toBeNull();
    const kept = { ...EMPTY_CARRY, worldInfo: { ...EMPTY_CARRY.worldInfo, last_message_id: "m1" } };
    expect(toWebSave(snapshot({ carry: kept })).world_info.last_message_id).toBe("m1");
  });

  it("變數表裡 JSON 帶不走的值照 JSON 規則丟掉", () => {
    const save = toWebSave(snapshot({ local: { a: 1, b: undefined } }));
    expect(save.mvu?.layers.chat).toEqual({ a: 1 });
  });
});

/** 一桌在分頁裡接著玩（global 照接著玩的規則補進 `tab`）、玩完再匯出；`play` 照 ST 變數語意改分頁 global。 */
function reExport(game: RestoredGame, tab: Record<string, unknown>, play: (global: VariableScope) => void = () => {}) {
  adoptGlobals(tab, game.global);
  const global = new VariableScope(tab);
  play(global);
  return toWebSave({
    card: game.card,
    userName: game.userName,
    openingIndex: game.openingIndex,
    entries: game.entries,
    local: game.local,
    global: tab,
    globalWritten: global.written,
    carry: game.carry,
    exportedAt: Date.parse("2026-10-08T00:00:00Z"),
  });
}

const restoredOf = (save: WebSave) => {
  const restored = restoreWebSave(save);
  if (!restored.ok) throw new Error(JSON.stringify(restored.error));
  return restored.game;
};

describe("匯入網頁存檔回網頁版", () => {
  it("契約 fixture 進出網頁版一個字都不掉（世界書路、MVU 其他層、每則變數表、觸發狀態、卡片 storage、時間原文）", () => {
    for (const name of ["short.json", "minimal.json", "web-export.json", "worldbook-route.json", "worldbook-route-mvu.json"]) {
      const original = fixture(name);
      const again = reExport(restoredOf(original), {});
      expect(JSON.parse(JSON.stringify(again)), name).toEqual({ ...original, exported_at: "2026-10-08T00:00:00.000Z" });
    }
  });

  it("時間原文（次毫秒、時區）原樣寫回，不經 Date 重新序列化", () => {
    const save = fixture("minimal.json");
    save.messages[0].ts = "2026-10-07T21:00:00.123456789+08:00";
    const game = restoredOf(save);
    expect(game.entries[0].sentAt).toBe(Date.UTC(2026, 9, 7, 13, 0, 0, 123));
    expect(reExport(game, {}).messages[0].ts).toBe("2026-10-07T21:00:00.123456789+08:00");
  });

  it("conflicting global value survives load and re-export", () => {
    const save = toWebSave(snapshot({ global: { conflict: "saved", 只在存檔: 1 }, globalWritten: new Set(["conflict", "只在存檔"]) }));
    const tab: Record<string, unknown> = { conflict: "existing" };
    const again = reExport(restoredOf(save), tab);
    // 分頁照只補缺：已有的不覆蓋
    expect(tab).toEqual({ conflict: "existing", 只在存檔: 1 });
    // 這桌沒寫過 conflict：存檔原值帶回
    expect(again.mvu?.layers.global).toEqual({ conflict: "saved", 只在存檔: 1 });
    // 這桌寫過（改值、刪掉）就用新狀態
    const written = reExport(restoredOf(save), { conflict: "existing" }, (live) => {
      live.set("conflict", "played");
      live.del("只在存檔");
    });
    expect(written.mvu?.layers.global).toEqual({ conflict: "played" });
  });

  it("a key written back to its original value still exports the played value", () => {
    const save = toWebSave(snapshot({ global: { conflict: "saved" }, globalWritten: new Set(["conflict"]) }));
    // 寫成分頁原本就有的值：值沒變，但這桌寫過
    const same = reExport(restoredOf(save), { conflict: "existing" }, (live) => live.set("conflict", "existing"));
    expect(same.mvu?.layers.global).toEqual({ conflict: "existing" });
    // 改成別的再改回來
    const back = reExport(restoredOf(save), { conflict: "existing" }, (live) => {
      live.set("conflict", "played");
      live.set("conflict", "existing");
    });
    expect(back.mvu?.layers.global).toEqual({ conflict: "existing" });
  });

  it("another save's global keys never leak into this export", () => {
    const save = toWebSave(snapshot({ global: { 這桌的: 1 }, globalWritten: new Set(["這桌的"]) }));
    const again = reExport(restoredOf(save), { 別桌的: "x" }, (live) => live.set("這桌新寫的", 2));
    expect(again.mvu?.layers.global).toEqual({ 這桌的: 1, 這桌新寫的: 2 });
    // 新開的桌：分頁裡別桌的鍵也不帶
    const fresh = toWebSave(snapshot({ global: { 別桌的: "x", 新開這桌寫的: 1 }, globalWritten: new Set(["新開這桌寫的"]) }));
    expect(fresh.mvu?.layers.global).toEqual({ 新開這桌寫的: 1 });
  });

  it("逐字稿、旗標、這段對話的變數都還原", () => {
    const restored = restoreWebSave(toWebSave(snapshot()));
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    const { game } = restored;
    expect(game.entries).toEqual(entries.map((entry) => ({ ...entry, ts: new Date(entry.sentAt!).toISOString() })));
    expect(game.local).toEqual({ 錢包: "15" });
    expect(game.global).toEqual({ 名聲: 3 });
    expect(game.card.regexAllowed).toBe(true);
    expect(game.card.route).toBe("character");
    expect(game.userName).toBe("旅人");
    expect(game.openingIndex).toBe(0);
  });

  it("版號不認得拒收、內容不合契約說明哪一欄、不是存檔就說不是", () => {
    expect(restoreWebSaveText(read("future-version.json"))).toEqual({
      ok: false,
      error: { kind: "save", error: { kind: "version", version: "2" } },
    });
    const invalidDir = new URL("invalid/", CONTRACT);
    for (const name of readdirSync(invalidDir)) {
      const result = restoreWebSaveText(readFileSync(new URL(name, invalidDir), "utf8"));
      const expected = name.split("--")[0];
      expect(result.ok, name).toBe(false);
      if (!result.ok && result.error.kind === "save") expect(result.error.error.kind, name).toBe(expected);
      else throw new Error(`${name} 應在契約檢查就拒收`);
    }
    expect(restoreWebSaveText('{"spec":"chara_card_v2"}')).toEqual({ ok: false, error: { kind: "save", error: { kind: "not_web_save" } } });
  });

  it("網頁版玩不了的卡不收", () => {
    const save = fixture("minimal.json");
    const broken = { ...save, card: { spec: "chara_card_v2", spec_version: "2.0", data: { name: "兩\n行" } } };
    expect(restoreWebSave(broken)).toEqual({ ok: false, error: { kind: "card", error: "name_not_single_line" } });
  });

  it("沒有 mvu 的存檔：變數從空的開始，再匯出帶空表", () => {
    const save = { ...fixture("minimal.json"), mvu: null, messages: fixture("minimal.json").messages.map(({ message_vars: _vars, ...rest }) => rest) };
    const restored = restoreWebSave(save);
    if (!restored.ok) throw new Error(JSON.stringify(restored.error));
    expect(restored.game.local).toEqual({});
    expect(restored.game.carry.mvu).toBeNull();
    const card = playCardFromValue("json", save.card);
    expect(card.text.name).toBe("A");
  });
});
