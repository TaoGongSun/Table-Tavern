// 卡內世界書 → ST World Info 條目（釘版本 06bde939 world-info.js）。陣列形（V2 character_book）照
// convertCharacterBook 換成 ST 欄位；物件形本來就是 ST 世界書檔的條目，缺的欄照 newWorldInfoEntryDefinition
// 的預設補。內容開頭的 @@ 裝飾照 parseDecorators 拆出來。每條帶一個穩定 ID（存檔 world_info.entries 的 id），
// ST 用 `${world}.${uid}` 與內容雜湊認條目的地方，這裡一律用穩定 ID（卡在桌上不會變，兩者等價）。
import { bookEntriesKeyed } from "../cards/card-view";
import { isObject, type JsonObject } from "../cards/card-file";

export const WI_POSITION = { before: 0, after: 1, ANTop: 2, ANBottom: 3, atDepth: 4, EMTop: 5, EMBottom: 6, outlet: 7 } as const;
export const WI_LOGIC = { AND_ANY: 0, NOT_ALL: 1, NOT_ANY: 2, AND_ALL: 3 } as const;
export const DEFAULT_DEPTH = 4;
export const DEFAULT_WEIGHT = 100;
const KNOWN_DECORATORS = ["@@activate", "@@dont_activate"];

export interface WiEntry {
  /** 穩定 ID */
  id: string;
  /** 卡片契約裡這條的 key（陣列索引或物件鍵） */
  bookKey: string;
  key: string[] | null;
  keysecondary: string[];
  comment: string;
  content: string;
  constant: boolean;
  selective: boolean;
  /** 原值（可能不是數字；排序照 ST 直接相減） */
  order: unknown;
  position: number;
  excludeRecursion: boolean;
  preventRecursion: boolean;
  delayUntilRecursion: number | boolean;
  disable: boolean;
  probability: number;
  useProbability: boolean;
  depth: number;
  selectiveLogic: number;
  outletName: string;
  group: string;
  groupOverride: boolean;
  groupWeight: number;
  scanDepth: number | null;
  caseSensitive: boolean | null;
  matchWholeWords: boolean | null;
  useGroupScoring: boolean | null;
  role: number;
  sticky: number | null;
  cooldown: number | null;
  delay: number | null;
  matchPersonaDescription: boolean;
  matchCharacterDescription: boolean;
  matchCharacterPersonality: boolean;
  matchCharacterDepthPrompt: boolean;
  matchScenario: boolean;
  matchCreatorNotes: boolean;
  triggers: string[];
  ignoreBudget: boolean;
  decorators: string[];
}

const strings = (value: unknown): string[] => (Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []);
const keyList = (value: unknown): string[] | null => (Array.isArray(value) ? strings(value) : null);
const bool = (value: unknown, fallback: boolean) => (typeof value === "boolean" ? value : fallback);
const num = (value: unknown, fallback: number) => (typeof value === "number" && Number.isFinite(value) ? value : fallback);
const nullableNum = (value: unknown) => (typeof value === "number" && Number.isFinite(value) ? value : null);
const nullableBool = (value: unknown) => (typeof value === "boolean" ? value : null);
const str = (value: unknown, fallback = "") => (typeof value === "string" ? value : fallback);

/** parseDecorators：開頭連續的 @@ 行裡認得的裝飾拿出來，其餘內容照原樣。 */
export function parseDecorators(content: string): [string[], string] {
  const known = (line: string) => {
    const data = line.startsWith("@@@") ? line.substring(1) : line;
    return KNOWN_DECORATORS.some((decorator) => data.startsWith(decorator));
  };
  if (!content.startsWith("@@")) return [[], content];
  let newContent = content;
  const lines = content.split("\n");
  const decorators: string[] = [];
  let fallbacked = false;
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    if (line.startsWith("@@")) {
      if (line.startsWith("@@@") && !fallbacked) continue;
      if (known(line)) {
        decorators.push(line.startsWith("@@@") ? line.substring(1) : line);
        fallbacked = false;
      } else {
        fallbacked = true;
      }
    } else {
      newContent = lines.slice(index).join("\n");
      break;
    }
  }
  return [decorators, newContent];
}

type Fields = Omit<WiEntry, "id" | "bookKey" | "decorators">;

