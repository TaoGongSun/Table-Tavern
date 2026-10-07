// 桌檔契約 v1（網頁存檔）：型別與外框檢查。規格見同目錄 web-save.md；桌面版匯入器
// src-tauri/src/import/web_save/parse.rs 是同一套結構規則，卡的有效性與變數表的細部上限由匯入器再驗。
// 純 TS、不碰 Tauri：桌面版匯入分流與網頁版存檔（@desktop 別名）共用。
// 帶副檔名：web/e2e 用 Node 直接載入這支檔驗真匯出的存檔
import { hasLoneSurrogate, tableProblem } from "../vars-table.ts";

export const WEB_SAVE_FORMAT = "table-tavern-web-save";
export const WEB_SAVE_VERSION = 1;
export const MAX_WEB_SAVE_BYTES = 64 * 1024 * 1024;
const MAX_MESSAGES = 50_000;
const MAX_TEXT_BYTES = 2 * 1024 * 1024;
const MAX_ID_CHARS = 128;
const MAX_USER_NAME_CHARS = 256;
const MAX_LAYER_ID_CHARS = 256;
const MAX_CARD_STORAGE_BYTES = 64 * 1024;

export type VarsTable = Record<string, unknown>;

export interface WebSaveMessage {
  id: string;
  role: "user" | "char";
  text: string;
  raw?: string;
  ts: string;
  opening?: boolean;
  interrupted?: boolean;
  message_vars?: VarsTable;
}
// 可省的欄（raw、opening、interrupted、message_vars、card_png）鍵不在＝沒有；在就不能是 null。
// 必填可 null 的欄（opening_index、mvu、mvu.macros、mvu.macros.char、mvu.seed、world_info.last_message_id）鍵一定要在。

/** 世界書計時（訊息則數），見契約三。 */
export interface WebSaveTimedEffect {
  start: number;
  end: number;
  protected: boolean;
}

export interface WebSaveWorldInfo {
  entries: { id: string; key: string }[];
  /** sticky／cooldown：穩定 ID → 計時；兩張表都可省 */
  timed: { sticky?: Record<string, WebSaveTimedEffect>; cooldown?: Record<string, WebSaveTimedEffect> };
  last_message_id: string | null;
  message_effects: Record<string, unknown>;
}

export interface WebSaveMvu {
  macros: { user: string; char: string | null } | null;
  seed: VarsTable | null;
  layers: {
    chat: VarsTable;
    character: VarsTable;
    global: VarsTable;
    preset: VarsTable;
    script: Record<string, VarsTable>;
    extension: Record<string, VarsTable>;
  };
}

export interface WebSave {
  format: typeof WEB_SAVE_FORMAT;
  version: typeof WEB_SAVE_VERSION;
  exported_at: string;
  card: Record<string, unknown>;
  card_png?: string;
  import_route: "character" | "worldbook";
  regex_allowed: boolean;
  user_name: string;
  opening_index: number | null;
  messages: WebSaveMessage[];
  world_info: WebSaveWorldInfo;
  mvu: WebSaveMvu | null;
  card_storage: Record<string, string>;
}

export type WebSaveError = { kind: "not_web_save" } | { kind: "version"; version: string } | { kind: "invalid"; detail: string };

export type WebSaveResult = { ok: true; save: WebSave } | { ok: false; error: WebSaveError };

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const chars = (text: string) => [...text].length;
const utf8Bytes = (text: string) => new TextEncoder().encode(text).length;
const RFC3339 = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(\.\d{1,9})?)?(?:Z|([+-])(\d{2}):(\d{2}))$/;

/**
 * RFC 3339 時間（日期、`T`、時分，秒與小數可省，`Z` 或 ±hh:mm）→ 毫秒；格式不對或日曆上不存在
 * （13 月、2 月 30 日、25 點、60 秒、偏移 24 小時）回 null。桌面版 parse.rs 的 `is_rfc3339` 同一套規則。
 */
export function rfc3339Millis(text: string): number | null {
  const match = RFC3339.exec(text);
  if (!match) return null;
  const [year, month, day, hour, minute] = match.slice(1, 6).map(Number);
  const second = match[6] === undefined ? 0 : Number(match[6]);
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  if (days === undefined || day < 1 || day > days || hour > 23 || minute > 59 || second > 59) return null;
  const offsetHours = match[9] === undefined ? 0 : Number(match[9]);
  const offsetMinutes = match[10] === undefined ? 0 : Number(match[10]);
  if (offsetHours > 23 || offsetMinutes > 59) return null;
  const millis = match[7] === undefined ? 0 : Number(match[7].slice(1).padEnd(3, "0").slice(0, 3));
  const date = new Date(0);
  date.setUTCFullYear(year, month - 1, day);
  date.setUTCHours(hour, minute, second, millis);
  const offset = (match[8] === "-" ? -1 : 1) * (offsetHours * 60 + offsetMinutes);
  return date.getTime() - offset * 60_000;
}

