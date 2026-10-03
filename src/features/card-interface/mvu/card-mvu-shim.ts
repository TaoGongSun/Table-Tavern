// 卡片介面沙盒的 MVU（MagVarUpdate）讀變數墊片：把 app 的狀態樹轉成 MVU 的 stat_data，讓 MVU 前端卡
// 畫得出值。行為對照 MVU 與酒館助手（JS-Slash-Runner）的型別規格與實作行為，只當規格書讀、不抄碼
// （見 .ai/plans/card-mvu-shim.md）。沙盒原始碼（讀寫函式、事件）在 card-mvu-shim-source.ts。
import { type StateNode, type TranscriptEvent } from "../../../shared/contracts/backend-contracts";

/** MVU 一樓的變數表（由狀態樹推得的那種形狀） */
export type MvuData = {
  stat_data: Record<string, unknown>;
  display_data: Record<string, unknown>;
  delta_data: Record<string, unknown>;
};

/** 一樓的寫入目標：事件 key（事件 id；舊事件沒有 id 時是 "@逐字稿位置"）與這樓表的版本（null＝尚無表） */
export interface MvuTarget {
  key: string;
  rev: string | null;
}

/** 交給沙盒的 MVU 快照：每樓資料去重成 states，floorState 依樓號對回 */
export interface CardMvu {
  /** 產生目前殼的那一樓 */
  currentId: number;
  /** 最後一則非 system 樓；酒館助手 message 層的 'latest' 指它。-1＝沒有 */
  latestId: number;
  /** 每樓的變數表（卡片變數模式時是事件上的整張表，可能沒有 stat_data） */
  states: Record<string, unknown>[];
  floorState: number[];
  /** 每樓的寫入目標；null＝這樓不能寫（空桌時代替開場白的那一樓） */
  targets: (MvuTarget | null)[];
  /** 卡片變數模式：每樓讀自己的表、沒有表回 {}；false＝還沒啟用，照包 1 由狀態樹推得每樓資料 */
  active: boolean;
  /** 產生這份快照時的桌世代與幕（-1＝還不知道，寫入一律被拒）：寫入帶著它，不冒用之後的新值 */
  generation: number;
  scene: number;
  /** 非 message 層：key＝`chat`、`character:<卡 id>`、`global`、`preset`、`script:<原 ID>`、`extension:<原 ID>`；
   *  script／extension 列出已存在的全部（卡片讀值是同步的）。沒列的層沙盒當空表、版本 null */
  layers: Record<string, MvuLayer>;
  /** `character` 層的身分：目前殼所屬卡的 character_id，世界書卡（沒有 id）固定 `world` */
  characterId: string;
}

/** 非 message 層（計畫 8.7）一層的現況：`rev` null＝檔案還不存在（沙盒當空表） */
export interface MvuLayer {
  rev: string | null;
  vars: Record<string, unknown>;
  /** 讀取失敗：未知不等於不存在，沙盒讀取拋錯、寫入拒絕；script／extension 以 `script:`／`extension:` 標整個類別 */
  error?: string;
}

export type StateTree = Record<string, StateNode>;

/** MVU 在 AI 樓尾補的狀態欄占位（MagVarUpdate update_variables.ts 的寫法） */
export const MVU_PLACEHOLDER = "<StatusPlaceHolderImpl/>";

const NUMBER = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/;
const MACRO = /\{\{(user|char)\}\}/gi;
/** MVU 標記可擴充陣列的元素（schema.ts 的 EXTENSIBLE_MARKER） */
const EXTENSIBLE_MARKER = "$__META_EXTENSIBLE__$";
const hasOwn = (object: object, key: string) => Object.prototype.hasOwnProperty.call(object, key);

function finiteDeep(value: unknown): boolean {
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(finiteDeep);
  if (value !== null && typeof value === "object") return Object.values(value).every(finiteDeep);
  return true;
}

function parseNumber(text: string): number | null {
  if (!NUMBER.test(text)) return null;
  const value = Number(text);
  return Number.isFinite(value) ? value : null;
}

