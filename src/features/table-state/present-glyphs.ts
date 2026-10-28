// 在場名單的顯示字形：模型照卡上原字寫 present，畫面依介面語言轉成繁或簡（作者裁決 2026-10-27）。
// 只管顯示；存檔、送模型、登場比對、換幕結算一律原字。轉換用 OpenCC（opencc-js 詞組轉換），
// 本桌已知人名先整名鎖定字形，模型半轉的名字逐字折疊後唯一對上已知人名就顯示那人的字形。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

export type GlyphTarget = "tw" | "hk" | "cn";

/** 介面語言要轉成哪種字形；非中文介面不轉 */
export function glyphTarget(lang: string): GlyphTarget | null {
  if (lang === "zh-TW") return "tw";
  if (lang === "zh-HK") return "hk";
  if (lang === "zh-CN") return "cn";
  return null;
}

/** 與後端 `split_present_names` 同一組分隔符 */
const PRESENT_SEPARATORS = /[、，,／/；;]/;

export function splitPresentNames(raw: string): string[] {
  return raw
    .split(PRESENT_SEPARATORS)
    .map((name) => name.trim())
    .filter((name) => name.length > 0);
}

// 輔助保護字：本身也是對面字形的正規字、常見於姓名、OpenCC 會轉成另一字的字，轉完還原原字
const PROTECTED: Record<GlyphTarget, ReadonlySet<string>> = {
  tw: new Set("余于范沈郁谷后干姜卜朴斗丑松"),
  hk: new Set("余于范沈郁谷后干姜卜朴斗丑松"),
  cn: new Set("沈乾於"),
};

export interface GlyphEngine {
  target: GlyphTarget;
  /** OpenCC 詞組轉換 */
  convert: (text: string) => string;
  /** 逐字折成簡體字形（與後端 fold_glyphs 同一張 TSCharacters 字表），只拿來比對 */
  fold: (text: string) => string;
}

/** 解析 OpenCC TSCharacters 字表：一字多候選取第一個（與後端 glyph.rs 相同） */
export function parseFoldTable(text: string): Map<string, string> {
  const table = new Map<string, string>();
  for (const line of text.split("\n")) {
    const [from, to] = line.split("\t");
    if (!from || to === undefined || [...from].length !== 1) continue;
    const first = to.trim().split(" ")[0];
    if ([...first].length !== 1) continue;
    table.set(from, first);
  }
  return table;
}

/** 單一名字的顯示字形：詞組轉換後逐字還原保護字；轉換前後字數不同就保留原字 */
export function convertName(name: string, engine: GlyphEngine): string {
  const source = [...name];
  const converted = [...engine.convert(name)];
  if (source.length !== converted.length) return name;
  const keep = PROTECTED[engine.target];
  return converted.map((char, index) => (keep.has(source[index]) ? source[index] : char)).join("");
}

/** 本桌已知人名 → 鎖定的顯示字形（原字去重） */
export function buildNameLock(knownNames: readonly string[], engine: GlyphEngine): Map<string, string> {
  const lock = new Map<string, string>();
  for (const name of knownNames) {
    const trimmed = name.trim();
    if (trimmed && !lock.has(trimmed)) lock.set(trimmed, convertName(trimmed, engine));
  }
  return lock;
}

/** present 的顯示字串：逐名處理，名字之間互不影響，以頓號接回 */
export function displayPresent(raw: string, lock: Map<string, string>, engine: GlyphEngine): string {
  const names = splitPresentNames(raw);
  if (names.length === 0) return raw;
  const folded = [...lock.keys()].map((known) => [known, engine.fold(known)] as const);
  return names
    .map((name) => {
      const locked = lock.get(name);
      if (locked !== undefined) return locked;
      // 歸位：逐字折疊後整名相等、且只對上一個已知人名
      const key = engine.fold(name);
      const hits = folded.filter(([, value]) => value === key);
      if (hits.length === 1) return lock.get(hits[0][0]) ?? name;
      return convertName(name, engine);
    })
    .join("、");
}

const engines: Partial<Record<GlyphTarget, Promise<GlyphEngine>>> = {};

/** 動態載入轉換器（只載 cn2t／t2cn 子入口）；失敗不快取，下次再試 */
export function loadGlyphEngine(target: GlyphTarget): Promise<GlyphEngine> {
  engines[target] ??= (async () => {
    const [opencc, foldText] = await Promise.all([
      target === "cn" ? import("opencc-js/t2cn") : import("opencc-js/cn2t"),
      import("../../../src-tauri/src/data/scene/opencc/TSCharacters.txt?raw"),
    ]);
    const convert =
      target === "cn"
        ? opencc.default.Converter({ from: "tw", to: "cn" })
        : opencc.default.Converter({ from: "cn", to: target });
    const table = parseFoldTable(foldText.default);
    const fold = (text: string) => [...text].map((char) => table.get(char) ?? char).join("");
    return { target, convert, fold };
  })().catch((error: unknown) => {
    delete engines[target];
    throw error;
  });
  return engines[target];
}