const isTime = (value: unknown): value is string => typeof value === "string" && rfc3339Millis(value) !== null;

/** 整份存檔（含原卡外殼）任何字串或鍵帶孤立代理字元：桌面版的 JSON 解析不收，這裡同樣整份拒收。 */
function hasBrokenText(value: unknown): boolean {
  if (typeof value === "string") return hasLoneSurrogate(value);
  if (Array.isArray(value)) return value.some(hasBrokenText);
  if (isObject(value)) return Object.entries(value).some(([key, child]) => hasLoneSurrogate(key) || hasBrokenText(child));
  return false;
}
const singleLine = (text: string) => !/[\r\n]/.test(text);

/** 標準 base64、補齊 `=`、結尾位元為零（與桌面版 base64 STANDARD 解碼的接受集合相同）。 */
function isCanonicalBase64(value: unknown): boolean {
  if (typeof value !== "string" || value.length % 4 !== 0) return false;
  try {
    return btoa(atob(value)) === value;
  } catch {
    return false;
  }
}

class Invalid extends Error {}
const fail = (detail: string): never => {
  throw new Invalid(detail);
};

function idOf(value: unknown, field: string, max: number): string {
  if (typeof value !== "string" || chars(value) === 0 || chars(value) > max) fail(field);
  return value as string;
}

/** 變數表：上限照卡片變數（shared/contracts/vars-table，與桌面版同一組）。 */
function tableOf(value: unknown, field: string): VarsTable {
  const problem = tableProblem(value);
  if (problem) fail(`${field}: ${problem}`);
  return value as VarsTable;
}

function tablesById(value: unknown, field: string): Record<string, VarsTable> {
  if (!isObject(value)) fail(field);
  for (const [id, table] of Object.entries(value as Record<string, unknown>)) {
    idOf(id, field, MAX_LAYER_ID_CHARS);
    tableOf(table, `${field}.${id}`);
  }
  return value as Record<string, VarsTable>;
}

function checkMessages(value: unknown): WebSaveMessage[] {
  if (!Array.isArray(value) || value.length > MAX_MESSAGES) fail("messages");
  const seen = new Set<string>();
  (value as unknown[]).forEach((message, index) => {
    const at = (field: string) => `messages[${index}].${field}`;
    if (!isObject(message)) fail(at(""));
    const m = message as Record<string, unknown>;
    const id = idOf(m.id, at("id"), MAX_ID_CHARS);
    if (seen.has(id)) fail(at("id 重複"));
    seen.add(id);
    if (m.role !== "user" && m.role !== "char") fail(at("role"));
    if (typeof m.text !== "string" || utf8Bytes(m.text) > MAX_TEXT_BYTES) fail(at("text"));
    if (m.raw !== undefined && (typeof m.raw !== "string" || utf8Bytes(m.raw) > MAX_TEXT_BYTES)) fail(at("raw"));
    if (!isTime(m.ts)) fail(at("ts"));
    for (const flag of ["opening", "interrupted"] as const) {
      if (m[flag] !== undefined && typeof m[flag] !== "boolean") fail(at(flag));
    }
    if (m.opening === true && (index !== 0 || m.role !== "char")) fail(at("opening"));
    if (m.message_vars !== undefined) tableOf(m.message_vars, at("message_vars"));
  });
  return value as WebSaveMessage[];
}

function checkWorldInfo(value: unknown, messages: WebSaveMessage[]): WebSaveWorldInfo {
  if (!isObject(value) || !Array.isArray(value.entries) || !isObject(value.timed) || !isObject(value.message_effects)) {
    fail("world_info");
  }
  const info = value as unknown as WebSaveWorldInfo;
  const seen = new Set<string>();
  for (const entry of info.entries as unknown[]) {
    if (!isObject(entry) || typeof entry.key !== "string") fail("world_info.entries");
    const id = idOf((entry as Record<string, unknown>).id, "world_info.entries[].id", MAX_ID_CHARS);
    if (seen.has(id)) fail("world_info.entries[].id 重複");
    seen.add(id);
  }
  const last: unknown = info.last_message_id;
  if (last !== null && (typeof last !== "string" || !messages.some((message) => message.id === last))) {
    fail("world_info.last_message_id");
  }
  // timed：只有 sticky／cooldown 兩張表，鍵是 entries 裡的穩定 ID，值是 {start, end, protected}
  // 照數值判斷（JSON.parse 之後分不出 2 與 2.0）：2.0、2e0、-0 都算整數，與桌面版同一規則
  const count = (value: unknown) => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
  for (const [kind, table] of Object.entries(info.timed)) {
    if ((kind !== "sticky" && kind !== "cooldown") || !isObject(table)) fail("world_info.timed");
    for (const [id, effect] of Object.entries(table as Record<string, unknown>)) {
      if (!seen.has(id)) fail(`world_info.timed.${kind} 的條目不在 entries`);
      const e = effect as Record<string, unknown>;
      const ok =
        isObject(effect) &&
        Object.keys(e).every((key) => key === "start" || key === "end" || key === "protected") &&
        count(e.start) &&
        count(e.end) &&
        typeof e.protected === "boolean";
      if (!ok) fail(`world_info.timed.${kind}`);
    }
  }
  return info;
}

