// 桌檔契約 v1（網頁存檔）：型別與外框檢查。規格見同目錄 web-save.md；桌面版匯入器
// src-tauri/src/import/web_save/parse.rs 是同一套結構規則，卡的有效性與變數表的細部上限由匯入器再驗。
// 純 TS、不碰 Tauri：桌面版匯入分流與網頁版存檔（@desktop 別名）共用。

export const WEB_SAVE_FORMAT = "table-tavern-web-save";
export const WEB_SAVE_VERSION = 1;
export const MAX_WEB_SAVE_BYTES = 64 * 1024 * 1024;
const MAX_MESSAGES = 50_000;
const MAX_TEXT_BYTES = 2 * 1024 * 1024;
const MAX_ID_CHARS = 128;
const MAX_USER_NAME_CHARS = 256;
const MAX_LAYER_ID_CHARS = 256;
const MAX_TABLE_BYTES = 2 * 1024 * 1024;
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

export interface WebSaveWorldInfo {
  entries: { id: string; key: string }[];
  timed: Record<string, unknown>;
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
const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d{1,9})?)?(Z|[+-]\d{2}:\d{2})$/;
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

function tableOf(value: unknown, field: string): VarsTable {
  if (!isObject(value) || utf8Bytes(JSON.stringify(value)) > MAX_TABLE_BYTES) fail(field);
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
    if (typeof m.ts !== "string" || !RFC3339.test(m.ts)) fail(at("ts"));
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
  if (typeof value.exported_at !== "string" || !RFC3339.test(value.exported_at)) fail("exported_at");
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
