// 包 1 的最小提示組裝：照 ST Chat Completion 預設順序（main prompt → description → personality →
// scenario → 歷史 → post_history_instructions），巨集只換 {{char}}／{{user}}。完整 ST 行為
// （範例對話 <START> 切段、depth_prompt、送模前 regex、完整巨集引擎、世界書）在包 2、5 依卡片契約補齊。
import type { CharacterCardData } from "../cards/sample-card";
import type { ChatMessage } from "../openrouter/stream-chat";
import type { ChatEntry } from "./chat-turn";

/** ST 預設 main prompt。卡片有 system_prompt 時改用卡片的（{{original}} 代入預設）。 */
const DEFAULT_MAIN_PROMPT = "Write {{char}}'s next reply in a fictional chat between {{char}} and {{user}}.";

export function replaceNameMacros(text: string, charName: string, userName: string): string {
  return text
    .replace(/\{\{char\}\}|<BOT>/gi, charName)
    .replace(/\{\{user\}\}|<USER>/gi, userName);
}

export function buildMessages(card: CharacterCardData, userName: string, entries: ChatEntry[]): ChatMessage[] {
  const fill = (text: string) => replaceNameMacros(text, card.name, userName).trim();
  const system: string[] = [];
  const main = card.system_prompt.trim()
    ? card.system_prompt.replace(/\{\{original\}\}/gi, DEFAULT_MAIN_PROMPT)
    : DEFAULT_MAIN_PROMPT;
  system.push(fill(main));
  if (card.description.trim()) system.push(fill(card.description));
  if (card.personality.trim()) system.push(fill(`{{char}}'s personality: ${card.personality}`));
  if (card.scenario.trim()) system.push(fill(`Scenario: ${card.scenario}`));

  const messages: ChatMessage[] = system.map((content) => ({ role: "system", content }));
  for (const entry of entries) {
    messages.push({ role: entry.role === "user" ? "user" : "assistant", content: entry.text });
  }
  if (card.post_history_instructions.trim()) {
    messages.push({ role: "system", content: fill(card.post_history_instructions) });
  }
  return messages;
}
