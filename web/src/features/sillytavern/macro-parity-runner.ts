// 桌面版巨集引擎對拍（worldbook-st-trigger-parity 包 3）：案例由網頁版實作跑出預期值（web/scripts/gen-st-macro-fixtures.mjs），
// 網頁版與 Rust 的 parity 測試各自重跑、只比對。案例檔在 src/shared/contracts/st-macros/，欄位說明見該目錄的 st-macros.md。
// 時間一律用固定時鐘，時區固定 Asia/Taipei（+08:00，沒有日光節約）：產生腳本與測試都先設好 process.env.TZ。
import type { ChatLine } from "./macro-engine";
import { substituteParams, type CardText, type MacroContext } from "./substitute";
import { createChatVariables, type VariableMap } from "./variables";

export const FIXTURE_TIMEZONE = "Asia/Taipei";
export const FIXTURE_UTC_OFFSET_MINUTES = 480;
/** 2026-10-07 23:05:09.123 +08:00（星期三） */
export const FIXTURE_NOW_MS = Date.UTC(2026, 9, 7, 15, 5, 9, 123);

export interface MacroCase {
  name: string;
  input: string;
  context?: {
    card?: Partial<CardText>;
    userName?: string;
    /** 舊到新 */
    chat?: ChatLine[];
    variables?: { local?: VariableMap; global?: VariableMap };
    chatId?: string;
    nowMs?: number;
    input?: string;
    generationType?: string;
    model?: string;
    limits?: { maxContext: number; maxResponse: number };
    outlets?: Record<string, string>;
    /** 依序取用；用完之後一律 0 */
    random?: number[];
  };
  options?: { original?: string; replaceCharacterCard?: boolean };
}

export interface MacroExpected {
  output: string;
  /** 代換後的變數表，`JSON.stringify` 原文（比鍵順序與數字寫法） */
  local: string;
  global: string;
  randomUsed: number;
}

export const DEFAULT_CARD: CardText = {
  name: "莫拉",
  description: "{{char}} 是老闆娘，{{user}} 是客人。{{personality}}",
  personality: "愛笑",
  scenario: "雨夜",
  first_mes: "歡迎，{{user}}。",
  mes_example: "<START>\n{{user}}: 嗨\n{{char}}: 你好",
  creator_notes: "",
  system_prompt: "",
  post_history_instructions: "記得押韻",
  alternate_greetings: ["第二個開場"],
  character_version: "2.1",
  depth_prompt: "",
};

export const DEFAULT_CHAT: ChatLine[] = [
  { isUser: false, text: "歡迎" },
  { isUser: true, text: "我要一間房" },
  { isUser: false, text: "二樓" },
];

export function runMacroCase(item: MacroCase): MacroExpected {
  const ctx = item.context ?? {};
  const variables = createChatVariables(structuredClone(ctx.variables?.local ?? {}), structuredClone(ctx.variables?.global ?? {}));
  const sequence = ctx.random ?? [];
  let used = 0;
  const context: MacroContext = {
    card: { ...DEFAULT_CARD, ...ctx.card },
    userName: ctx.userName ?? "旅人",
    chat: ctx.chat ?? DEFAULT_CHAT,
    variables,
    chatId: ctx.chatId ?? "chat-1",
    input: ctx.input,
    generationType: ctx.generationType,
    model: ctx.model,
    limits: ctx.limits,
    now: () => new Date(ctx.nowMs ?? FIXTURE_NOW_MS),
    random: () => {
      const value = used < sequence.length ? sequence[used] : 0;
      used += 1;
      return value;
    },
    outlets: ctx.outlets,
  };
  const output = substituteParams(item.input, context, {
    original: item.options?.original,
    replaceCharacterCard: item.options?.replaceCharacterCard,
  });
  return {
    output,
    local: JSON.stringify(variables.local.values),
    global: JSON.stringify(variables.global.values),
    randomUsed: used,
  };
}