/**
 * 狀態列用：回傳把 present 原字轉成顯示字形的函式。轉換器還沒載入、載入失敗、非中文介面都原樣顯示；
 * 顯示一律依 render 當下的值計算，載入期間值更新或換桌也不會蓋回舊值。
 */
export function usePresentDisplay(lang: string, knownNames: readonly string[]): (raw: string) => string {
  const target = glyphTarget(lang);
  const [engine, setEngine] = useState<GlyphEngine | null>(null);
  useEffect(() => {
    if (!target) return;
    let alive = true;
    loadGlyphEngine(target).then(
      (loaded) => {
        if (alive) setEngine(loaded);
      },
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [target]);
  const namesKey = knownNames.join("\u0000");
  const ready = engine && engine.target === target ? engine : null;
  const lock = useMemo(
    () => (ready ? buildNameLock(namesKey ? namesKey.split("\u0000") : [], ready) : null),
    [ready, namesKey],
  );
  return (raw) => (ready && lock ? displayPresent(raw, lock, ready) : raw);
}

let foldTable: Promise<(text: string) => string> | undefined;

/** 只載字形折疊表（不分介面語言）：在場名單比對用；失敗不快取，下次再試 */
function loadFold(): Promise<(text: string) => string> {
  foldTable ??= import("../../../src-tauri/src/data/scene/opencc/TSCharacters.txt?raw")
    .then((raw) => {
      const table = parseFoldTable(raw.default);
      return (text: string) => [...text].map((char) => table.get(char) ?? char).join("");
    })
    .catch((error: unknown) => {
      foldTable = undefined;
      throw error;
    });
  return foldTable;
}

/** 名字比對用的字形折疊；還沒載入或載入失敗時回 null（呼叫端改用原字比對） */
export function useGlyphFold(): ((text: string) => string) | null {
  const [fold, setFold] = useState<((text: string) => string) | null>(null);
  useEffect(() => {
    let alive = true;
    loadFold().then(
      (loaded) => {
        if (alive) setFold(() => loaded);
      },
      () => {},
    );
    return () => {
      alive = false;
    };
  }, []);
  return fold;
}

/**
 * 在場名單集合（後端 `present_scoped`）：只留鍵名對得上 present 的項目。還沒有在場資料（present 空）就全留；
 * 已有明確名單卻一個都對不上（例如在場的人還沒被加進集合）就是空的，不把離場的人重新列出來。
 * 比對與後端 state_scope 相同：在場項目含名字就算，先比原字，再比折疊後字形。
 */
export function presentItems<T>(
  entries: [string, T][],
  present: string,
  fold: ((text: string) => string) | null,
): [string, T][] {
  const names = splitPresentNames(present);
  if (names.length === 0) return entries;
  // 與後端 state_scope 同一套：在場項目含名字就算（「林远（受伤）」含「林远」），原字比不到再比折疊字形
  const foldedNames = fold ? names.map(fold) : null;
  const kept = entries.filter(
    ([key]) =>
      names.some((name) => name.includes(key)) ||
      (foldedNames !== null && fold !== null && foldedNames.some((name) => name.includes(fold(key)))),
  );
  return kept;
}

interface PersonEntry {
  title: string;
  is_person?: boolean;
  disabled?: boolean;
}

/**
 * 本桌全部人名：全部角色卡（含隱藏、封存）、玩家、人物條目標題。人物條目在換桌、present 變動、
 * `refreshKey` 變動時重讀一次世界書（本地檔）。世界書在主欄的編輯畫面裡改，改完回到遊戲畫面
 * （狀態列重新顯示）時呼叫端換 `refreshKey`，名冊增刪、改名、停用都跟得上；讀失敗就只用卡名。
 */
export function useKnownPresentNames(
  worldId: string,
  cardNames: readonly string[],
  playerName: string | null | undefined,
  present: string,
  refreshKey: string | number = 0,
): string[] {
  const [persons, setPersons] = useState<{ worldId: string; titles: string[] }>({ worldId: "", titles: [] });
  useEffect(() => {
    if (!worldId) return;
    let alive = true;
    invoke<PersonEntry[]>("read_worldbook", { worldId }).then(
      (entries) => {
        if (alive) {
          const titles = entries.filter((entry) => entry.is_person && !entry.disabled).map((entry) => entry.title);
          setPersons({ worldId, titles });
        }
      },
      () => {
        if (alive) setPersons({ worldId, titles: [] });
      },
    );
    return () => {
      alive = false;
    };
  }, [worldId, present, refreshKey]);
  const titles = persons.worldId === worldId ? persons.titles : [];
  return useMemo(
    () => [...cardNames, ...(playerName ? [playerName] : []), ...titles],
    [cardNames.join("\u0000"), playerName, titles],
  );
}
