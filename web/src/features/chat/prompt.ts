// 提示組裝：照 SillyTavern 釘版本 06bde939 的 Chat Completion 預設行為（openai.js
// prepareOpenAIMessages、PromptManager 預設順序、script.js Generate）。順序：
// main（卡片 system_prompt 覆蓋，{{original}}＝預設 main）→ 世界書前 → description →
// personality → scenario → 世界書後 → 範例對話（每段前一則 [Example Chat]）→
// [Start a new Chat]＋歷史（作者註記、depth_prompt、世界書依深度插入）→ post_history_instructions。
// 卡欄位先過一輪不含卡欄位的代換，接著第 0 則寫回（Generate 先 getCharacterCardFields 再代換 chat[0]），
// 每段提示再完整代換一次；空的段落不送。
// 世界書：第 0 則寫回後、組提示前照 checkWorldInfo 掃一次（sillytavern/world-info-scan.ts）；世界書前／後
// 是固定段落，範例上／下併進範例對話，依深度插入與作者註記上／下照 injections.ts 插進歷史，outlet 給 {{outlet}}。
// 上下文預算照 populateChatCompletion（openai.js:1185）與 ChatCompletion（:3917）：預算＝上下文上限－保留輸出，
// 先預留 3，固定段落（main、卡欄位、post_history_instructions）先佔、佔不下就整句不送（ST 的「Mandatory prompts
// exceed the context size」）；[Start a new Chat] 先預約，歷史由新到舊邊代換邊放、放不下就停（更舊的不代換）；
// 剩下的才一段段放範例對話。所以先掉範例、再掉最舊訊息（D23〔作者裁決 2026-10-07〕）。世界書前／後算固定段落。
import { activeRegexScripts, loadsMvu } from "../cards/play-card";
import type { ChatMessage } from "../openrouter/stream-chat";
import type { CountedMessage } from "../sillytavern/tokens";
import { exampleDialogues, parseMesExamples } from "../sillytavern/mes-examples";
import { getRegexedString, REGEX_PLACEMENT } from "../sillytavern/regex-scripts";
import { baseChatReplace, substituteParams } from "../sillytavern/substitute";
import { checkWorldInfo, MVU_WI_SETTINGS, ST_WI_SETTINGS, type WiResult } from "../sillytavern/world-info-scan";
import type { ChatEntry } from "./chat-turn";
import { authorsNotePrompt, injectExtensionPrompts, roleName, type ExtensionPrompt, type HistoryMessage } from "./injections";
import { macroContext, promptText, settleFirstMessage, type ChatSetup, type WorldInfoState } from "./st-text";

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
  /** {{maxContext}}／{{maxResponse}}：該模型的上下文上限與保留的輸出量；與 `countTokens` 都有才裁切 */
  limits?: { maxContext: number; maxResponse: number };
  /** 這支模型一則訊息估多少 token（sillytavern/tokens.ts） */
  countTokens?: (message: CountedMessage) => number;
  /** 一段文字估多少 token（世界書預算） */
  countText?: (text: string) => number;
}

/**
 * 一次組裝的結果：送出的訊息、第 0 則寫回後的逐字稿、掃完世界書後的計時狀態（這桌沒有世界書是 null）。
 * `overflow`＝固定段落就超過預算，不送（messages 為空）。
 */
export interface ComposedPrompt {
  messages: ChatMessage[];
  entries: ChatEntry[];
  overflow: boolean;
  worldInfo: WorldInfoState | null;
}

