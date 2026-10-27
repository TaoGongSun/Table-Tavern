// MVU 與酒館助手類巨集的執行期（lodash、yaml、json5、jsonrepair 都在這一包，用得到的桌才載）：
// 開局初始化（initvar.ts）與收到 AI 回覆時的變數更新（MVU handleVariablesInMessage）。
// zod 卡（卡上有註冊 schema 的腳本）改走隔離 Worker（zod/frame-host.ts），整次失敗丟 ZodRunError、不退回 MagVarUpdate。
import type { PlayCard } from "../cards/play-card";
import type { ChatEntry } from "../chat/chat-turn";
import { createHostMvu, type HostMvu, type MvuMacros } from "./engine";
import type { Evaluator } from "./evaluate";
import { correctlyMerge, initvarBody, openingOverride, parseStructured } from "@desktop/features/mvu-engine/initvar";
import { zodNotes } from "@desktop/features/mvu-engine/zod-feedback";
import { initialTable, initvarEntries } from "./initvar";
import { lastValidVars, finishReplyText, type VarsTable } from "./tables";
import type { ZodRunner } from "./zod/frame-host";

export { replaceMacroLike, type LayerReader, type VariableLayer } from "./macro-like";
export { createZodRunner, ZodRunError } from "./zod/frame-host";

export interface MvuRuntime {
  mvu: HostMvu;
  macros: MvuMacros;
  /** zod 桌才有：這桌的隔離引擎（呼叫端管生命週期） */
  zod: (() => ZodRunner) | null;
}

export function createRuntime(evaluator: Evaluator, macros: MvuMacros, zod: (() => ZodRunner) | null = null): MvuRuntime {
  return { mvu: createHostMvu(evaluator, macros), macros, zod };
}

/** 上游 mvu_zod 每次更新收尾時放進 schema 欄的佔位字串（zod 桌不生成 MVU schema） */
const ZOD_PLACEHOLDER_SCHEMA = "没有用别管这个";

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
  | {
      ok: true;
      table: VarsTable;
      /** 開場那一樓的表；null＝沒有開場白，或 zod 桌開場白的更新整次失敗（那一樓不寫變數） */
      opening: VarsTable | null;
      /** zod 桌：初始值沒過 schema（照原值）；開場白的更新整次失敗；開場白指令被擋下的回饋（第一輪送 GM） */
      zodInitInvalid?: boolean;
      openingFailed?: boolean;
      openingNotes?: string[];
    }
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
  if (runtime.zod) return initializeZod(runtime, runtime.zod(), name, card, opening, substitute);
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
 * zod 桌開局（規格 1.3、4 節，與桌面同序）：世界書 initvar → 開場白 `<initvar>` 覆寫 → cleanUpMetadata → zodInitialize
 * （只做一次；續玩、收回不重跑）→ 開場白本身的指令走 zod 迴圈。驗證不過照原值並提示；開局整次失敗丟 ZodRunError
 * （呼叫端在玩家下一次操作時重試）；開場白的更新整次失敗時那一樓不寫變數、表停在開局值。
 */
async function initializeZod(
  runtime: MvuRuntime,
  zod: ZodRunner,
  name: string,
  card: PlayCard,
  opening: string | null,
  substitute: (text: string) => string,
): Promise<InitOutcome> {
  const merged: VarsTable = {};
  for (const entry of initvarEntries(cardData(card))) {
    let parsed: unknown;
    try {
      parsed = parseStructured(substitute(initvarBody(entry.content)));
    } catch (error) {
      return { ok: false, comment: entry.comment, error: String(error) };
    }
    if (parsed) correctlyMerge(merged, parsed);
  }
  const override = opening === null ? null : openingOverride(opening, substitute);
  const initialized = await zod.initialize(override ?? merged);
  const table: VarsTable = {
    display_data: {},
    initialized_lorebooks: { [name]: [] },
    stat_data: initialized.stat_data,
    delta_data: {},
    schema: ZOD_PLACEHOLDER_SCHEMA,
  };
  const zodInitInvalid = !initialized.outcome.ok;
  if (zodInitInvalid) console.error("[table-tavern] zod 卡的初始值沒過 schema，照原值", initialized.outcome);
  if (opening === null) return { ok: true, table, opening: null, zodInitInvalid };
  try {
    const result = await zod.parse(opening, structuredClone(table), runtime.macros);
    const openingNotes = zodNotes(result.results);
    return { ok: true, table: result.table as VarsTable, opening: result.table as VarsTable, zodInitInvalid, openingNotes };
  } catch (error) {
    console.error("[table-tavern] 開場白的變數指令處理失敗", error);
    return { ok: true, table, opening: null, zodInitInvalid, openingFailed: true };
  }
}

/**
 * 收到一則 AI 回覆（MVU handleVariablesInMessage）：至少 5 個字、前面有帶 stat_data 的有效表（沒有就用開局種子）
 * 才處理——套上這則的更新指令，把結果寫進這則的變數表，並在樓尾補狀態欄占位。不處理或處理失敗回 null；zod 桌整次失敗丟 ZodRunError。
 */
export async function processReply(runtime: MvuRuntime, entries: ChatEntry[], index: number, seed: VarsTable | null): Promise<ChatEntry | null> {
  const entry = entries[index];
  if (!entry || entry.role !== "char" || entry.text.length < 5) return null;
  const base = lastValidVars(entries, index) ?? (seed === null ? undefined : (structuredClone(seed) as VarsTable));
  if (base === undefined || !("stat_data" in base)) return null;
  let next: VarsTable;
  let notes: string[] = [];
  if (runtime.zod) {
    // zod 桌整次失敗照拋（ZodRunError）：呼叫端提示、這則不寫變數。逐條被擋下的回饋掛在這則上，下一輪送 GM
    const result = await runtime.zod().parse(entry.text, base, runtime.macros);
    next = result.table as VarsTable;
    notes = zodNotes(result.results);
  } else {
    try {
      next = await runtime.mvu.parseMessage(entry.text, base);
    } catch (error) {
      console.error("[table-tavern] 這則回覆的變數指令處理失敗", error);
      return null;
    }
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
  const { mvuNotes: _previous, ...rest } = entry;
  return {
    ...rest,
    text,
    vars,
    ...(text === entry.text ? {} : { raw: entry.raw ?? entry.text }),
    ...(notes.length > 0 ? { mvuNotes: notes } : {}),
  };
}
