// 一個回合結束後逐字稿怎麼收（純函式）。取消分兩種（計畫包 1）：
// - 停止並保留：已有正文就留下、標「回應中斷」，玩家句不刪（照 ai-response-stop〔作者裁決 2026-10-01〕）。
// - 取消未完成回合：一個字都還沒出來就停，玩家句自動收回、原文放回輸入框
//   ——網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是讓玩家手動收回。
// 失敗（含換模後仍失敗）比照桌面版失敗處理：收回玩家句、原文放回輸入框，半截不留。
import type { CallOutcome } from "../openrouter/smart-call";

export interface ChatEntry {
  id: string;
  role: "user" | "char";
  text: string;
  /** 回應中途被停止或被供應商截斷。 */
  interrupted?: boolean;
  opening?: boolean;
}

export interface TurnResult {
  entries: ChatEntry[];
  /** 要放回輸入框的原文；null＝不動輸入框。 */
  restoreInput: string | null;
  /** 失敗時的錯誤字串（給 error-text 翻成玩家看得懂的話）。 */
  error: string | null;
}

export function resolveTurn(
  entries: ChatEntry[],
  userEntry: ChatEntry,
  outcome: CallOutcome,
  newId: () => string,
): TurnResult {
  const withoutUser = entries.filter((entry) => entry.id !== userEntry.id);
  switch (outcome.kind) {
    case "ok":
      return {
        entries: [...entries, { id: newId(), role: "char", text: outcome.text, interrupted: outcome.truncated !== null }],
        restoreInput: null,
        error: null,
      };
    case "aborted":
      if (outcome.text.trim() !== "") {
        return {
          entries: [...entries, { id: newId(), role: "char", text: outcome.text, interrupted: true }],
          restoreInput: null,
          error: null,
        };
      }
      return { entries: withoutUser, restoreInput: userEntry.text, error: null };
    case "error":
      return { entries: withoutUser, restoreInput: userEntry.text, error: outcome.display };
  }
}
