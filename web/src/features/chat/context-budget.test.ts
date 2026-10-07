// 上下文預算（D23）：同一組逐字稿在不同上限下送出哪些訊息，照 ST populateChatCompletion 的捨棄順序——
// 固定段落先佔、歷史由新到舊、剩下才放範例；固定段落放不下就整句不送。
import { describe, expect, it } from "vitest";
import { playCardFromValue } from "../cards/play-card";
import type { ChatMessage } from "../openrouter/stream-chat";
import { messageTokenCounter, tokenizerModel } from "../sillytavern/tokens";
import { createChatVariables } from "../sillytavern/variables";
import type { ChatEntry } from "./chat-turn";
import { composePrompt } from "./prompt";

const CARD = playCardFromValue("json", {
  name: "守夜人",
  description: "守夜人在城牆上看守了二十年，認得每一盞燈的主人，說話慢而篤定。",
  mes_example: "<START>\n{{user}}: 今晚冷嗎？\n{{char}}: 冷，但燈還亮著。\n<START>\n{{user}}: 你在等誰？\n{{char}}: 等天亮。",
  post_history_instructions: "保持守夜人的口吻。",
});
const LINES = [
  "城門已經關了，你從哪裡來？",
  "我從南邊的村子來，想找個地方過夜。",
  "城牆下的小屋還空著，跟我來。",
  "謝謝你，這裡的燈一直都這麼亮嗎？",
  "二十年來沒有一晚熄過。",
  "那你一定很累了吧。",
];
const ENTRIES: ChatEntry[] = LINES.map((text, index) => ({ id: `h${index}`, role: index % 2 === 0 ? "char" : "user", text }));
const MAX_RESPONSE = 100;

/** 送出的訊息縮寫：main、desc、ex（[Example Chat]）、ex1/ex2（第幾段範例的對話）、new、h0–h5、phi */
function shape(messages: ChatMessage[]): string[] {
  let dialogue = 0;
  return messages.map((message) => {
    if (message.content.startsWith("Write ")) return "main";
    if (message.content === CARD.text.description) return "desc";
    if (message.content === "[Example Chat]") return (dialogue += 1), "ex";
    if (message.name) return `ex${dialogue}`;
    if (message.content === "[Start a new Chat]") return "new";
    if (message.content === CARD.text.post_history_instructions) return "phi";
    return `h${LINES.indexOf(message.content)}`;
  });
}

function send(model: string, tokenizer: string | null, budget: number) {
  const setup = { card: CARD, userName: "旅人", variables: createChatVariables(), chatId: "c" };
  const result = composePrompt(setup, ENTRIES, {
    limits: { maxContext: budget + MAX_RESPONSE, maxResponse: MAX_RESPONSE },
    countTokens: messageTokenCounter(model, tokenizer),
  });
  return result.overflow ? "overflow" : shape(result.messages).join(" ");
}

const ALL_HISTORY = "h0 h1 h2 h3 h4 h5";
const FIXED = (middle: string) => `main desc ${middle} phi`.replace(/ {2,}/g, " ");

