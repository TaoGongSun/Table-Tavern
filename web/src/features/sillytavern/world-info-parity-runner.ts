// 桌面版對拍（worldbook-st-trigger-parity）：同一份案例由網頁版實作跑出預期值（web/scripts/gen-world-info-fixtures.mjs），
// 網頁版與 Rust 的 parity 測試各自重跑、只比對。案例檔在 src/shared/contracts/world-info/，欄位說明見該目錄的 world-info.md。
import { isObject, type JsonObject } from "../cards/card-file";
import { resolveEntry, sortByOrder, type Fields } from "./world-info-book";
import { checkWorldInfo, parseRegexFromString, type WiGlobalScanData, type WiResult, type WiSettings, type WiTimed } from "./world-info-scan";

export interface ScanCase {
  name: string;
  /** 物件形（ST 世界書檔）原始條目；掃描前照 sortByOrder 排序 */
  entries: { id: string; raw: JsonObject }[];
  /** 新到舊 */
  chat: string[];
  settings: WiSettings;
  /** null＝沒有上限 */
  maxContext: number | null;
  globalScan?: Partial<WiGlobalScanData>;
  trigger?: string;
  timed?: WiTimed;
  /** 依序取用；用完再取就是案例寫錯 */
  random?: number[];
}

/** outlets 寫成依 JS 鍵順序的 `[名稱, 內容[]]` 陣列（物件在 Rust 讀回來會丟掉順序） */
export type ScanExpected = Omit<WiResult, "outlets"> & { outlets: [string, string[]][]; substituteCalls: string[]; randomCalls: number };

export interface SortCase {
  name: string;
  /** 每個元素的 order；沒有 order 鍵＝undefined */
  items: { order?: unknown }[];
}

export interface RegexCase {
  name: string;
  key: string;
  haystack: string;
  /** 桌面版刻意與 JS 不同的結果（方案七的已知差異）；網頁版照樣比 expected */
  knownDifference?: RegexOutcome & { note: string };
}

export interface RegexOutcome {
  parsed: boolean;
  matches: boolean | null;
}

export interface EntryCase {
  name: string;
  form: "worldFile" | "characterBook";
  raw: JsonObject;
}

const EMPTY_SCAN: WiGlobalScanData = {
  personaDescription: "",
  characterDescription: "",
  characterPersonality: "",
  characterDepthPrompt: "",
  scenario: "",
  creatorNotes: "",
};

/** 對拍用的代換：非恆等（`{{user}}`→Alice、`{{char}}`→Bob），每次呼叫的輸入都記下來。 */
export const substituteRule = (text: string): string => text.split("{{user}}").join("Alice").split("{{char}}").join("Bob");

/** 計數規則：Unicode code point 數。 */
export const countCodePoints = (text: string): number => [...text].length;

export function runScanCase(input: ScanCase): ScanExpected {
  const calls: string[] = [];
  const sequence = input.random ?? [];
  let used = 0;
  const entries = input.entries
    .map(({ id, raw }) => {
      const { fields, decorators } = resolveEntry(raw, true);
      return { ...fields, id, bookKey: id, decorators };
    })
    .sort(sortByOrder);
  const timed = input.timed ?? { sticky: {}, cooldown: {} };
  const result = checkWorldInfo(entries, {
    chat: input.chat,
    maxContext: input.maxContext ?? Number.POSITIVE_INFINITY,
    globalScan: { ...EMPTY_SCAN, ...input.globalScan },
    trigger: input.trigger ?? "normal",
    timed: { sticky: { ...timed.sticky }, cooldown: { ...timed.cooldown } },
    substitute: (text) => {
      calls.push(text);
      return substituteRule(text);
    },
    regex: (text) => text,
    countTokens: countCodePoints,
    random: () => {
      if (used >= sequence.length) throw new Error(`${input.name}: random sequence exhausted`);
      return sequence[used++];
    },
    settings: input.settings,
  });
  return { ...result, outlets: Object.entries(result.outlets), substituteCalls: calls, randomCalls: used };
}

/** sortByOrder 之後的原索引順序（V8 Array.prototype.sort）。 */
export function runSortCase(input: SortCase): number[] {
  return input.items
    .map((item, index) => ({ order: item.order, index }))
    .sort(sortByOrder)
    .map((item) => item.index);
}

export function runRegexCase(input: RegexCase): RegexOutcome {
  const regex = parseRegexFromString(input.key);
  return regex ? { parsed: true, matches: regex.test(input.haystack) } : { parsed: false, matches: null };
}

export function runEntryCase(input: EntryCase): Fields & { decorators: string[] } {
  if (!isObject(input.raw)) throw new Error(`${input.name}: raw must be an object`);
  const { fields, decorators } = resolveEntry(input.raw, input.form === "worldFile");
  return { ...fields, decorators };
}
