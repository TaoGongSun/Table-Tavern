// 訊息文字在各時機的 ST 處理（釘版本 06bde939）：
// - 玩家送出：一般 regex（placement 1）→ 巨集代換，存的是處理後的字（sendMessageAsUser）。
// - 模型回覆：一般 regex（placement 2），不做巨集代換。
// - 開場白：只套一般 regex（placement 2），巨集保留原文；第 0 則每次顯示都重新代換（messageFormatting），
//   每次送出前把第 0 則代換結果寫回逐字稿（Generate 的 chat[0].mes = substituteParams(chat[0].mes)）。
// - 編輯：只跑 runOnEdit 的腳本 → 去頭尾空白 → 巨集代換。
// - 顯示：markdownOnly 腳本（帶深度），不改存檔。
// 玩家沒允許這張卡的腳本（D21）時，以上每個時機都不套任何腳本。
import { activeRegexScripts, type PlayCard } from "../cards/play-card";
import type { ChatLine } from "../sillytavern/macro-engine";
import { getRegexedString, REGEX_PLACEMENT } from "../sillytavern/regex-scripts";
import { substituteParams, type MacroContext } from "../sillytavern/substitute";
import type { ChatVariables } from "../sillytavern/variables";
import type { ChatEntry } from "./chat-turn";

/** 一場對話固定的東西：卡、玩家名、變數、對話識別。 */
export interface ChatSetup {
  card: PlayCard;
  userName: string;
  variables: ChatVariables;
  chatId: string;
}

export const chatLines = (entries: ChatEntry[]): ChatLine[] =>
  entries.map((entry) => ({ isUser: entry.role === "user", text: entry.text, sentAt: entry.sentAt }));

export function macroContext(setup: ChatSetup, entries: ChatEntry[], extra: Partial<MacroContext> = {}): MacroContext {
  return {
    card: setup.card.text,
    userName: setup.userName,
    chat: chatLines(entries),
    variables: setup.variables,
    chatId: setup.chatId,
    ...extra,
  };
}

const placementOf = (role: ChatEntry["role"]) => (role === "user" ? REGEX_PLACEMENT.USER_INPUT : REGEX_PLACEMENT.AI_OUTPUT);

export function userText(setup: ChatSetup, before: ChatEntry[], raw: string): string {
  const context = macroContext(setup, before, { input: raw });
  return substituteParams(getRegexedString(raw, REGEX_PLACEMENT.USER_INPUT, activeRegexScripts(setup.card), context), context);
}

export function replyText(setup: ChatSetup, before: ChatEntry[], raw: string): string {
  return getRegexedString(raw, REGEX_PLACEMENT.AI_OUTPUT, activeRegexScripts(setup.card), macroContext(setup, before));
}

export function openingText(setup: ChatSetup, raw: string): string {
  return getRegexedString(raw, REGEX_PLACEMENT.AI_OUTPUT, activeRegexScripts(setup.card), macroContext(setup, []));
}

/** 送出前：第 0 則換成當下的代換結果並寫回（ST Generate 對 chat[0] 的處理）。 */
export function settleFirstMessage(setup: ChatSetup, entries: ChatEntry[], extra: Partial<MacroContext> = {}): ChatEntry[] {
  const first = entries[0];
  if (!first) return entries;
  const text = substituteParams(first.text, macroContext(setup, entries, extra));
  return text === first.text ? entries : [{ ...first, text }, ...entries.slice(1)];
}

export function editedText(setup: ChatSetup, entries: ChatEntry[], role: ChatEntry["role"], raw: string): string {
  const context = macroContext(setup, entries);
  const regexed = getRegexedString(raw, placementOf(role), activeRegexScripts(setup.card), context, { isEdit: true });
  return substituteParams(regexed.trim(), context);
}

/** 第 index 則的顯示文字（深度＝從最後一則往回數）。 */
export function displayText(setup: ChatSetup, entries: ChatEntry[], index: number, extra: Partial<MacroContext> = {}): string {
  const entry = entries[index];
  const context = macroContext(setup, entries, extra);
  // ST messageFormatting：第 0 則（非玩家）顯示前先代換巨集
  const text = index === 0 && entry.role === "char" ? substituteParams(entry.text, context) : entry.text;
  return getRegexedString(text, placementOf(entry.role), activeRegexScripts(setup.card), context, {
    isMarkdown: true,
    depth: entries.length - index - 1,
  });
}

/** 送模前的歷史訊息：promptOnly 腳本（帶深度）。 */
export function promptText(setup: ChatSetup, entries: ChatEntry[], index: number, context: MacroContext): string {
  const entry = entries[index];
  return getRegexedString(entry.text, placementOf(entry.role), activeRegexScripts(setup.card), context, {
    isPrompt: true,
    depth: entries.length - index - 1,
  });
}
