// 一桌 ⇄ 網頁存檔（桌檔契約 v1，src/shared/contracts/web-save/web-save.md）。匯出四類欄位全帶：每則的變數表
// 跟著逐字稿、MVU 種子與其他層由這桌的卡片變數（features/mvu）給、卡片 storage 由卡片介面給；世界書的
// message_effects 讀進來時原樣收著再帶回。ST 聊天變數就是 MVU 的兩層：這段對話的 local＝`chat`、
// 跨對話的 global＝`global`（TavernHelper 的 chat／global 變數讀的就是這兩處）。
import {
  parseWebSave,
  rfc3339Millis,
  WEB_SAVE_FORMAT,
  WEB_SAVE_VERSION,
  type VarsTable,
  type WebSave,
  type WebSaveError,
  type WebSaveMvu,
  type WebSaveWorldInfo,
} from "@desktop/shared/contracts/web-save/web-save";
import { decodeCardFile, isObject } from "../cards/card-file";
import { playableError, playCardFromValue, type ImportErrorCode, type PlayCard } from "../cards/play-card";
import type { ChatEntry } from "../chat/chat-turn";
import type { VariableMap } from "../sillytavern/variables";

/** 存檔帶進來、由這桌接手的部分（世界書是接著玩的起點；MVU 其他層與種子、卡片 storage 交給這桌）。 */
export interface SaveCarry {
  worldInfo: WebSaveWorldInfo;
  /** MVU 除了 chat／global 兩層之外的部分（null＝存檔裡沒有 mvu） */
  mvu: Omit<WebSaveMvu, "layers"> & { layers: Omit<WebSaveMvu["layers"], "chat" | "global"> } | null;
  cardStorage: Record<string, string>;
  /** 存檔原本的 global 層（新開的桌是空的）：再匯出時這桌沒寫過的鍵照這裡的值帶回 */
  savedGlobal: VarsTable;
}

export const EMPTY_CARRY: SaveCarry = {
  worldInfo: { entries: [], timed: {}, last_message_id: null, message_effects: {} },
  mvu: null,
  cardStorage: {},
  savedGlobal: {},
};

export interface SnapshotInput {
  card: PlayCard;
  userName: string;
  openingIndex: number | null;
  entries: ChatEntry[];
  local: VariableMap;
  /** 分頁現在的 global 變數（所有桌共用的那一份） */
  global: VariableMap;
  /** 這桌玩的期間寫過或刪過的 global 鍵（寫入紀錄，寫回原值也算） */
  globalWritten: ReadonlySet<string>;
  carry: SaveCarry;
  /** 這桌現在的世界書（chat/world-info-setup.ts 的 worldInfoForSave）；不給就照 carry 帶回 */
  worldInfo?: WebSaveWorldInfo;
  exportedAt: number;
}

const iso = (ms: number, fallback: number) => new Date(Number.isFinite(ms) ? ms : fallback).toISOString();

/** 標準 base64（補齊 `=`）。 */
export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let start = 0; start < bytes.length; start += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(start, start + 0x8000));
  }
  return btoa(binary);
}

function fromBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

/** 變數表照 JSON 的樣子存（undefined、函式之類 JSON 帶不走的值照 JSON.stringify 的規則丟掉）。 */
const jsonTable = (values: VariableMap): VarsTable => JSON.parse(JSON.stringify(values)) as VarsTable;

/** 這一桌現在的樣子，照契約 v1 寫成一份網頁存檔。 */
export function toWebSave(input: SnapshotInput): WebSave {
  const { card, carry, entries } = input;
  const ids = new Set(entries.map((entry) => entry.id));
  const lastId = carry.worldInfo.last_message_id;
  const rest = carry.mvu;
  const mvu: WebSaveMvu = {
    macros: rest?.macros ?? null,
    seed: rest?.seed ?? null,
    layers: {
      chat: jsonTable(input.local),
      character: rest?.layers.character ?? {},
      global: jsonTable(globalsForSave(input.global, input.globalWritten, carry.savedGlobal)),
      preset: rest?.layers.preset ?? {},
      script: rest?.layers.script ?? {},
      extension: rest?.layers.extension ?? {},
    },
  };
  // 存檔本來就沒有 mvu、這桌也沒寫出任何變數：照樣寫 null（這桌沒有卡片變數）
  const none = rest === null && Object.keys(mvu.layers.chat).length === 0 && Object.keys(mvu.layers.global).length === 0;
  return {
    format: WEB_SAVE_FORMAT,
    version: WEB_SAVE_VERSION,
    exported_at: iso(input.exportedAt, Date.now()),
    card: card.shell as Record<string, unknown>,
    ...(card.png ? { card_png: toBase64(card.png) } : {}),
    import_route: card.route,
    regex_allowed: card.regexAllowed,
    user_name: input.userName,
    opening_index: input.openingIndex,
    messages: entries.map((entry, index) => ({
      id: entry.id,
      role: entry.role,
      text: entry.text,
      ...(entry.raw !== undefined && entry.raw !== entry.text ? { raw: entry.raw } : {}),
      ts: entry.ts ?? iso(entry.sentAt ?? input.exportedAt, input.exportedAt),
      ...(entry.opening && index === 0 && entry.role === "char" ? { opening: true } : {}),
      ...(entry.interrupted ? { interrupted: true } : {}),
      ...(entry.vars ? { message_vars: entry.vars } : {}),
    })),
    world_info: input.worldInfo ?? {
      ...carry.worldInfo,
      // 計時算到的那則被刪掉了：當作還沒算過（契約要求指向存在的訊息）
      last_message_id: lastId !== null && ids.has(lastId) ? lastId : null,
    },
    mvu: none ? null : mvu,
    card_storage: carry.cardStorage,
  };
}

