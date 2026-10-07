// 一個回合結束後逐字稿怎麼收（純函式）。取消分兩種（計畫包 1）：
// - 停止並保留：已有正文就留下、標「回應中斷」，玩家句不刪（照 ai-response-stop〔作者裁決 2026-10-01〕）。
// - 取消未完成回合：一個字都還沒出來就停，玩家句自動收回、原文放回輸入框
//   ——網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是讓玩家手動收回。
// 失敗（含換模後仍失敗）比照桌面版失敗處理：收回玩家句、原文放回輸入框，半截不留。
// 重新生成：先拿掉最後一則回覆再送；沒拿到新回覆（取消或失敗）就把原回覆放回去，不丟資料。
import type { CallOutcome } from "../openrouter/smart-call";

export interface ChatEntry {
  id: string;
  role: "user" | "char";
  text: string;
  /** 回應中途被停止或被供應商截斷。 */
  interrupted?: boolean;
  opening?: boolean;
  /** 落進逐字稿的時間（毫秒）；{{idle_duration}} 用 */
  sentAt?: number;
}

/** 這一輪是怎麼來的：玩家送出一句，或對最後一則重新生成。 */
export type PendingTurn =
  | { kind: "send"; userEntry: ChatEntry; rawInput: string }
  | { kind: "regenerate"; replaced: ChatEntry | null };

export interface TurnResult {
  entries: ChatEntry[];
  /** 要放回輸入框的原文；null＝不動輸入框。 */
  restoreInput: string | null;
  /** 失敗時的錯誤字串（給 error-text 翻成玩家看得懂的話）。 */
  error: string | null;
}

/**
 * `before`：送出時是含玩家句的逐字稿；重新生成時是拿掉舊回覆後的逐字稿。
 * `finish`：模型原文 → 存進逐字稿的文字（套一般 regex）。
 */
export function resolveTurn(
  before: ChatEntry[],
  pending: PendingTurn,
  outcome: CallOutcome,
  newId: () => string,
  finish: (text: string) => string = (text) => text,
  now: number = Date.now(),
): TurnResult {
  const reply = (text: string, interrupted: boolean): ChatEntry => ({ id: newId(), role: "char", text: finish(text), interrupted, sentAt: now });
  const nothing = (error: string | null): TurnResult =>
    pending.kind === "send"
      ? { entries: before.filter((entry) => entry.id !== pending.userEntry.id), restoreInput: pending.rawInput, error }
      : { entries: pending.replaced ? [...before, pending.replaced] : before, restoreInput: null, error };
  switch (outcome.kind) {
    case "ok":
      return { entries: [...before, reply(outcome.text, outcome.truncated !== null)], restoreInput: null, error: null };
    case "aborted":
      if (outcome.text.trim() !== "") return { entries: [...before, reply(outcome.text, true)], restoreInput: null, error: null };
      return nothing(null);
    case "error":
      return nothing(outcome.display);
  }
}

/** 重新生成的起點：最後一則是模型回覆（非開場白）就拿掉它；最後一則是玩家句就直接再生；其餘不能重生。 */
export function regenerateBase(entries: ChatEntry[]): { before: ChatEntry[]; replaced: ChatEntry | null } | null {
  const last = entries[entries.length - 1];
  if (!last || last.opening) return null;
  if (last.role === "user") return { before: entries, replaced: null };
  return { before: entries.slice(0, -1), replaced: last };
}

export function deleteLast(entries: ChatEntry[]): ChatEntry[] {
  return entries.slice(0, -1);
}

export function replaceLast(entries: ChatEntry[], text: string): ChatEntry[] {
  const last = entries[entries.length - 1];
  if (!last) return entries;
  return [...entries.slice(0, -1), { ...last, text }];
}