function parseJson(text: string): unknown {
  if (!text.startsWith("[") && !text.startsWith("{")) return undefined;
  try {
    const value: unknown = JSON.parse(text);
    // 巢狀裡任一非有限數字（1e999 → Infinity）深拷貝會變 null：整個葉值保留原字串
    return finiteDeep(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

/**
 * 狀態樹的葉子一律是字串（匯入時引號已剝），照 YAML 純量慣例收窄還原型別。
 * type＝重構記下的原卡欄位型別（number／bool／list），有就照它，不符就保留字串。
 */
export function restoreLeaf(text: string, type?: string): unknown {
  if (type === "number") return parseNumber(text) ?? text;
  if (type === "bool") return text === "true" ? true : text === "false" ? false : text;
  if (type === "list") {
    const value = parseJson(text);
    return Array.isArray(value) ? value : text;
  }
  const number = parseNumber(text);
  if (number !== null) return number;
  if (text === "true") return true;
  if (text === "false") return false;
  if (text === "null") return null;
  const json = parseJson(text);
  return json === undefined ? text : json;
}

interface Macros {
  user: string;
  char: string | null;
}

/** 一次掃完 {{user}}／{{char}}：替換值用回呼交字面，名字裡的 `$&` 不會被當成替換語法，也不會被二次代換 */
function substitute(text: string, macros: Macros): string {
  return text.replace(MACRO, (match, name: string) => {
    if (name.toLowerCase() === "user") return macros.user;
    return macros.char ?? match;
  });
}

/** 資料鍵一律存成自有屬性：`__proto__` 這類鍵用一般指定會改到原型而消失 */
function setOwn(target: Record<string, unknown>, key: string, value: unknown): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

/** MVU 的 metadata 載體：陣列裡 `$arrayMeta: true` 且帶 `$meta` 的元素 */
function isArrayMetaCarrier(value: unknown): boolean {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    (value as Record<string, unknown>).$arrayMeta === true &&
    hasOwn(value, "$meta")
  );
}

/**
 * 整份只清一次：MVU 初始化時 cleanUpMetadata 移除物件的 `$meta`、陣列裡的 EXTENSIBLE_MARKER 與
 * metadata 載體元素；巨集在解析 initvar 前整段代換，所以字串值與鍵都換。
 */
function cleanValue(value: unknown, macros: Macros): unknown {
  if (typeof value === "string") return substitute(value, macros);
  if (Array.isArray(value)) {
    return value
      .filter((item) => item !== EXTENSIBLE_MARKER && !isArrayMetaCarrier(item))
      .map((item) => cleanValue(item, macros));
  }
  if (value !== null && typeof value === "object") return cleanObject(value as Record<string, unknown>, macros);
  return value;
}

/** 鍵代換後撞到同層別的鍵（原本就叫那個名字，或另一個也換成同名）時，這個鍵保留原字面，不覆蓋別人 */
function cleanObject(source: Record<string, unknown>, macros: Macros): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  const keys = Object.keys(source);
  const literal = new Set(keys);
  for (const key of keys) {
    if (key === "$meta") continue;
    const renamed = substitute(key, macros);
    const target = renamed !== key && (literal.has(renamed) || hasOwn(result, renamed)) ? key : renamed;
    if (target !== renamed) console.warn("[table-tavern] 變數鍵代換後撞名，保留原字面", key);
    setOwn(result, target, cleanValue(source[key], macros));
  }
  return result;
}

/** 狀態樹 → 還原型別後的原始結構（還沒清 metadata、沒代換巨集） */
function restoreTree(tree: StateTree, path: string[], valueTypes: Record<string, string>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const key of Object.keys(tree)) {
    const node = tree[key];
    const here = [...path, key];
    const dotted = here.join(".");
    const type = hasOwn(valueTypes, dotted) ? valueTypes[dotted] : undefined;
    setOwn(result, key, typeof node === "string" ? restoreLeaf(node, type) : restoreTree(node, here, valueTypes));
  }
  return result;
}

/** 一棵狀態樹 → MVU 一樓的變數表。display_data 給 stat_data 的拷貝；app 不存舊值，delta_data 為空 */
export function buildMvuData(
  tree: StateTree,
  valueTypes: Record<string, string>,
  macros: Macros,
): MvuData {
  const stat = cleanValue(restoreTree(tree, [], valueTypes), macros) as Record<string, unknown>;
  return { stat_data: stat, display_data: JSON.parse(JSON.stringify(stat)) as Record<string, unknown>, delta_data: {} };
}

/**
 * 會改狀態的樓：只有開場（post_opening）與 GM 回覆（gm_narrate）會套用狀態更新，兩者都落成 speaker_id 空字串
 * 的 narration 事件；中止或截斷的 GM 回覆也是同一形狀，一併算進來。玩家句、角色台詞、系統事件落檔時
 * 只是蓋上當下快照。
 */
export function changesState(event: TranscriptEvent): boolean {
  return event.kind === "narration" && event.speaker_id === "";
}