/** 從存檔接著玩要的東西。 */
export interface RestoredGame {
  card: PlayCard;
  userName: string;
  openingIndex: number | null;
  entries: ChatEntry[];
  local: VariableMap;
  /** 存檔裡的跨對話變數；接著玩時只補這個分頁還沒有的鍵（同桌面版 D18） */
  global: VariableMap;
  carry: SaveCarry;
}

export type RestoreError = { kind: "save"; error: WebSaveError } | { kind: "card"; error: ImportErrorCode };

/** 卡：PNG 讀出的卡與 `card` 完全相同才留 PNG（與桌面版同規則），否則以 `card` 為準。 */
function cardOf(save: WebSave): PlayCard {
  if (save.card_png !== undefined) {
    const png = fromBase64(save.card_png);
    const decoded = decodeCardFile(png);
    if (decoded.ok && decoded.source !== "json" && JSON.stringify(decoded.value) === JSON.stringify(save.card)) {
      return { ...playCardFromValue(decoded.source, save.card), png };
    }
  }
  return playCardFromValue("json", save.card);
}

/** 一份已過契約檢查的存檔 → 網頁版的一桌。卡網頁版玩不了（或那條匯入身分桌面版會拒收）就不收。 */
export function restoreWebSave(save: WebSave): { ok: true; game: RestoredGame } | { ok: false; error: RestoreError } {
  const base = cardOf(save);
  const card: PlayCard = { ...base, route: save.import_route, regexAllowed: save.regex_allowed };
  const unplayable = playableError(card);
  if (unplayable) return { ok: false, error: { kind: "card", error: unplayable } };
  if (save.import_route === "worldbook" && card.view.validity.worldbook !== null) {
    return { ok: false, error: { kind: "card", error: "standalone_book" } };
  }
  const entries: ChatEntry[] = save.messages.map((message) => ({
    id: message.id,
    role: message.role,
    text: message.text,
    // 契約檢查過的時間一定解得開；原文另外留著，再匯出原樣寫回
    sentAt: rfc3339Millis(message.ts) ?? undefined,
    ts: message.ts,
    ...(message.raw !== undefined ? { raw: message.raw } : {}),
    ...(message.opening ? { opening: true } : {}),
    ...(message.interrupted ? { interrupted: true } : {}),
    ...(isObject(message.message_vars) ? { vars: message.message_vars } : {}),
  }));
  const mvu = save.mvu;
  return {
    ok: true,
    game: {
      card,
      userName: save.user_name,
      openingIndex: save.opening_index,
      entries,
      local: structuredClone(mvu?.layers.chat ?? {}),
      global: structuredClone(mvu?.layers.global ?? {}),
      carry: {
        worldInfo: save.world_info,
        mvu: mvu && {
          macros: mvu.macros,
          seed: mvu.seed,
          layers: {
            character: mvu.layers.character,
            preset: mvu.layers.preset,
            script: mvu.layers.script,
            extension: mvu.layers.extension,
          },
        },
        cardStorage: save.card_storage,
        savedGlobal: mvu?.layers.global ?? {},
      },
    },
  };
}

/** 檔案文字 → 一桌：先過契約（版號不認得拒收），再看卡能不能玩。 */
export function restoreWebSaveText(text: string): { ok: true; save: WebSave; game: RestoredGame } | { ok: false; error: RestoreError } {
  const parsed = parseWebSave(text);
  if (!parsed.ok) return { ok: false, error: { kind: "save", error: parsed.error } };
  const restored = restoreWebSave(parsed.save);
  return restored.ok ? { ok: true, save: parsed.save, game: restored.game } : restored;
}

/** 接著玩時的跨對話變數：分頁沒有的鍵補進去（同 D18 只補缺），已有的不覆蓋。 */
export function adoptGlobals(target: VariableMap, incoming: VariableMap): void {
  for (const [key, value] of Object.entries(incoming)) {
    if (!(key in target)) target[key] = structuredClone(value);
  }
}

/**
 * 這一格要存的 global 層：只有「存檔原有的鍵＋這桌玩的期間寫過的鍵」，分頁裡別桌的鍵不混進來。這桌寫過
 * 或刪過的鍵（照寫入紀錄，不比值）用分頁的現況；沒寫過的鍵照存檔原值——分頁已有別的值、接著玩時沒落地的
 * 也一樣以存檔為準（契約：沒落地的值不算丟）。
 */
export function globalsForSave(tab: VariableMap, written: ReadonlySet<string>, saved: VarsTable): VariableMap {
  const result: VariableMap = {};
  for (const key of new Set([...Object.keys(saved), ...written])) {
    if (written.has(key)) {
      if (key in tab) result[key] = tab[key];
    } else {
      result[key] = saved[key];
    }
  }
  return result;
}