function checkMvu(value: unknown): WebSaveMvu | null {
  if (value === null) return null;
  if (!isObject(value) || !isObject(value.layers)) fail("mvu");
  const mvu = value as Record<string, unknown>;
  const macros = mvu.macros;
  if (macros !== null) {
    const m = macros as Record<string, unknown>;
    const ok =
      isObject(macros) &&
      typeof m.user === "string" &&
      singleLine(m.user) &&
      (m.char === null || (typeof m.char === "string" && singleLine(m.char)));
    if (!ok) fail("mvu.macros");
  }
  if (mvu.seed !== null) tableOf(mvu.seed, "mvu.seed");
  const layers = mvu.layers as Record<string, unknown>;
  for (const name of ["chat", "character", "global", "preset"]) tableOf(layers[name], `mvu.layers.${name}`);
  tablesById(layers.script, "mvu.layers.script");
  tablesById(layers.extension, "mvu.layers.extension");
  return value as unknown as WebSaveMvu;
}

function check(value: Record<string, unknown>): WebSave {
  if (!isTime(value.exported_at)) fail("exported_at");
  if (!isObject(value.card)) fail("card");
  if (value.card_png !== undefined && !isCanonicalBase64(value.card_png)) fail("card_png");
  if (value.import_route !== "character" && value.import_route !== "worldbook") fail("import_route");
  if (typeof value.regex_allowed !== "boolean") fail("regex_allowed");
  const user = value.user_name;
  if (typeof user !== "string" || !singleLine(user) || chars(user) > MAX_USER_NAME_CHARS) fail("user_name");
  const opening = value.opening_index;
  if (opening !== null && (typeof opening !== "number" || !Number.isInteger(opening) || opening < 0)) fail("opening_index");
  const messages = checkMessages(value.messages);
  checkWorldInfo(value.world_info, messages);
  const mvu = checkMvu(value.mvu);
  if ((mvu === null || mvu.seed === null) && messages.some((message) => isObject(message.message_vars))) {
    fail("messages[].message_vars 需要 mvu.seed");
  }
  const storage = value.card_storage;
  if (
    !isObject(storage) ||
    Object.values(storage).some((item) => typeof item !== "string") ||
    utf8Bytes(JSON.stringify(storage)) > MAX_CARD_STORAGE_BYTES
  ) {
    fail("card_storage");
  }
  return value as unknown as WebSave;
}

/** 解析並檢查一份網頁存檔。`not_web_save`＝不是網頁存檔（呼叫端照一般卡處理）；`version`＝版號不認得。 */
export function parseWebSave(text: string): WebSaveResult {
  if (utf8Bytes(text) > MAX_WEB_SAVE_BYTES) return { ok: false, error: { kind: "invalid", detail: "檔案太大" } };
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { ok: false, error: { kind: "not_web_save" } };
  }
  if (!isObject(value) || value.format !== WEB_SAVE_FORMAT) return { ok: false, error: { kind: "not_web_save" } };
  if (!("version" in value)) return { ok: false, error: { kind: "invalid", detail: "version" } };
  if (value.version !== WEB_SAVE_VERSION) {
    return { ok: false, error: { kind: "version", version: JSON.stringify(value.version ?? null) } };
  }
  if (hasBrokenText(value)) return { ok: false, error: { kind: "invalid", detail: "JSON: 孤立代理字元" } };
  try {
    return { ok: true, save: check(value) };
  } catch (reason) {
    if (reason instanceof Invalid) return { ok: false, error: { kind: "invalid", detail: reason.message } };
    throw reason;
  }
}

const WHITESPACE = new Set([0x20, 0x0a, 0x0d, 0x09]);

/** 匯入分流：檔案是網頁存檔（含版號不認得、內容不合契約的）就回 true，交給存檔匯入報錯；卡檔與 PNG 回 false。 */
export function looksLikeWebSave(bytes: Uint8Array): boolean {
  if (bytes.find((byte) => !WHITESPACE.has(byte)) !== 0x7b) return false;
  const result = parseWebSave(new TextDecoder().decode(bytes));
  return result.ok || result.error.kind !== "not_web_save";
}