/** convertCharacterBook 的一條（V2 陣列形）。 */
function fromCharacterBook(entry: JsonObject): Fields {
  const ext = isObject(entry.extensions) ? entry.extensions : {};
  return {
    key: keyList(entry.keys),
    keysecondary: strings(entry.secondary_keys),
    comment: str(entry.comment),
    content: str(entry.content),
    constant: bool(entry.constant, false),
    selective: bool(entry.selective, false),
    order: entry.insertion_order,
    position: num(ext.position, entry.position === "before_char" ? WI_POSITION.before : WI_POSITION.after),
    excludeRecursion: bool(ext.exclude_recursion, false),
    preventRecursion: bool(ext.prevent_recursion, false),
    delayUntilRecursion: typeof ext.delay_until_recursion === "number" || typeof ext.delay_until_recursion === "boolean" ? ext.delay_until_recursion : false,
    disable: !entry.enabled,
    probability: num(ext.probability, 100),
    useProbability: bool(ext.useProbability, true),
    depth: num(ext.depth, DEFAULT_DEPTH),
    selectiveLogic: num(ext.selectiveLogic, WI_LOGIC.AND_ANY),
    outletName: str(ext.outlet_name),
    group: str(ext.group),
    groupOverride: bool(ext.group_override, false),
    groupWeight: num(ext.group_weight, DEFAULT_WEIGHT),
    scanDepth: nullableNum(ext.scan_depth),
    caseSensitive: nullableBool(ext.case_sensitive),
    matchWholeWords: nullableBool(ext.match_whole_words),
    useGroupScoring: nullableBool(ext.use_group_scoring),
    role: num(ext.role, 0),
    sticky: nullableNum(ext.sticky),
    cooldown: nullableNum(ext.cooldown),
    delay: nullableNum(ext.delay),
    matchPersonaDescription: bool(ext.match_persona_description, false),
    matchCharacterDescription: bool(ext.match_character_description, false),
    matchCharacterPersonality: bool(ext.match_character_personality, false),
    matchCharacterDepthPrompt: bool(ext.match_character_depth_prompt, false),
    matchScenario: bool(ext.match_scenario, false),
    matchCreatorNotes: bool(ext.match_creator_notes, false),
    triggers: strings(ext.triggers),
    ignoreBudget: bool(ext.ignore_budget, false),
  };
}

/** 物件形（ST 世界書檔的條目），缺的欄照 newWorldInfoEntryDefinition 的預設。 */
function fromWorldFile(entry: JsonObject): Fields {
  return {
    key: Object.prototype.hasOwnProperty.call(entry, "key") ? keyList(entry.key) : [],
    keysecondary: strings(entry.keysecondary),
    comment: str(entry.comment),
    content: str(entry.content),
    constant: bool(entry.constant, false),
    selective: bool(entry.selective, true),
    order: Object.prototype.hasOwnProperty.call(entry, "order") ? entry.order : 100,
    position: num(entry.position, 0),
    excludeRecursion: bool(entry.excludeRecursion, false),
    preventRecursion: bool(entry.preventRecursion, false),
    delayUntilRecursion: typeof entry.delayUntilRecursion === "number" || typeof entry.delayUntilRecursion === "boolean" ? entry.delayUntilRecursion : 0,
    disable: bool(entry.disable, false),
    probability: num(entry.probability, 100),
    useProbability: bool(entry.useProbability, true),
    depth: num(entry.depth, DEFAULT_DEPTH),
    selectiveLogic: num(entry.selectiveLogic, WI_LOGIC.AND_ANY),
    outletName: str(entry.outletName),
    group: str(entry.group),
    groupOverride: bool(entry.groupOverride, false),
    groupWeight: num(entry.groupWeight, DEFAULT_WEIGHT),
    scanDepth: nullableNum(entry.scanDepth),
    caseSensitive: nullableBool(entry.caseSensitive),
    matchWholeWords: nullableBool(entry.matchWholeWords),
    useGroupScoring: nullableBool(entry.useGroupScoring),
    role: num(entry.role, 0),
    sticky: nullableNum(entry.sticky),
    cooldown: nullableNum(entry.cooldown),
    delay: nullableNum(entry.delay),
    matchPersonaDescription: bool(entry.matchPersonaDescription, false),
    matchCharacterDescription: bool(entry.matchCharacterDescription, false),
    matchCharacterPersonality: bool(entry.matchCharacterPersonality, false),
    matchCharacterDepthPrompt: bool(entry.matchCharacterDepthPrompt, false),
    matchScenario: bool(entry.matchScenario, false),
    matchCreatorNotes: bool(entry.matchCreatorNotes, false),
    triggers: strings(entry.triggers),
    ignoreBudget: bool(entry.ignoreBudget, false),
  };
}

