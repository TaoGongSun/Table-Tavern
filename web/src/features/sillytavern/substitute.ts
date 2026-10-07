// substituteParams（釘版本 script.js:2981、MacroEnvBuilder.js）：一次巨集代換的環境由這裡組。
// 卡欄位照 getCharacterCardFieldsLazy：用到才算、先過一輪不含卡欄位的代換（baseChatReplace）。
import type { CharacterEnv, ChatLine, MacroEnv } from "./macro-engine";
import { ENGINE, getStringHash } from "./macro-library";
import type { ChatVariables } from "./variables";

/** 提示組裝用得到的卡欄位原文（卡片沒有的欄位是空字串）。 */
export interface CardText {
  name: string;
  description: string;
  personality: string;
  scenario: string;
  first_mes: string;
  mes_example: string;
  creator_notes: string;
  system_prompt: string;
  post_history_instructions: string;
  alternate_greetings: string[];
  character_version: string;
  depth_prompt: string;
}

/** 一段對話裡固定的代換條件。 */
export interface MacroContext {
  card: CardText;
  userName: string;
  chat: ChatLine[];
  variables: ChatVariables;
  chatId: string;
  input?: string;
  generationType?: string;
  model?: string;
  limits?: { maxContext: number; maxResponse: number };
  now?: () => Date;
  random?: () => number;
  /** 這次世界書掃出的 outlet（{{outlet::名稱}}） */
  outlets?: Record<string, string>;
}

export interface SubstituteOptions {
  original?: string;
  /** false＝卡欄位巨集（{{description}} 等）不代入，ST baseChatReplace 用 */
  replaceCharacterCard?: boolean;
  postProcess?: (value: string) => string;
  dynamicMacros?: Record<string, string | (() => string)>;
}

const MOBILE = /Android|iPhone|iPad|iPod|Mobile/i;

/** ST baseChatReplace：非空字串才代換（不含卡欄位），並拿掉 \r。 */
export function baseChatReplace(value: string, context: MacroContext): string {
  if (!value) return value;
  return substituteParams(value, context, { replaceCharacterCard: false }).replace(/\r/g, "");
}

function characterFields(context: MacroContext): Partial<CharacterEnv> {
  const card = context.card;
  const resolvers: { [K in keyof CharacterEnv]: () => CharacterEnv[K] } = {
    charPrompt: () => baseChatReplace(card.system_prompt.trim(), context),
    charInstruction: () => baseChatReplace(card.post_history_instructions.trim(), context),
    description: () => baseChatReplace(card.description.trim(), context),
    personality: () => baseChatReplace(card.personality.trim(), context),
    scenario: () => baseChatReplace(card.scenario.trim(), context),
    persona: () => "",
    mesExamplesRaw: () => baseChatReplace(card.mes_example.trim(), context),
    charDepthPrompt: () => baseChatReplace(card.depth_prompt.trim(), context),
    creatorNotes: () => baseChatReplace(card.creator_notes.trim(), context),
    firstMessage: () => baseChatReplace(card.first_mes.trim(), context),
    alternateGreetings: () => card.alternate_greetings.map((greeting) => baseChatReplace(greeting.trim(), context)),
    version: () => card.character_version,
  };
  const fields: Partial<CharacterEnv> = {};
  for (const [key, resolve] of Object.entries(resolvers)) {
    let cached: unknown;
    let done = false;
    Object.defineProperty(fields, key, {
      enumerable: true,
      get() {
        if (!done) {
          cached = resolve();
          done = true;
        }
        return cached;
      },
    });
  }
  return fields;
}

export function substituteParams(content: string, context: MacroContext, options: SubstituteOptions = {}): string {
  if (!content) return "";
  let originalUsed = false;
  const env: MacroEnv = {
    contentHash: getStringHash(content),
    names: {
      user: context.userName,
      char: context.card.name,
      group: context.card.name,
      groupNotMuted: context.card.name,
      notChar: context.userName,
    },
    character: options.replaceCharacterCard === false ? {} : characterFields(context),
    model: context.model ?? "",
    original:
      typeof options.original === "string"
        ? () => {
            if (originalUsed) return "";
            originalUsed = true;
            return options.original!;
          }
        : null,
    postProcess: options.postProcess ?? ((value) => value),
    dynamicMacros: Object.fromEntries(Object.entries(options.dynamicMacros ?? {}).map(([key, value]) => [key.toLowerCase(), value])),
    chat: context.chat,
    variables: context.variables,
    input: context.input ?? "",
    generationType: context.generationType ?? "normal",
    limits: context.limits ?? { maxContext: 0, maxResponse: 0 },
    now: context.now ?? (() => new Date()),
    chatId: context.chatId,
    random: context.random ?? Math.random,
    outlets: context.outlets ?? {},
    isMobile: typeof navigator !== "undefined" && MOBILE.test(navigator.userAgent),
  };
  return ENGINE.evaluate(content, env);
}
