// 卡片正規化檢視（卡片契約 src/shared/contracts/card-view/card-view.md）：與桌面版
// src-tauri/src/import/card_view_tests.rs 對同一份 golden.json。分路（probe_import）、開場白
// （card_openings）、世界書路（worldbook_json）、介面腳本（interface.rs card_interface）都照桌面版逐條改寫。
import { cardData, decodeCardFile, isObject, type CardErrorCode, type CardSource, type JsonObject } from "./card-file";

export interface EntryView {
  key: string;
  uid: number | null;
  keys: string[];
  secondary_keys: string[];
  comment: string;
  content: string;
  constant: boolean;
  enabled: boolean;
  order: number;
  position: unknown;
}

export type BookForm = "array" | "object" | "none";

export interface BookView {
  name: string | null;
  form: BookForm;
  entries: EntryView[];
}

export interface InterfaceScriptView {
  name: string;
  find_regex: string;
  replace_string: string;
  trim_strings: string[];
  min_depth: number | null;
  max_depth: number | null;
}

export interface RouteView {
  name: string | null;
  book_shaped: boolean;
  lorebook_heavy: boolean;
  book_entries: number;
  alternate_greetings: number;
  decision: "worldbook" | "ask";
  suggested: "worldbook" | "character";
}

export interface CardView {
  source: CardSource;
  shell: { wrapped: boolean; spec: string | null; spec_version: string | null; extra_keys: string[] };
  fields: Record<(typeof STRING_FIELDS)[number], string | null>;
  tags: string[];
  alternate_greetings: string[];
  extensions: unknown;
  openings: string[];
  route: RouteView;
  books: {
    character: BookView | null;
    worldbook: BookView & { source: "character_book" | "top_level" | "persona" | "none" };
  };
  interface: { unsupported: string | null; mvu: boolean; scripts: InterfaceScriptView[] };
  /** 各匯入路桌面版會不會拒收（契約「有效性規則」）；null＝收 */
  validity: { character: CharacterInvalid | null; worldbook: "card_nothing_to_import" | null };
}

export type CharacterInvalid = "card_missing_name" | "name_not_single_line";

export type CardViewResult = CardView | { error: CardErrorCode };

export const STRING_FIELDS = [
  "name",
  "description",
  "personality",
  "scenario",
  "first_mes",
  "mes_example",
  "creator_notes",
  "system_prompt",
  "post_history_instructions",
  "creator",
  "character_version",
] as const;

/** 人設欄：lorebook_heavy 的人設份量，也是沒有條目時轉成常駐條目的內容（card.rs PERSONA_FIELDS）。 */
const PERSONA_FIELDS = ["description", "personality", "scenario", "mes_example"] as const;

/** Rust `str::trim` 的空白集合（Unicode White_Space）；JS trim 多收 U+FEFF、少收 U+0085。 */
const RUST_SPACE = "\\t\\n\\v\\f\\r \\u0085\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000";
const RUST_TRIM = new RegExp(`^[${RUST_SPACE}]+|[${RUST_SPACE}]+$`, "g");
export const rustTrim = (text: string) => text.replace(RUST_TRIM, "");

const charCount = (text: string) => Array.from(text).length;
const utf8Length = (text: string) => new TextEncoder().encode(text).length;
const asciiUpper = (text: string) => text.replace(/[a-z]+/g, (part) => part.toUpperCase());

export const stringField = (data: unknown, field: string): string | null =>
  isObject(data) && typeof data[field] === "string" ? (data[field] as string) : null;

const has = (data: unknown, key: string) => isObject(data) && Object.prototype.hasOwnProperty.call(data, key);
const get = (data: unknown, key: string): unknown => (isObject(data) ? data[key] : undefined);
const isInteger = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value);
const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