describe("context budget table (same transcript, different limits)", () => {
  // tiktoken 結構（cl100k 一族；每則另加 role 與固定開銷，估得比 Qwen 這類多）：固定 79＋預留 3、[Start a new Chat] 12、
  // 歷史新到舊 15/17/21/20/22/19、範例兩段 48、44
  const gptOss = ["openai/gpt-oss-20b:free", "GPT"] as const;
  it.each([
    [300, FIXED(`ex ex1 ex1 ex ex2 ex2 new ${ALL_HISTORY}`)],
    [299, FIXED(`ex ex1 ex1 new ${ALL_HISTORY}`)],
    // 第一段範例放不下就停，後面那段放得下也不放
    [255, FIXED(`new ${ALL_HISTORY}`)],
    [208, FIXED(`new ${ALL_HISTORY}`)],
    [207, FIXED("new h1 h2 h3 h4 h5")],
    [94, FIXED("new")],
    // [Start a new Chat] 放不下也算固定段落超過
    [93, "overflow"],
    [81, "overflow"],
  ])("gpt-oss (tiktoken, gpt-3.5-turbo) budget %i", (budget, expected) => {
    expect(send(...gptOss, budget)).toBe(expected);
  });

  // HF tokenizer 結構（所有欄位以空一行接起來整段算）：固定 62＋預留 3、[Start a new Chat] 6、歷史 8/12/15/14/16/13、
  // 範例 29、25
  const qwen = ["qwen/qwen3-235b-a22b:free", "Qwen"] as const;
  const deepseek = ["deepseek/deepseek-chat-v3-0324:free", null] as const;
  it.each([
    [207, FIXED(`ex ex1 ex1 ex ex2 ex2 new ${ALL_HISTORY}`)],
    [202, FIXED(`ex ex1 ex1 new ${ALL_HISTORY}`)],
    [149, FIXED(`new ${ALL_HISTORY}`)],
    [148, FIXED("new h1 h2 h3 h4 h5")],
    [106, FIXED("new h3 h4 h5")],
    [105, FIXED("new h4 h5")],
    [71, FIXED("new")],
    [70, "overflow"],
  ])("qwen and deepseek (web tokenizer) budget %i", (budget, expected) => {
    expect(send(...qwen, budget)).toBe(expected);
    expect(send(...deepseek, budget)).toBe(expected);
  });

  it("no limits or counter: nothing is trimmed", () => {
    const setup = { card: CARD, userName: "旅人", variables: createChatVariables(), chatId: "c" };
    expect(shape(composePrompt(setup, ENTRIES).messages).join(" ")).toBe(FIXED(`ex ex1 ex1 ex ex2 ex2 new ${ALL_HISTORY}`));
  });

  it("messages older than the cut are never substituted, like ST's lazy history loop", () => {
    const card = playCardFromValue("json", { name: "C" });
    // 第 0 則照 ST 一定先寫回（會代換），所以放一則沒有巨集的開場
    const entries: ChatEntry[] = [
      { id: "o", role: "char", text: "開場" },
      { id: "a", role: "user", text: "舊{{setvar::older::1}}" },
      { id: "b", role: "char", text: "中{{setvar::cut::1}}" },
      { id: "c", role: "user", text: "新" },
    ];
    // 每則非空訊息算 1：預留 3＋main 1＋[Start a new Chat] 1，再放得下 1 則
    const run = (budget: number) => {
      const setup = { card, userName: "U", variables: createChatVariables(), chatId: "c" };
      const result = composePrompt(setup, entries, {
        limits: { maxContext: budget, maxResponse: 0 },
        countTokens: (message) => (message.content ? 1 : 0),
      });
      return { sent: result.messages.map((message) => message.content), local: setup.variables.local.values };
    };
    const tight = run(6);
    expect(tight.sent).toEqual(["Write C's next reply in a fictional chat between C and U.", "[Start a new Chat]", "新"]);
    // 放不下的那一則已經代換（ST 先組 Message 再判斷），更舊的沒碰
    expect(tight.local).toEqual({ cut: "1" });
    expect(run(7).local).toEqual({ cut: "1", older: "1" });
  });
});

describe("ST token counting structure (text tokens by guesstimate: UTF-8 bytes / 3.35)", () => {
  it("picks the tokenizer like ST's OpenRouter branch", () => {
    expect(tokenizerModel("meta-llama/llama-3.3-70b-instruct:free", "Llama3")).toBe("llama3");
    expect(tokenizerModel("qwen/qwen3-coder:free", "Qwen")).toBe("qwen2");
    expect(tokenizerModel("google/gemma-3-27b-it:free", "Gemini")).toBe("gemma");
    expect(tokenizerModel("cohere/command-a", "Cohere")).toBe("command-a");
    expect(tokenizerModel("deepseek/deepseek-r1:free", "DeepSeek")).toBe("deepseek");
    expect(tokenizerModel("z-ai/glm-4.5-air:free", "Other")).toBe("gpt-3.5-turbo");
    expect(tokenizerModel("anthropic/claude-sonnet-4", "Claude")).toBe("claude");
  });

  it("counts one message per ST path", () => {
    const plain = { role: "system", content: "abcdefg" };
    const named = { role: "system", content: "hi", name: "example_user" };
    // tiktoken：3＋role 2＋content 3＋padding 3－2
    expect(messageTokenCounter("openai/gpt-oss-20b:free", "GPT")(plain)).toBe(9);
    // 有 name：再加 name 4＋1
    expect(messageTokenCounter("openai/gpt-oss-20b:free", "GPT")(named)).toBe(12);
    // web tokenizer：「system\n\nabcdefg」15 位元組→5，－2
    expect(messageTokenCounter("qwen/qwen3-coder:free", "Qwen")(plain)).toBe(3);
    expect(messageTokenCounter("qwen/qwen3-coder:free", "Qwen")(named)).toBe(6);
    // sentencepiece 同樣整段算
    expect(messageTokenCounter("mistralai/mistral-nemo:free", "Mistral")(plain)).toBe(3);
    // claude 前端不扣 2
    expect(messageTokenCounter("anthropic/claude-sonnet-4", "Claude")(plain)).toBe(5);
    // 中文一字 3 位元組
    expect(messageTokenCounter("qwen/qwen3-coder:free", "Qwen")({ role: "user", content: "你好" })).toBe(2);
    // 空內容不算（有 name 才算）
    expect(messageTokenCounter("openai/gpt-oss-20b:free", "GPT")({ role: "system", content: "" })).toBe(0);
  });
});
