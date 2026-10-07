// MVU 與酒館助手類巨集的執行期（lodash、yaml、json5、jsonrepair 都在這一包，用得到的桌才載）：
// 開局初始化（initvar.ts）與收到 AI 回覆時的變數更新（MVU handleVariablesInMessage）。
import type { PlayCard } from "../cards/play-card";
import type { ChatEntry } from "../chat/chat-turn";
import { createHostMvu, type HostMvu, type MvuMacros } from "./engine";
import type { Evaluator } from "./evaluate";
import { correctlyMerge, initialTable, initvarEntries, openingOverride } from "./initvar";
import { lastValidVars, finishReplyText, type VarsTable } from "./tables";

export { replaceMacroLike, type LayerReader, type VariableLayer } from "./macro-like";

export interface MvuRuntime {
  mvu: HostMvu;
}

export function createRuntime(evaluator: Evaluator, macros: MvuMacros): MvuRuntime {
  return { mvu: createHostMvu(evaluator, macros) };
}

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

/** 卡內世界書在 initialized_lorebooks 裡的名字（ST 匯入卡片時給嵌入書的名字） */
function bookName(card: PlayCard): string {
  const shell = card.shell as Record<string, unknown>;
  const data = isObject(shell?.data) ? shell.data : shell;
  const book = isObject(data?.character_book) ? data.character_book : null;
  return typeof book?.name === "string" && book.name.trim() !== "" ? book.name : `${card.text.name}'s Lorebook`;
}

function cardData(card: PlayCard): unknown {
  const shell = card.shell as Record<string, unknown>;
  return isObject(shell?.data) ? shell.data : shell;
}

export type InitOutcome =
  | { ok: true; table: VarsTable; opening: VarsTable | null }
  | { ok: false; comment: string; error: string };

/**
 * 開局（MVU initCheck）：世界書 initvar → 變數表；有開場白就以它為第 0 樓——開場白的 `<initvar>` 區塊整份取代
 * 初始值，再套開場白本身的更新指令。`substitute`＝ST 巨集代換。
 */
export async function initializeVariables(
  runtime: MvuRuntime,
  card: PlayCard,
  opening: string | null,
  substitute: (text: string) => string,
): Promise<InitOutcome> {
  const name = bookName(card);
  const init = initialTable(initvarEntries(cardData(card)), name, substitute, runtime.mvu);
  if (!init.ok) return init;
  if (opening === null) return { ok: true, table: init.table, opening: null };
  const current = correctlyMerge({}, structuredClone(init.table)) as VarsTable;
  const override = openingOverride(opening, substitute);
  if (override !== null) {
    current.stat_data = override;
    current.initialized_lorebooks = { [name]: [] };
  }
  let table = current;
  try {
    table = await runtime.mvu.parseMessage(opening, current);
  } catch (error) {
    console.error("[table-tavern] 開場白的變數指令處理失敗", error);
  }
  return { ok: true, table, opening: table };
}

/**
 * 收到一則 AI 回覆（MVU handleVariablesInMessage）：至少 5 個字、前面有帶 stat_data 的有效表（沒有就用開局種子）
 * 才處理——套上這則的更新指令，把結果寫進這則的變數表，並在樓尾補狀態欄占位。不處理或處理失敗回 null。
 */
export async function processReply(runtime: MvuRuntime, entries: ChatEntry[], index: number, seed: VarsTable | null): Promise<ChatEntry | null> {
  const entry = entries[index];
  if (!entry || entry.role !== "char" || entry.text.length < 5) return null;
  const base = lastValidVars(entries, index) ?? (seed === null ? undefined : (structuredClone(seed) as VarsTable));
  if (base === undefined || !("stat_data" in base)) return null;
  let next: VarsTable;
  try {
    next = await runtime.mvu.parseMessage(entry.text, base);
  } catch (error) {
    console.error("[table-tavern] 這則回覆的變數指令處理失敗", error);
    return null;
  }
  // MVU 寫回時只換這幾個欄位，其餘照這則原本的表
  const vars: VarsTable = { ...(entry.vars ?? {}) };
  vars.initialized_lorebooks = next.initialized_lorebooks;
  vars.stat_data = next.stat_data;
  for (const field of ["schema", "display_data", "delta_data"]) {
    if (next[field] !== undefined) vars[field] = next[field];
    else delete vars[field];
  }
  const text = finishReplyText(entry.text);
  return { ...entry, text, vars, ...(text === entry.text ? {} : { raw: entry.raw ?? entry.text }) };
}
