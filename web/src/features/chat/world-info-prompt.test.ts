// 世界書進提示的位置（ST 06bde939 Chat Completion）：前／後是固定段落、範例上／下併進範例對話、依深度插入與
// 作者註記照 extension prompt 的規則插進歷史、outlet 給 {{outlet}}；掃出的計時狀態跟著組裝結果回來。
import { describe, expect, it } from "vitest";
import { playCardFromValue } from "../cards/play-card";
import { createChatVariables } from "../sillytavern/variables";
import type { ChatEntry } from "./chat-turn";
import { composePrompt } from "./prompt";
import type { ChatSetup } from "./st-text";
import { tableWorldInfo } from "./world-info-setup";
import { EMPTY_CARRY } from "../saves/web-save-codec";

const entry = (keys: string[], content: string, extra: Record<string, unknown> = {}) => ({
  keys,
  content,
  enabled: true,
  insertion_order: 100,
  position: "before_char",
  ...extra,
});

const CARD = playCardFromValue("json", {
  spec: "chara_card_v2",
  spec_version: "2.0",
  data: {
    name: "瑟拉",
    description: "守夜人。{{outlet::door}}",
    mes_example: "<START>\n{{user}}: 冷嗎\n{{char}}: 冷。",
    post_history_instructions: "保持口吻。",
    extensions: { depth_prompt: { prompt: "卡的深度提示", depth: 1, role: "system" } },
    character_book: {
      entries: [
        entry([], "世界觀：雪國。", { constant: true, insertion_order: 1 }),
        entry(["雪"], "雪夜不出門。", { position: "after_char" }),
        entry(["雪"], "深度一：system", { extensions: { position: 4, depth: 1, role: 0 } }),
        entry(["雪"], "深度零：user", { extensions: { position: 4, depth: 0, role: 1 } }),
        entry(["雪"], "註記上", { extensions: { position: 2 } }),
        entry(["雪"], "註記下", { extensions: { position: 3 } }),
        entry(["雪"], "<START>\n{{user}}: 雪大嗎\n{{char}}: 很大。", { extensions: { position: 5 } }),
        entry(["門"], "門外有腳印。", { extensions: { position: 7, outlet_name: "door" } }),
        entry(["{{user}}"], "巨集關鍵字命中：{{user}}"),
      ],
    },
  },
});

const ENTRIES: ChatEntry[] = [
  { id: "o", role: "char", text: "爐火很旺。", opening: true },
  { id: "u1", role: "user", text: "我推開門。" },
  { id: "c1", role: "char", text: "外面在下雪。" },
  { id: "u2", role: "user", text: "旅人想喝湯。" },
];

function setup(): ChatSetup {
  const table = tableWorldInfo(CARD, EMPTY_CARRY.worldInfo);
  return { card: CARD, userName: "旅人", variables: createChatVariables(), chatId: "c", worldInfo: { entries: table.entries, state: table.state } };
}

describe("world info in the composed prompt", () => {
  it("places every position like ST", () => {
    const result = composePrompt(setup(), ENTRIES);
    expect(result.messages).toEqual([
      { role: "system", content: "Write 瑟拉's next reply in a fictional chat between 瑟拉 and 旅人." },
      // 世界書前（order 小的在前）
      { role: "system", content: "世界觀：雪國。\n巨集關鍵字命中：旅人" },
      // 卡欄位在掃世界書之前就代換過（getCharacterCardFields），看到的是上一輪的 outlet（第一輪是空的）
      { role: "system", content: "守夜人。" },
      { role: "system", content: "雪夜不出門。" },
      // 範例上：插在卡的範例之前
      { role: "system", content: "[Example Chat]" },
      { role: "system", name: "example_user", content: "雪大嗎" },
      { role: "system", name: "example_assistant", content: "很大。" },
      { role: "system", content: "[Example Chat]" },
      { role: "system", name: "example_user", content: "冷嗎" },
      { role: "system", name: "example_assistant", content: "冷。" },
      { role: "system", content: "[Start a new Chat]" },
      // 作者註記（世界書上／下，中間空一行）預設深度 4：對話只有 4 則，就在最前面
      { role: "system", content: "註記上\n\n註記下" },
      { role: "assistant", content: "爐火很旺。" },
      { role: "user", content: "我推開門。" },
      // 深度 1：卡的 depth_prompt 與世界書同角色同深度，照鍵名排序併成一則（DEPTH_PROMPT 在 customDepthWI 前）
      { role: "assistant", content: "外面在下雪。" },
      { role: "system", content: "卡的深度提示\n深度一：system" },
      { role: "user", content: "旅人想喝湯。" },
      // 深度 0：最後一則之後
      { role: "user", content: "深度零：user" },
      { role: "system", content: "保持口吻。" },
    ]);
  });

  it("returns the new timed state and where it counted to", () => {
    const sticky = playCardFromValue("json", {
      name: "S",
      character_book: { entries: [entry(["雪"], "黏著", { extensions: { sticky: 2 } })] },
    });
    const table = tableWorldInfo(sticky, EMPTY_CARRY.worldInfo);
    const result = composePrompt({ card: sticky, userName: "U", variables: createChatVariables(), chatId: "c", worldInfo: table }, ENTRIES);
    expect(result.worldInfo).toEqual({
      timed: { sticky: { "wi-0": { start: 4, end: 6, protected: false } }, cooldown: {} },
      lastMessageId: "u2",
      outlets: {},
    });
    // 傳進去的狀態沒被改（真正派送的那一發才落地）
    expect(table.state.timed).toEqual({ sticky: {}, cooldown: {} });
  });

  it("the previous turn's outlet is what card fields see before this turn's scan (ST keeps it until the scan)", () => {
    const first = composePrompt(setup(), ENTRIES);
    expect(first.worldInfo?.outlets).toEqual({ door: "門外有腳印。" });
    // 下一輪：描述在掃描前代換，讀到的是上一輪留下的 outlet；掃完換成這一輪的
    const next = setup();
    next.worldInfo!.state = { ...first.worldInfo!, outlets: { door: "上一輪的腳印" } };
    const second = composePrompt(next, ENTRIES);
    expect(second.messages.map((message) => message.content)).toContain("守夜人。上一輪的腳印");
    expect(second.worldInfo?.outlets).toEqual({ door: "門外有腳印。" });
  });

  it("world info before/after count as fixed prompts for the budget", () => {
    const limits = { maxContext: 0, maxResponse: 0 };
    const count = (message: { content: string }) => (message.content ? 1 : 0);
    // 預留 3＋main、世界書前、描述、世界書後、PHI 各 1＋[Start a new Chat] 1＝9；8 就放不下
    expect(composePrompt(setup(), ENTRIES, { limits: { ...limits, maxContext: 8 }, countTokens: count }).overflow).toBe(true);
    expect(composePrompt(setup(), ENTRIES, { limits: { ...limits, maxContext: 9 }, countTokens: count }).overflow).toBe(false);
  });
});
