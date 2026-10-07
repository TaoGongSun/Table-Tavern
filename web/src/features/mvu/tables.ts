// 一桌的卡片變數（MVU 與酒館助手變數層）：每則訊息的變數表跟著逐字稿（ChatEntry.vars），其餘層與開局種子
// 跟著這桌。這裡是不帶大型依賴的純邏輯：哪一則算有效表、樓尾占位、交給卡片 iframe 的快照、卡片寫入。
// 規格對照 MagVarUpdate 438f9ffc 與 JS-Slash-Runner 46ec10df（只當規格書讀），沙盒墊片沿用桌面版。
import type { CardMvu, MvuLayer } from "@desktop/features/card-interface/mvu/card-mvu-shim";
import { MVU_PLACEHOLDER } from "@desktop/features/card-interface/mvu/card-mvu-shim";
import { parseLayerKey, validateTable } from "@desktop/features/card-interface/mvu/card-mvu-write";
import type { ChatEntry } from "../chat/chat-turn";

export type VarsTable = Record<string, unknown>;

/** 訊息層以外、跟著這桌的層（chat、global 是 ST 的聊天變數，另外存） */
export interface ExtraLayers {
  character: VarsTable;
  preset: VarsTable;
  script: Record<string, VarsTable>;
  extension: Record<string, VarsTable>;
}

export const EMPTY_LAYERS: ExtraLayers = { character: {}, preset: {}, script: {}, extension: {} };

/** 網頁版只有一張卡：character 層的身分固定這個 */
export const CHARACTER_ID = "card";

const isObject = (value: unknown): value is VarsTable => typeof value === "object" && value !== null && !Array.isArray(value);

/** MVU isMvuData：有 stat_data 也有 schema 才算一張有效的變數表 */
export function isMvuData(vars: unknown): boolean {
  return isObject(vars) && vars.stat_data !== undefined && vars.schema !== undefined;
}

/** MVU getLastValidVariable：[0, end) 裡最後一則帶有效表的那張（深拷貝）；沒有回 undefined */
export function lastValidVars(entries: ChatEntry[], end: number): VarsTable | undefined {
  for (let index = Math.min(end, entries.length) - 1; index >= 0; index--) {
    const vars = entries[index].vars;
    if (isMvuData(vars)) return structuredClone(vars) as VarsTable;
  }
  return undefined;
}

/** 酒館助手類巨集的 message 層：最後一則帶變數表（任何物件）的訊息那張 */
export function latestMessageVars(entries: ChatEntry[]): VarsTable {
  for (let index = entries.length - 1; index >= 0; index--) {
    const vars = entries[index].vars;
    if (isObject(vars)) return vars;
  }
  return {};
}

/**
 * MVU 處理完一則 AI 回覆後改寫訊息（handleVariablesInMessage）：沒有占位就在樓尾補 `\n\n<StatusPlaceHolderImpl/>`，
 * 再拿掉 `<status_current_variable>` 區塊。
 */
export function finishReplyText(text: string): string {
  let result = text.includes(MVU_PLACEHOLDER) ? text : `${text}\n\n${MVU_PLACEHOLDER}`;
  if (result.includes("<status_current_variable>")) {
    result = result.replace(/<(status_current_variable)>(?:(?!<\1>).)*<\/\1?>/gis, "");
  }
  return result;
}

/** MVU filterPrompts：送模前每則訊息拿掉 `\n<StatusPlaceHolderImpl/>` */
export function withoutPlaceholder(content: string): string {
  return content.split(`\n${MVU_PLACEHOLDER}`).join("");
}

/**
 * 各寫入目標的版本：一個目標每次被看到的內容跟上次不同（整份 JSON 逐字比對）就發一個新的版本號；內容沒變就是
 * 同一版（沙盒拿那一版當預期版本寫回仍算數）。版本號是遞增流水號，不是內容雜湊，不會兩份不同的內容撞同一版。
 * null＝這個目標沒有表。
 */
export class Revisions {
  private readonly seen = new Map<string, { text: string; rev: string }>();
  private counter = 0;
  private readonly prefix = Math.random().toString(36).slice(2, 8);

