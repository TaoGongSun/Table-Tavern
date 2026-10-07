// 依深度插進對話的提示（ST 的 extension prompts，IN_CHAT）：作者註記（世界書「作者註記上／下」條目）、
// 卡的 depth_prompt、世界書「依深度插入」條目。照釘版本 06bde939 openai.js populationInjectionPrompts 與
// script.js getExtensionPrompt：同深度同角色的照鍵名排序、各自去頭尾空白後以換行接起來、整段代換一次巨集；
// 每個深度依 system→user→assistant 各成一則，插在「倒數第 n 則之前」（深度 0＝最後一則之後）。
import type { ChatMessage } from "../openrouter/stream-chat";

const MAX_INJECTION_DEPTH = 10_000;
const ROLES: ChatMessage["role"][] = ["system", "user", "assistant"];
/** ST extension_prompt_roles：0 system、1 user、2 assistant */
export const roleName = (role: number): ChatMessage["role"] => ROLES[role] ?? "system";

export interface ExtensionPrompt {
  /** ST 的鍵名（排序用）：作者註記 `2_floating_prompt`、卡 `DEPTH_PROMPT`、世界書 `customDepthWI_<深度>_<角色>` */
  key: string;
  value: string;
  depth: number;
  role: ChatMessage["role"];
}

export type HistoryMessage = ChatMessage & { injected?: boolean };

/** 作者註記：原本的註記是空的，只剩世界書上／下兩組（authors-note 的 ANWithWI）。預設深度 4、system。 */
export function authorsNotePrompt(top: string[], bottom: string[]): ExtensionPrompt {
  const value = `${top.join("\n")}\n\n${bottom.join("\n")}`.replace(/(^\n)|(\n$)/g, "");
  return { key: "2_floating_prompt", value, depth: 4, role: "system" };
}

export function injectExtensionPrompts(
  history: HistoryMessage[],
  prompts: ExtensionPrompt[],
  substitute: (text: string) => string,
): HistoryMessage[] {
  const newestFirst = [...history].reverse();
  const live = prompts.filter((prompt) => prompt.value && Number.isInteger(prompt.depth) && prompt.depth >= 0 && prompt.depth <= MAX_INJECTION_DEPTH);
  const depths = [...new Set(live.map((prompt) => prompt.depth))].sort((a, b) => a - b);
  let inserted = 0;
  for (const depth of depths) {
    const messages: HistoryMessage[] = [];
    for (const role of ROLES) {
      const joined = live
        .filter((prompt) => prompt.depth === depth && prompt.role === role)
        .sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0))
        .map((prompt) => prompt.value.trim())
        .join("\n");
      const content = (joined.length ? substitute(joined) : "").trim();
      if (content) messages.push({ role, content, injected: true });
    }
    if (messages.length) {
      newestFirst.splice(depth + inserted, 0, ...messages);
      inserted += messages.length;
    }
  }
  return newestFirst.reverse();
}
