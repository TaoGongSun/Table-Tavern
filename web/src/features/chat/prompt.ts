// 提示組裝：照 SillyTavern 釘版本 06bde939 的 Chat Completion 預設行為（openai.js
// prepareOpenAIMessages、PromptManager 預設順序、script.js Generate）。順序：
// main（卡片 system_prompt 覆蓋，{{original}}＝預設 main）→ 世界書前（包 5）→ description →
// personality → scenario → 世界書後（包 5）→ 範例對話（每段前一則 [Example Chat]）→
// [Start a new Chat]＋歷史（depth_prompt 依深度插入）→ post_history_instructions。
// 卡欄位先過一輪不含卡欄位的代換，接著第 0 則寫回（Generate 先 getCharacterCardFields 再代換 chat[0]），
// 每段提示再完整代換一次；空的段落不送。
// 上下文長度預算（ST 依 token 預算先捨範例、再捨最舊訊息）在包 4b（D23）。
import type { ChatMessage } from "../openrouter/stream-chat";
import { exampleDialogues, parseMesExamples } from "../sillytavern/mes-examples";
import { baseChatReplace, substituteParams } from "../sillytavern/substitute";
import type { ChatEntry } from "./chat-turn";
import { macroContext, promptText, settleFirstMessage, type ChatSetup } from "./st-text";

export const DEFAULT_MAIN_PROMPT = "Write {{char}}'s next reply in a fictional chat between {{charIfNotGroup}} and {{user}}.";
const NEW_CHAT_PROMPT = "[Start a new Chat]";
const NEW_EXAMPLE_CHAT_PROMPT = "[Example Chat]";
const PERSONALITY_FORMAT = "{{personality}}";
const SCENARIO_FORMAT = "{{scenario}}";

export interface PromptOptions {
  generationType?: "normal" | "regenerate";
  now?: () => Date;
  random?: () => number;
  /** {{model}}：這一發要送的模型 id */
  model?: string;
  /** {{input}}：玩家這一句的原文 */
  input?: string;
  /** {{maxContext}}／{{maxResponse}}：該模型的上下文上限與保留的輸出量 */
  limits?: { maxContext: number; maxResponse: number };
}

type HistoryMessage = ChatMessage & { injected?: boolean };

/** populationInjectionPrompts：深度 0＝最後一則之後，深度 n＝倒數第 n 則之前。 */
function injectDepthPrompt(history: HistoryMessage[], depth: number, role: ChatMessage["role"], content: string): HistoryMessage[] {
  const newestFirst = [...history].reverse();
  if (content.trim()) newestFirst.splice(Math.min(depth, newestFirst.length), 0, { role, content: content.trim(), injected: true });
  return newestFirst.reverse();
}

/** 一次組裝的結果：送出的訊息，與第 0 則寫回後的逐字稿。 */
export interface ComposedPrompt {
  messages: ChatMessage[];
  entries: ChatEntry[];
}

/** 照 ST Generate 組一次提示；巨集副作用（setvar 等）直接落在 `setup.variables`。 */
export function composePrompt(setup: ChatSetup, unsettled: ChatEntry[], options: PromptOptions = {}): ComposedPrompt {
  const card = setup.card;
  const extra = {
    generationType: options.generationType ?? "normal",
    now: options.now,
    random: options.random,
    model: options.model,
    input: options.input,
    limits: options.limits,
  };
  let context = macroContext(setup, unsettled, extra);
  const fill = (text: string, original?: string) => substituteParams(text, context, original === undefined ? {} : { original });

  // 代換的先後照 ST（setvar 之類的副作用順序才對得上）：
  // 1. getCharacterCardFields 依它的物件順序各算一次（第一輪、不含卡欄位）
  const system = baseChatReplace(card.text.system_prompt.trim(), context);
  const mesExamples = baseChatReplace(card.text.mes_example.trim(), context);
  const description = baseChatReplace(card.text.description.trim(), context);
  const personality = baseChatReplace(card.text.personality.trim(), context);
  const scenario = baseChatReplace(card.text.scenario.trim(), context);
  const jailbreak = baseChatReplace(card.text.post_history_instructions.trim(), context);
  const depthPrompt = baseChatReplace(card.text.depth_prompt.trim(), context);
  baseChatReplace(card.text.creator_notes.trim(), context);
  baseChatReplace(card.text.first_mes.trim(), context);
  for (const greeting of card.text.alternate_greetings) baseChatReplace(greeting.trim(), context);
  // 2. 第 0 則代換後寫回，之後的代換看到的是寫回後的逐字稿
  const entries = settleFirstMessage(setup, unsettled, extra);
  context = macroContext(setup, entries, extra);
  // 歷史訊息先過送模前 regex（Generate 的 coreChat）
  const regexed: HistoryMessage[] = entries.map((entry, index) => ({
    role: entry.role === "user" ? "user" : "assistant",
    content: promptText(setup, entries, index, context),
  }));
  // 3. preparePromptsForChatCompletion：scenario／personality 格式（ST 的 {{scenario}}／{{personality}} 會讓卡欄位
  //    依 getCharacterCardFieldsLazy 再算一次，含 setvar 的欄位副作用也跑第二次——照 ST）
  const scenarioText = scenario ? fill(SCENARIO_FORMAT) : "";
  const personalityText = personality ? fill(PERSONALITY_FORMAT) : "";
  // 4. getPromptCollection 先備好預設 main（{{original}} 用的就是它），再備各卡欄位段、覆蓋 main 與 PHI
  const defaultMain = fill(DEFAULT_MAIN_PROMPT);
  const descriptionText = fill(description);
  const personalityPrompt = fill(personalityText);
  const scenarioPrompt = fill(scenarioText);
  const main = system ? fill(system, defaultMain) : defaultMain;
  const postHistory = jailbreak ? fill(jailbreak, "") : "";
  // 5. populateChatCompletion：depth_prompt 插入 → [Start a new Chat] → 歷史由新到舊代換 → 範例
  const history = injectDepthPrompt(regexed, card.depthPrompt.depth, card.depthPrompt.role, fill(depthPrompt));
  const newChat = fill(NEW_CHAT_PROMPT);
  const historyFilled = [...history].reverse().map((message) => ({ role: message.role, content: fill(message.content) })).reverse();
  const examples = exampleDialogues(parseMesExamples(mesExamples), setup.userName, card.text.name).map((dialogue) => ({
    header: fill(NEW_EXAMPLE_CHAT_PROMPT),
    dialogue,
  }));

  const messages: ChatMessage[] = [];
  const push = (message: ChatMessage) => {
    if (message.content) messages.push(message);
  };
  push({ role: "system", content: main });
  push({ role: "system", content: descriptionText });
  push({ role: "system", content: personalityPrompt });
  push({ role: "system", content: scenarioPrompt });
  for (const { header, dialogue } of examples) {
    push({ role: "system", content: header });
    for (const example of dialogue) push({ role: "system", name: example.name, content: example.content });
  }
  push({ role: "system", content: newChat });
  for (const message of historyFilled) push(message);
  push({ role: "system", content: postHistory });
  return { messages, entries };
}