  of(key: string, table: unknown): string | null {
    if (table === undefined || table === null) return null;
    const text = JSON.stringify(table) ?? "";
    const known = this.seen.get(key);
    if (known && known.text === text) return known.rev;
    this.counter += 1;
    const rev = `${this.prefix}-${this.counter}`;
    this.seen.set(key, { text, rev });
    return rev;
  }
}

/** 這桌各層現在的值，key 與沙盒的寫入目標同一套（`chat`、`global`、`character:card`、`preset`、`script:<id>`、`extension:<id>`） */
export function layerTables(chat: VarsTable, global: VarsTable, extra: ExtraLayers): Record<string, VarsTable> {
  const tables: Record<string, VarsTable> = { chat, global, [`character:${CHARACTER_ID}`]: extra.character, preset: extra.preset };
  for (const [id, vars] of Object.entries(extra.script)) tables[`script:${id}`] = vars;
  for (const [id, vars] of Object.entries(extra.extension)) tables[`extension:${id}`] = vars;
  return tables;
}

/** 交給卡片 iframe 的 MVU 快照（每則讀自己的表、沒有表的回 {}）；currentId 由各支 iframe 換成自己那一樓 */
export function cardMvuBase(
  entries: ChatEntry[],
  layers: Record<string, VarsTable>,
  macros: { user: string; char: string | null },
  revisions: Revisions,
): CardMvu {
  const states: VarsTable[] = [];
  const seen = new Map<string, number>();
  const floorState = entries.map((entry) => {
    const vars = isObject(entry.vars) ? entry.vars : {};
    const text = JSON.stringify(vars);
    let index = seen.get(text);
    if (index === undefined) {
      index = states.length;
      states.push(vars);
      seen.set(text, index);
    }
    return index;
  });
  const snapshotLayers: Record<string, MvuLayer> = {};
  for (const [key, vars] of Object.entries(layers)) snapshotLayers[key] = { rev: revisions.of(key, vars), vars };
  return {
    currentId: 0,
    latestId: entries.length - 1,
    states,
    floorState,
    targets: entries.map((entry) => ({ key: entry.id, rev: revisions.of(entry.id, entry.vars) })),
    active: true,
    generation: 0,
    scene: 0,
    layers: snapshotLayers,
    characterId: CHARACTER_ID,
    macros,
  };
}

/** 卡片寫入要動的地方：訊息層用訊息 id，其餘層用層的 key */
export interface WriteTarget {
  /** 現在的值；undefined＝找不到這個目標 */
  read: (key: string) => { found: boolean; table: VarsTable | null };
  write: (key: string, table: VarsTable) => void;
}

/**
 * 一筆卡片寫入（沙盒 replaceVariables／Mvu.replaceMvuData 送來的整張表）：形狀不對回 null（不理）；否則驗上限、
 * 比對預期版本（Revisions），相符才寫，回要送回沙盒的結算（被拒時附上目標現在的值與版本）。網頁版寫入是同步的，不必排隊。
 */
export function cardWrite(data: Record<string, unknown>, target: WriteTarget, revisions: Revisions): Record<string, unknown> | null {
  const { requestId, target: key, payload, base, generation, scene } = data;
  if (typeof requestId !== "string" || typeof key !== "string" || typeof payload !== "string") return null;
  if (!(typeof base === "string" || base === null) || !Number.isInteger(generation) || !Number.isInteger(scene)) return null;
  const reject = (error: string, withAuthority: boolean) => {
    const current = target.read(key);
    return {
      kind: "mvu-settle",
      results: [{ requestId, ok: false, error }],
      ...(withAuthority ? { authority: { key, table: current.found ? current.table : null, rev: current.found ? revisions.of(key, current.table) : null } } : {}),
    };
  };
  const problem = validateTable(payload);
  if (problem !== null) return reject(problem, true);
  if (generation !== 0 || scene !== 0) return reject("stale", false);
  const layer = parseLayerKey(key);
  if (layer?.layer === "character" && layer.id !== CHARACTER_ID) return reject("bad-target", true);
  const current = target.read(key);
  if (!current.found) return reject("bad-target", true);
  if (revisions.of(key, current.table) !== base) return reject("stale", true);
  const table = JSON.parse(payload) as VarsTable;
  target.write(key, table);
  return { kind: "mvu-settle", results: [{ requestId, ok: true, rev: revisions.of(key, table) }] };
}
