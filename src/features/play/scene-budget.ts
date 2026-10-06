// 換幕容量（long-prompt-scene-hint 範圍 3）：後端 `scene_budget` 回傳的形狀，與鎖的即時判定。
// 鎖的式子與後端關卡（src-tauri/src/scene_budget）同一個：H_S＋當次本句＋G_reply > cap_S，且可鎖。
// 前端只是即時提示，送出時以後端關卡為準。

export type SceneBudgetUnit = "tokens" | "bytes";

export interface SummaryBudget {
  unit: SceneBudgetUnit;
  used: number;
  cap: number;
  hint: boolean;
  ratio: number | null;
  reliable: boolean;
  lockable: boolean;
  gReply: number;
  over: boolean;
}

export interface SceneBudgetReply {
  worldId: string;
  configGen: string;
  /** 請求時帶的設定指紋，原樣帶回 */
  configTag: string;
  requestSeq: number;
  scene: number;
  summary: SummaryBudget | null;
  chatHint: boolean;
}

/** 與 Rust `scene_budget::estimate::budget_tokens` 同係數的保守估計 */
export function budgetTokens(text: string): number {
  let total = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0)!;
    if (code > 0x7f) total += 1.45;
    else if (/[A-Za-z]/.test(ch)) total += 0.3;
    else if (/[0-9]/.test(ch)) total += 1.0;
    else if (/\s/.test(ch)) total += 0.15;
    else total += 0.7;
  }
  // 與 Rust 的 f64 累加同順序、同 ceil
  return Math.ceil(total);
}

const DRAFT_OVERHEAD_TOKENS = 16;
const DRAFT_OVERHEAD_BYTES = 64;

/** 當次本句在換幕單位下的量；空字串＝無玩家句的動作 */
export function draftSize(text: string, unit: SceneBudgetUnit, ratio: number): number {
  if (text === "") return 0;
  if (unit === "bytes") return new TextEncoder().encode(text).length + DRAFT_OVERHEAD_BYTES;
  return Math.ceil(budgetTokens(text) * ratio) + DRAFT_OVERHEAD_TOKENS;
}

/** 再送這一句（或無玩家句的動作）會不會讓換幕一次送不出去 */
export function wouldOverflow(summary: SummaryBudget | null | undefined, draft: string): boolean {
  if (!summary || !summary.lockable) return false;
  const size = draftSize(draft.trim(), summary.unit, summary.ratio ?? 1);
  return summary.used + size + summary.gReply > summary.cap;
}

/** 提醒列要出哪一種：換幕呼叫觸發優先（文案說得出「換幕整理」），只因聊天觸發用較中性的說法 */
export function capacityHint(budget: SceneBudgetReply | null): "summary" | "chat" | null {
  if (!budget) return null;
  if (budget.summary?.hint) return "summary";
  if (budget.chatHint) return "chat";
  return null;
}