/** 鍵排序照 Rust 字串（UTF-8 位元組＝碼位）順序。 */
function compareCodePoints(a: string, b: string): number {
  const left = Array.from(a);
  const right = Array.from(b);
  for (let index = 0; index < Math.min(left.length, right.length); index++) {
    const diff = left[index].codePointAt(0)! - right[index].codePointAt(0)!;
    if (diff !== 0) return diff;
  }
  return left.length - right.length;
}

/** 照 Rust `str::parse::<u64>`：可帶一個前置 `+`，其餘只收數字。 */
function parseU64(key: string): bigint | null {
  if (!/^\+?\d+$/.test(key)) return null;
  const value = BigInt(key.replace(/^\+/, ""));
  return value <= 18_446_744_073_709_551_615n ? value : null;
}

/** 條目展開（card.rs book_entries_keyed）：陣列照原順序；物件形照數字鍵、非數字鍵排最後依字元序；
 *  不是物件的值不算條目。 */
export function bookEntriesKeyed(entries: unknown): [string, JsonObject][] {
  const objects = (pairs: [string, unknown][]) => pairs.filter((pair): pair is [string, JsonObject] => isObject(pair[1]));
  if (Array.isArray(entries)) return objects(entries.map((value, index) => [String(index), value]));
  if (!isObject(entries)) return [];
  return objects(Object.entries(entries)).sort(([a], [b]) => {
    const x = parseU64(a);
    const y = parseU64(b);
    if (x !== null && y !== null) return x < y ? -1 : x > y ? 1 : 0;
    if (x !== null) return -1;
    if (y !== null) return 1;
    return compareCodePoints(a, b);
  });
}

function parseI64(key: string): number | null {
  if (!/^[+-]?\d+$/.test(key)) return null;
  const value = Number(key);
  return Number.isSafeInteger(value) ? value : null;
}

function entryView(key: string, entry: JsonObject, objectForm: boolean): EntryView {
  const text = (field: string) => (typeof entry[field] === "string" ? (entry[field] as string) : "");
  const flag = (field: string) => (typeof entry[field] === "boolean" ? (entry[field] as boolean) : null);
  /** V2 `enabled`：缺或 null＝啟用，其餘照 JS 真假值（與匯入同一套規則） */
  const enabledFlag = () => (entry.enabled === undefined || entry.enabled === null ? true : Boolean(entry.enabled));
  const integer = (field: string) => (isInteger(entry[field]) ? (entry[field] as number) : null);
  const common = {
    key,
    comment: text("comment"),
    content: text("content"),
    constant: flag("constant") ?? false,
    position: Object.prototype.hasOwnProperty.call(entry, "position") ? entry.position : null,
  };
  if (objectForm) {
    return {
      ...common,
      uid: integer("uid") ?? parseI64(key),
      keys: strings(entry.key),
      secondary_keys: strings(entry.keysecondary),
      enabled: !(flag("disable") ?? false),
      order: integer("order") ?? 0,
    } satisfies EntryView;
  }
  return {
    ...common,
    uid: integer("id"),
    keys: strings(entry.keys),
    secondary_keys: strings(entry.secondary_keys),
    enabled: enabledFlag(),
    order: integer("insertion_order") ?? 0,
  } satisfies EntryView;
}

export function entriesView(entries: unknown): EntryView[] {
  const objectForm = isObject(entries);
  return bookEntriesKeyed(entries)
    .filter((pair): pair is [string, JsonObject] => isObject(pair[1]))
    .map(([key, entry]) => entryView(key, entry, objectForm));
}

const bookForm = (entries: unknown): BookForm => (Array.isArray(entries) ? "array" : isObject(entries) ? "object" : "none");

function bookView(book: unknown): BookView {
  const entries = get(book, "entries");
  const name = get(book, "name");
  return { name: typeof name === "string" ? name : null, form: bookForm(entries), entries: entriesView(entries) };
}