/**
 * 每樓的狀態來源。events＝本場的樓（chatEvents 排除 gm_only 後，與公開樓號同一套索引）。
 * - 該樓之後沒有會改狀態的樓 → "live"：用目前狀態（含面板手動改值；前端事件快照不會跟著手改更新）。
 * - 否則 → 該樓之前（含）最近一則快照的樓號；一則都沒有 → "empty"。
 */
export function floorSources(events: TranscriptEvent[]): ("live" | "empty" | number)[] {
  let lastChange = -1;
  events.forEach((event, index) => {
    if (changesState(event)) lastChange = index;
  });
  let snapshot: number | "empty" = "empty";
  return events.map((event, index) => {
    if (event.state?.tree !== undefined) snapshot = index;
    return index >= lastChange ? "live" : snapshot;
  });
}

/**
 * 組沙盒要的 MVU 快照。events 為空（空桌，開場白當第 0 樓）時只有一樓、用目前狀態、不能寫。
 * roles＝每樓角色（算 latestId 用），長度與樓數相同；positions＝每樓在這一幕逐字稿裡的位置（舊事件沒有 id
 * 時拿它當寫入目標），沒給就等於樓號。
 * active（卡片變數模式）：每樓讀事件上自己的表，沒有表的樓回 {}，不往前繼承（計畫 8.2）；否則照包 1 的
 * 活樓／歷史樓規則由狀態樹推得。
 */
export function buildCardMvu(input: {
  events: TranscriptEvent[];
  positions?: number[];
  roles: string[];
  currentId: number;
  liveTree: StateTree;
  valueTypes: Record<string, string>;
  macros: Macros;
  active?: boolean;
  generation?: number;
  scene?: number;
  layers?: Record<string, MvuLayer>;
  characterId?: string;
}): CardMvu {
  const { events, liveTree, valueTypes, macros } = input;
  const active = input.active === true;
  const states: Record<string, unknown>[] = [];
  const seen = new Map<string, number>();
  const intern = (data: Record<string, unknown>) => {
    const text = JSON.stringify(data);
    let index = seen.get(text);
    if (index === undefined) {
      index = states.length;
      states.push(data);
      seen.set(text, index);
    }
    return index;
  };
  let floorState: number[];
  if (events.length === 0) {
    floorState = [intern(buildMvuData(liveTree, valueTypes, macros))];
  } else if (active) {
    floorState = events.map((event) => intern(event.message_vars ?? {}));
  } else {
    const bySource = new Map<string, number>();
    floorState = floorSources(events).map((source) => {
      const key = String(source);
      const cached = bySource.get(key);
      if (cached !== undefined) return cached;
      const tree =
        source === "live" ? liveTree : source === "empty" ? {} : ((events[source].state?.tree ?? {}) as StateTree);
      const index = intern(buildMvuData(tree, valueTypes, macros));
      bySource.set(key, index);
      return index;
    });
  }
  const targets =
    events.length === 0
      ? [null]
      : events.map((event, floor) => ({
          key: event.id ?? `@${input.positions?.[floor] ?? floor}`,
          rev: event.vars_rev ?? null,
        }));
  let latestId = -1;
  input.roles.forEach((role, index) => {
    if (role !== "system") latestId = index;
  });
  return {
    currentId: input.currentId,
    latestId,
    states,
    floorState,
    targets,
    active,
    generation: input.generation ?? -1,
    scene: input.scene ?? 0,
    layers: input.layers ?? {},
    characterId: input.characterId ?? "",
  };
}

/** 這一樓的 stat_data 有沒有東西（MVU 只在已有 stat_data 時補占位） */
export function hasStatData(mvu: CardMvu, floor: number): boolean {
  const stat = mvu.states[mvu.floorState[floor]]?.stat_data;
  return stat !== null && typeof stat === "object" && Object.keys(stat).length > 0;
}

/**
 * MVU 處理完一則收到的 AI 樓（非玩家）、已有 stat_data、內容不短於 5 字且還沒有占位時，在樓尾補
 * `\n\n<StatusPlaceHolderImpl/>`。只有 AI 回覆會觸發這段處理，app 的系統事件不算。開場白（post_opening
 * 寫的那則，事件帶 opening）由 initvar 初始化、不經這段處理，不補；沒有開場、第一樓就是 GM 回覆時照補。
 */
export function withMvuPlaceholder(message: string, opening: boolean, role: string, hasStat: boolean): string {
  if (opening || role !== "assistant" || !hasStat) return message;
  if (message.length < 5 || message.includes(MVU_PLACEHOLDER)) return message;
  return `${message}\n\n${MVU_PLACEHOLDER}`;
}

export { buildMvuShimSource } from "./card-mvu-shim-source";