/** ChatCompletion 的 token 預算；沒給上限或計數器就不裁切。 */
function tokenBudget(options: PromptOptions) {
  const { limits, countTokens } = options;
  let remaining = limits && countTokens ? limits.maxContext - limits.maxResponse : Number.POSITIVE_INFINITY;
  const cost = (message: ChatMessage) => countTokens?.(message) ?? 0;
  return {
    cost,
    reserve: (tokens: number) => void (remaining -= tokens),
    free: (tokens: number) => void (remaining += tokens),
    affords: (tokens: number) => remaining - tokens >= 0,
  };
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
    // 掃世界書之前看到的是上一輪的 outlet（ST 掃完才換）
    outlets: setup.worldInfo?.state.outlets,
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
  const creatorNotes = baseChatReplace(card.text.creator_notes.trim(), context);
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
  // 2.5 世界書：掃「名字: 內容」新到舊；預算是這次的提示預算（上下文上限－保留輸出）
  let wi: WiResult | null = null;
  if (setup.worldInfo && setup.worldInfo.entries.length > 0) {
    const limits = options.limits;
    const settings = loadsMvu(card) ? MVU_WI_SETTINGS : ST_WI_SETTINGS;
    wi = checkWorldInfo(structuredClone(setup.worldInfo.entries), {
      settings,
      chat: entries
        .map((entry, index) => (settings.includeNames ? `${entry.role === "user" ? setup.userName : card.text.name}: ` : "") + regexed[index].content)
        .reverse(),
      maxContext: limits ? limits.maxContext - limits.maxResponse : Number.POSITIVE_INFINITY,
      globalScan: {
        personaDescription: "",
        characterDescription: description,
        characterPersonality: personality,
        characterDepthPrompt: depthPrompt,
        scenario,
        creatorNotes,
      },
      trigger: extra.generationType,
      timed: setup.worldInfo.state.timed,
      substitute: (text) => fill(text),
      regex: (text, depth) =>
        getRegexedString(text, REGEX_PLACEMENT.WORLD_INFO, activeRegexScripts(card), context, { isPrompt: true, ...(depth === null ? {} : { depth }) }),
      countTokens: options.countText ?? (() => 0),
      random: options.random ?? Math.random,
    });
    // 掃完：舊 outlet 全部換掉（flushWIInjections 後再設這次的）
    const outlets = Object.fromEntries(Object.entries(wi.outlets).map(([name, values]) => [name, values.join("\n")]));
    context = { ...context, outlets };
  }
  const worldInfo: WorldInfoState | null =
    wi && setup.worldInfo ? { timed: wi.timed, lastMessageId: entries[entries.length - 1]?.id ?? null, outlets: context.outlets ?? {} } : (setup.worldInfo?.state ?? null);
  // 範例上／下：照 Generate 先過 baseChatReplace 再切段，上＝插在最前（後到的更前）、下＝接在最後
  const exampleBlocks = parseMesExamples(mesExamples);
  for (const example of wi?.examples ?? []) {
    if (example.content.length === 0) continue;
    const cleaned = parseMesExamples(baseChatReplace(example.content, context));
    if (example.position === "before") exampleBlocks.unshift(...cleaned);
    else exampleBlocks.push(...cleaned);
  }
  // 依深度插進歷史的提示：作者註記（世界書上／下）、卡的 depth_prompt、世界書依深度插入
  const authorsNote = authorsNotePrompt(wi?.anTop ?? [], wi?.anBottom ?? []);
  const injections: ExtensionPrompt[] = [
    authorsNote,
    { key: "DEPTH_PROMPT", value: depthPrompt, depth: card.depthPrompt.depth, role: card.depthPrompt.role },
    ...(wi?.depth ?? []).map((group) => ({
      key: `customDepthWI_${group.depth}_${group.role}`,
      value: group.entries.join("\n"),
      depth: group.depth,
      role: roleName(group.role),
    })),
  ];
  // 3. preparePromptsForChatCompletion：scenario／personality 格式（ST 的 {{scenario}}／{{personality}} 會讓卡欄位
  //    依 getCharacterCardFieldsLazy 再算一次，含 setvar 的欄位副作用也跑第二次——照 ST）
  const scenarioText = scenario ? fill(SCENARIO_FORMAT) : "";
  const personalityText = personality ? fill(PERSONALITY_FORMAT) : "";
  // 4. getPromptCollection 先備好預設 main（{{original}} 用的就是它），再備各卡欄位段、覆蓋 main 與 PHI
  const defaultMain = fill(DEFAULT_MAIN_PROMPT);
  const wiBefore = fill(wi?.before ?? "");
  const wiAfter = fill(wi?.after ?? "");
  const descriptionText = fill(description);
  const personalityPrompt = fill(personalityText);
  const scenarioPrompt = fill(scenarioText);
  // 作者註記有內容時也照 ST 過一次 preparePrompt（只有副作用；實際內容在插入歷史時再代換）
  if (authorsNote.value) fill(authorsNote.value);
  const main = system ? fill(system, defaultMain) : defaultMain;
  const postHistory = jailbreak ? fill(jailbreak, "") : "";
  // 5. populateChatCompletion：預留 3（每則回覆前的 <|start|>assistant<|message|>），固定段落逐段佔預算
  const budget = tokenBudget(options);
  budget.reserve(3);
  const fixed = [main, wiBefore, descriptionText, personalityPrompt, scenarioPrompt, wiAfter, postHistory].map(
    (content): ChatMessage => ({ role: "system", content }),
  );
  const overflow = { messages: [], entries, overflow: true, worldInfo };
  for (const message of fixed) {
    const tokens = budget.cost(message);
    if (!budget.affords(tokens)) return overflow;
    budget.reserve(tokens);
  }
  // 6. populateChatHistory：依深度插入 → 預約 [Start a new Chat] → 歷史由新到舊代換、放得下才放
  const history = injectExtensionPrompts(regexed, injections, fill);
  const newChat: ChatMessage = { role: "system", content: fill(NEW_CHAT_PROMPT) };
  const newChatTokens = budget.cost(newChat);
  budget.reserve(newChatTokens);
  const kept: ChatMessage[] = [];
  for (const message of [...history].reverse()) {
    const filled: ChatMessage = { role: message.role, content: fill(message.content) };
    const tokens = budget.cost(filled);
    if (!budget.affords(tokens)) break;
    budget.reserve(tokens);
    kept.unshift(filled);
  }
  // [Start a new Chat] 放回預算再正式放進去：連它都放不下也算固定段落超過
  budget.free(newChatTokens);
  if (!budget.affords(newChatTokens)) return overflow;
  budget.reserve(newChatTokens);
  // 7. populateDialogueExamples：每段連同 [Example Chat] 整段放得下才放，放不下就停
  const dialogues = exampleDialogues(exampleBlocks, setup.userName, card.text.name);
  const examples: ChatMessage[] = [];
  if (dialogues.length > 0) {
    const header: ChatMessage = { role: "system", content: fill(NEW_EXAMPLE_CHAT_PROMPT) };
    for (const dialogue of dialogues) {
      const block = [header, ...dialogue.map((example): ChatMessage => ({ role: "system", name: example.name, content: example.content }))];
      if (!budget.affords(block.reduce((sum, message) => sum + budget.cost(message), 0))) break;
      // 空內容不放進去也不扣（ChatCompletion.insert）；有 name 的空訊息仍算進上面那次 canAffordAll
      for (const message of block.filter((message) => message.content)) budget.reserve(budget.cost(message));
      examples.push(...block);
    }
  }

  const [mainMessage, beforeMessage, descriptionMessage, personalityMessage, scenarioMessage, afterMessage, postHistoryMessage] = fixed;
  const ordered = [
    mainMessage,
    beforeMessage,
    descriptionMessage,
    personalityMessage,
    scenarioMessage,
    afterMessage,
    ...examples,
    newChat,
    ...kept,
    postHistoryMessage,
  ];
  return { messages: ordered.filter((message) => message.content), entries, overflow: false, worldInfo };
}