/** 桌面版 probe_import 的各欄＋前端身分框的分路規則（useImportController.ts）。 */
export function probeRoute(data: unknown): RouteView {
  const rawEntries = get(get(data, "character_book"), "entries");
  const entries = rawEntries === undefined ? null : bookEntriesKeyed(rawEntries).map(([, value]) => value);
  const persona = PERSONA_FIELDS.map((field) => charCount(rustTrim(stringField(data, field) ?? ""))).reduce((a, b) => a + b, 0);
  const book = (entries ?? [])
    .map((entry) => get(entry, "content"))
    .filter((content): content is string => typeof content === "string")
    .map(charCount)
    .reduce((a, b) => a + b, 0);
  const lorebookHeavy = entries !== null && entries.length >= 3 && book >= persona * 3;
  const greetings = get(data, "alternate_greetings");
  const alternateGreetings = Array.isArray(greetings) ? greetings.length : 0;
  const rawName = stringField(data, "name");
  const name = rawName === null || rustTrim(rawName) === "" ? null : rustTrim(rawName);
  const bookShaped = !has(data, "character_book") && has(data, "entries");
  const decision = name === null || bookShaped ? "worldbook" : "ask";
  return {
    name,
    book_shaped: bookShaped,
    lorebook_heavy: lorebookHeavy,
    book_entries: entries?.length ?? 0,
    alternate_greetings: alternateGreetings,
    decision,
    suggested: decision === "worldbook" || lorebookHeavy || alternateGreetings > 0 ? "worldbook" : "character",
  };
}

/** card.rs card_openings：first_mes＋alternate_greetings 各自 trim 後的非空字串。 */
export function cardOpenings(data: unknown): string[] {
  const openings: string[] = [];
  const first = rustTrim(stringField(data, "first_mes") ?? "");
  if (first) openings.push(first);
  for (const greeting of strings(get(data, "alternate_greetings"))) {
    const text = rustTrim(greeting);
    if (text) openings.push(text);
  }
  return openings;
}

/** card.rs worldbook_json：世界書路會匯入的那本書。 */
export function worldbookRoute(data: unknown): CardView["books"]["worldbook"] {
  const characterBook = get(data, "character_book");
  const characterEntries = get(characterBook, "entries");
  if (characterEntries !== undefined && bookEntriesKeyed(characterEntries).length > 0) {
    return { source: "character_book", ...bookView(characterBook) };
  }
  if (has(data, "entries")) return { source: "top_level", ...bookView(data) };
  const content = PERSONA_FIELDS.map((field) => rustTrim(stringField(data, field) ?? ""))
    .filter((text) => text !== "")
    .join("\n\n");
  if (!content) return { source: "none", name: null, form: "none", entries: [] };
  const name = rustTrim(stringField(data, "name") ?? "");
  const entry = { id: 0, keys: [], secondary_keys: [], comment: name, content, constant: true, selective: false, insertion_order: 0, enabled: true, position: "before_char", case_sensitive: false, extensions: {} };
  return { source: "persona", ...bookView({ name, entries: [entry] }) };
}

function isCatchAllRegex(findRegex: string): boolean {
  let body = findRegex;
  if (body.startsWith("/")) {
    const rest = body.slice(1);
    const end = rest.lastIndexOf("/");
    if (end >= 0) body = rest.slice(0, end);
  }
  if (body.startsWith("^")) body = body.slice(1);
  if (body.endsWith("$")) body = body.slice(0, -1);
  return [".+", ".*", "[\\s\\S]*", "[\\s\\S]+"].includes(body);
}

/** interface.rs loads_mvu：酒館助手腳本裡有啟用、內容載入 MagVarUpdate 的腳本（遞迴找）。 */
export function loadsMvu(data: unknown): boolean {
  const walk = (value: unknown): boolean => {
    if (Array.isArray(value)) return value.some(walk);
    if (!isObject(value)) return false;
    const enabled = typeof value.enabled === "boolean" ? value.enabled : true;
    const hit = typeof value.content === "string" && value.content.includes("MagVarUpdate");
    return (enabled && hit) || (enabled && Object.values(value).some(walk));
  };
  const extensions = get(data, "extensions");
  if (extensions === undefined) return false;
  return ["tavern_helper", "TavernHelper_scripts"].some((key) => {
    const value = get(extensions, key);
    return value !== undefined && walk(value);
  });
}

