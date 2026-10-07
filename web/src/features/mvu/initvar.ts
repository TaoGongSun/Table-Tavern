// MVU 開局初始化（MagVarUpdate 438f9ffc `initvar/variable_init.ts` 的 initCheck／loadInitVarData，只當規格書讀）：
// 卡內世界書標題含 `[initvar]` 的條目（停用的也算）依序解析成 stat_data 合併，生成 schema、清掉元資料；
// 開場白裡的 `<initvar>` 區塊整份取代世界書的初始值；最後把開場白本身的更新指令套上去，就是第 0 樓的變數表。
// 解析任何一條失敗，這本書整本不算初始化（MVU 記錯誤後略過），這桌就沒有卡片變數。
import JSON5 from "json5";
import { jsonrepair } from "jsonrepair";
import lodash from "lodash";
import YAML from "yaml";
import { bookEntries } from "../sillytavern/world-info-book";
import type { HostMvu, MvuTable } from "./engine";

/** MVU util/common parseString：YAML 優先（`[`／`{` 開頭的改成最後才試），再 JSON5、jsonrepair */
export function parseStructured(content: string): unknown {
  const jsonFirst = /^[[{]/.test(content.trimStart());
  const yaml = () => YAML.parseDocument(content, { merge: true }).toJS();
  if (!jsonFirst) {
    try {
      return yaml();
    } catch {
      // 換下一種
    }
  }
  try {
    return JSON5.parse(content);
  } catch {
    // 換下一種
  }
  try {
    return JSON.parse(jsonrepair(content));
  } catch (error) {
    if (!jsonFirst) throw error;
  }
  return yaml();
}

/** 陣列整個取代、其餘深合併（MVU correctlyMerge） */
export function correctlyMerge<T extends object>(target: T, source: unknown): T {
  return lodash.mergeWith(target, source, (_left: unknown, right: unknown) => (Array.isArray(right) ? right : undefined));
}

/** 條目內容：`<initvar>` 包住或程式碼區塊包住的取裡面那段 */
function initvarBody(content: string): string {
  let body = content;
  const xml = body.trim().match(/.*<initvar>.*\n([\s\S]*)\n.*<\/initvar>.*/m);
  if (xml) body = xml[1];
  const fenced = body.trim().match(/```.*\n([\s\S]*)\n```/m);
  if (fenced) body = fenced[1];
  return body;
}

export interface InitvarEntry {
  comment: string;
  content: string;
}

/** 卡內世界書（角色書）裡標題含 `[initvar]` 的條目，照 ST 載入後的先後 */
export function initvarEntries(cardData: unknown): InitvarEntry[] {
  return bookEntries(cardData)
    .filter(({ fields }) => fields.comment.toLowerCase().includes("[initvar]"))
    .map(({ fields }) => ({ comment: fields.comment, content: fields.content }));
}

export type InitResult = { ok: true; table: MvuTable } | { ok: false; comment: string; error: string };

/**
 * 開局的變數表（還沒套開場白）：`substitute`＝ST 的巨集代換（MVU substitudeMacros）。
 * bookName＝這本書在 initialized_lorebooks 裡的名字。
 */
export function initialTable(entries: InitvarEntry[], bookName: string, substitute: (text: string) => string, mvu: HostMvu): InitResult {
  const merged: MvuTable = {};
  for (const entry of entries) {
    let parsed: unknown;
    try {
      parsed = parseStructured(substitute(initvarBody(entry.content)));
    } catch (error) {
      return { ok: false, comment: entry.comment, error: String(error) };
    }
    if (parsed) correctlyMerge(merged, parsed);
  }
  const emptySchema = { type: "object", properties: {} };
  const { stat, schema } = mvu.initializeSchema(merged, emptySchema);
  return {
    ok: true,
    table: { display_data: {}, initialized_lorebooks: { [bookName]: [] }, stat_data: stat, delta_data: {}, schema },
  };
}

/** 開場白裡的 `<initvar>` 區塊：有就整份取代世界書給的初始值（解析失敗的區塊略過）；沒有回 null */
export function openingOverride(opening: string, substitute: (text: string) => string): MvuTable | null {
  const override: MvuTable = {};
  let applied = false;
  for (const match of opening.matchAll(/<(initvar)>(?:\s*```.*)?([\s\S]*?)(?:```\s*)?<\/\1>/gim)) {
    try {
      correctlyMerge(override, parseStructured(substitute(match[2])));
      applied = true;
    } catch {
      // MVU 記錯誤後略過這一塊
    }
  }
  return applied ? override : null;
}