/**
 * 條目在 ST 裡的先後（同 order 時的掃描、插入、預算取捨與抽選都照它）：ST 載入世界書是
 * `Object.keys(data.entries)`。陣列形經 convertCharacterBook 以 `entry.id`（缺則陣列索引）當鍵建物件，
 * 所以是整數鍵由小到大、其餘照出現順序，id 重複時後面那條蓋掉前面、位置留在前面那條；物件形就是原物件的
 * 鍵順序。回傳卡片契約的 key（陣列索引或物件鍵）。
 */
function stOrder(entries: unknown, keyed: [string, JsonObject][]): string[] {
  if (!Array.isArray(entries)) {
    const kept = new Set(keyed.map(([bookKey]) => bookKey));
    return isObject(entries) ? Object.keys(entries).filter((key) => kept.has(key)) : [];
  }
  const slots: Record<string, string> = {};
  for (const [bookKey, entry] of keyed) slots[String(entry.id === undefined ? Number(bookKey) : entry.id)] = bookKey;
  return Object.keys(slots).map((slot) => slots[slot]);
}

/** 卡內世界書的條目（未配 ID），照 ST 載入後的先後：只有 `character_book` 才是 ST 會觸發的書。 */
export function bookEntries(cardData: unknown): { bookKey: string; fields: Fields; decorators: string[] }[] {
  const book = isObject(cardData) ? cardData.character_book : undefined;
  const entries = isObject(book) ? book.entries : undefined;
  const objectForm = isObject(entries);
  const keyed = bookEntriesKeyed(entries);
  const byKey = new Map(keyed);
  return stOrder(entries, keyed).map((bookKey) => {
    const entry = byKey.get(bookKey)!;
    const fields = objectForm ? fromWorldFile(entry) : fromCharacterBook(entry);
    const [decorators, content] = parseDecorators(fields.content);
    return { bookKey, fields: { ...fields, content }, decorators };
  });
}

/**
 * 每條配穩定 ID：存檔已經記過的照用（以卡片契約的 key 對），新的配 `wi-<key>`（撞到就加序號）。
 * 回傳整份 `world_info.entries`（存檔原有的、這張卡沒有的條目也原樣留著）。
 */
export function assignEntryIds(bookKeys: string[], saved: { id: string; key: string }[]): { id: string; key: string }[] {
  const result = [...saved];
  const used = new Set(saved.map((entry) => entry.id));
  for (const key of bookKeys) {
    if (saved.some((entry) => entry.key === key)) continue;
    let id = `wi-${key}`.slice(0, 120);
    for (let suffix = 2; used.has(id); suffix++) id = `wi-${key}`.slice(0, 120) + `-${suffix}`;
    used.add(id);
    result.push({ id, key });
  }
  return result;
}

/** 這張卡的條目，帶上穩定 ID，照 ST getSortedEntries（角色書照 order 由大到小，同值保持 ST 載入的先後）。 */
export function worldInfoEntries(cardData: unknown, ids: { id: string; key: string }[]): WiEntry[] {
  const entries = bookEntries(cardData).flatMap(({ bookKey, fields, decorators }) => {
    const id = ids.find((entry) => entry.key === bookKey)?.id;
    return id === undefined ? [] : [{ ...fields, id, bookKey, decorators }];
  });
  return entries.sort(sortByOrder);
}

/** ST sortFn：`b.order - a.order`（不是數字就照 JS 算出 NaN，當成相等）。 */
export function sortByOrder(a: { order: unknown }, b: { order: unknown }): number {
  const diff = (b.order as number) - (a.order as number);
  return Number.isNaN(diff) ? 0 : diff;
}