/** interface.rs card_interface：顯示腳本清單（啟用、非 promptOnly、placement 含 2）與不支援判定。 */
export function interfaceView(data: unknown): CardView["interface"] {
  const isScrypt = [stringField(data, "first_mes"), stringField(data, "description")].some(
    (text) => text !== null && asciiUpper(text).includes("SCRYPT"),
  );
  if (isScrypt) return { unsupported: "scrypt", mvu: false, scripts: [] };
  const raw = get(get(data, "extensions"), "regex_scripts");
  const scripts = (Array.isArray(raw) ? raw : [])
    .filter(
      (script) =>
        get(script, "disabled") !== true &&
        get(script, "promptOnly") !== true &&
        Array.isArray(get(script, "placement")) &&
        (get(script, "placement") as unknown[]).some((value) => value === 2),
    )
    .map(
      (script): InterfaceScriptView => ({
        name: stringField(script, "scriptName") ?? "",
        find_regex: stringField(script, "findRegex") ?? "",
        replace_string: stringField(script, "replaceString") ?? "",
        trim_strings: strings(get(script, "trimStrings")),
        min_depth: isInteger(get(script, "minDepth")) ? (get(script, "minDepth") as number) : null,
        max_depth: isInteger(get(script, "maxDepth")) ? (get(script, "maxDepth") as number) : null,
      }),
    );
  const remoteLoader = scripts.some(
    (script) => utf8Length(script.replace_string) < 2000 && script.replace_string.includes(".load(") && isCatchAllRegex(script.find_regex),
  );
  return {
    unsupported: remoteLoader ? "remote_loader" : null,
    mvu: loadsMvu(data),
    scripts: remoteLoader ? [] : scripts,
  };
}

/** 角色卡路的有效性（card.rs parse_character：名字要是字串、trim 後單行；空名字桌面版身分框不給走角色卡路）。 */
export function characterValidity(data: unknown): CharacterInvalid | null {
  const name = stringField(data, "name");
  if (name === null) return "card_missing_name";
  const trimmed = rustTrim(name);
  if (/[\n\r]/.test(trimmed)) return "name_not_single_line";
  return trimmed === "" ? "card_missing_name" : null;
}

export function viewFromValue(source: CardSource, value: unknown): CardView {
  const data = cardData(value);
  const wrapped = isObject(value) && isObject(value.data);
  const shellText = (field: string) => (isObject(value) && typeof value[field] === "string" ? (value[field] as string) : null);
  const characterBook = get(data, "character_book");
  const worldbook = worldbookRoute(data);
  return {
    source,
    shell: {
      wrapped,
      spec: shellText("spec"),
      spec_version: shellText("spec_version"),
      extra_keys:
        wrapped && isObject(value)
          ? Object.keys(value)
              .filter((key) => !["spec", "spec_version", "data"].includes(key))
              .sort(compareCodePoints)
          : [],
    },
    fields: Object.fromEntries(STRING_FIELDS.map((field) => [field, stringField(data, field)])) as CardView["fields"],
    tags: strings(get(data, "tags")),
    alternate_greetings: strings(get(data, "alternate_greetings")),
    extensions: get(data, "extensions") ?? null,
    openings: cardOpenings(data),
    route: probeRoute(data),
    books: {
      character: isObject(characterBook) ? bookView(characterBook) : null,
      worldbook,
    },
    interface: interfaceView(data),
    validity: {
      character: characterValidity(data),
      worldbook: worldbook.source === "none" ? "card_nothing_to_import" : null,
    },
  };
}

export function cardView(bytes: Uint8Array): CardViewResult {
  const decoded = decodeCardFile(bytes);
  if (!decoded.ok) return { error: decoded.error };
  return viewFromValue(decoded.source, decoded.value);
}
