// MVU 開局初始化（MagVarUpdate 438f9ffc `initvar/variable_init.ts` 的 initCheck／loadInitVarData，只當規格書讀）：
// 卡內世界書標題含 `[initvar]` 的條目（停用的也算）依序解析成 stat_data 合併，生成 schema、清掉元資料；
// 開場白裡的 `<initvar>` 區塊整份取代世界書的初始值；最後把開場白本身的更新指令套上去，就是第 0 樓的變數表。
// 解析任何一條失敗，這本書整本不算初始化（MVU 記錯誤後略過），這桌就沒有卡片變數。
import { correctlyMerge, initvarBody, parseStructured } from "@desktop/features/mvu-engine/initvar";
import { bookEntries } from "../sillytavern/world-info-book";
import type { HostMvu, MvuTable } from "./engine";

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
