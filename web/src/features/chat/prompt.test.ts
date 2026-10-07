import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { importCardBytes, type PlayCard } from "../cards/play-card";
import { SAMPLE_PLAY_CARD } from "../cards/sample-card";
import { createChatVariables } from "../sillytavern/variables";
import type { ChatEntry } from "./chat-turn";
import { composePrompt } from "./prompt";
import { displayText, editedText, openingText, replyText, settleFirstMessage, userText, type ChatSetup } from "./st-text";

const CONTRACT = fileURLToPath(new URL("../../../../src/shared/contracts/card-view/", import.meta.url));

function loadCard(file: string): PlayCard {
  const result = importCardBytes(new Uint8Array(readFileSync(CONTRACT + file)));
  if (!result.ok) throw new Error(result.error);
  return result.card;
}

const setupFor = (card: PlayCard): ChatSetup => ({ card, userName: "旅人", variables: createChatVariables(), chatId: "test-chat" });

describe("ST prompt assembly with the composite contract card", () => {
  const setup = setupFor({ ...loadCard("composite.json"), regexAllowed: true });
  const opening: ChatEntry = { id: "o", role: "char", text: openingText(setup, setup.card.openings[0]), opening: true };
  const user: ChatEntry = { id: "u", role: "user", text: userText(setup, [opening], "*揮手* 晚安") };

  it("keeps the opening's macros raw and stores the player line through the placement-1 script", () => {
    expect(opening.text).toBe("「歡迎光臨，{{user}}。」{{char}} 放下帳本。\n<StatusPlaceHolderImpl/>");
    expect(user.text).toBe("（揮手） 晚安");
  });

  it("the first message is re-substituted on every display and settled into the transcript before a send", () => {
    const timed: ChatEntry = { id: "t", role: "char", text: "現在 {{isotime}}，{{user}}。", opening: true };
    const at = (hours: number) => ({ now: () => new Date(2026, 9, 7, hours, 30) });
    expect(displayText(setup, [timed], 0, at(9))).toBe("現在 09:30，旅人。");
    expect(displayText(setup, [timed], 0, at(21))).toBe("現在 21:30，旅人。");
    const settled = settleFirstMessage(setup, [timed, user], at(9));
    expect(settled[0].text).toBe("現在 09:30，旅人。");
    expect(settled[1]).toBe(user);
    expect(displayText(setup, settled, 0, at(21))).toBe("現在 09:30，旅人。");
  });

  it("builds messages in the ST chat-completion order", () => {
    expect(composePrompt(setup, [opening, user]).messages).toEqual([
      {
        role: "system",
        content: "Write 灰燼旅店的莫拉's next reply in a fictional chat between 灰燼旅店的莫拉 and 旅人. 一律用繁體中文。",
      },
      { role: "system", content: "灰燼旅店的莫拉 是灰燼旅店的老闆娘，認得每一位常客；旅人 是今晚第一位客人。" },
      { role: "system", content: "精明、愛笑，嘴上不饒人。" },
      { role: "system", content: "雨夜，旅人 推開灰燼旅店的門。" },
      { role: "system", content: "[Example Chat]" },
      { role: "system", name: "example_user", content: "還有房間嗎？" },
      { role: "system", name: "example_assistant", content: "「當然有，二樓最裡面那間。」" },
      { role: "system", content: "[Example Chat]" },
      { role: "system", name: "example_user", content: "多少錢？" },
      { role: "system", name: "example_assistant", content: "「看你付得起多少。」" },
      { role: "system", content: "[Start a new Chat]" },
      // depth_prompt 深度 2：插在倒數第 2 則之前；送模前 regex 拿掉了開場白裡的狀態欄標記
      { role: "system", content: "記得 灰燼旅店的莫拉 說話總帶一句「親愛的」。" },
      { role: "assistant", content: "「歡迎光臨，旅人。」灰燼旅店的莫拉 放下帳本。\n" },
      { role: "user", content: "（揮手） 晚安" },
      { role: "system", content: "回覆最後一行固定寫 <StatusPlaceHolderImpl/>。" },
    ]);
  });

  it("depth prompt deeper than the history goes before the oldest message", () => {
    const messages = composePrompt(setupFor({ ...setup.card, depthPrompt: { depth: 9, role: "user" } }), [opening]).messages;
    const start = messages.findIndex((message) => message.content === "[Start a new Chat]");
    expect(messages.slice(start + 1, start + 3).map((message) => message.role)).toEqual(["user", "assistant"]);
  });

  it("display regex renders the interface placeholder without touching the stored text", () => {
    const shown = displayText(setup, [opening, user], 0);
    expect(shown).toContain("<!DOCTYPE html>");
    expect(shown).toContain("旅人 的錢包");
    expect(opening.text).toContain("<StatusPlaceHolderImpl/>");
  });

  it("model replies keep their placeholder in storage (only prompt/display scripts touch it)", () => {
    expect(replyText(setup, [opening, user], "好的。\n<StatusPlaceHolderImpl/>")).toBe("好的。\n<StatusPlaceHolderImpl/>");
  });

  it("edits run only runOnEdit scripts, trim and fill macros", () => {
    expect(editedText(setup, [opening, user], "user", "  *點頭* {{char}}  ")).toBe("（點頭） 灰燼旅店的莫拉");
  });
});

describe("regex scripts need the player's permission (D21, ST asks once on import)", () => {
  const card = loadCard("composite.json");
  const opening = (setup: ChatSetup): ChatEntry => ({ id: "o", role: "char", text: openingText(setup, card.openings[0]), opening: true });

  it("an imported card starts with its scripts refused, like ST before the player confirms", () => {
    expect(card.regexScripts.length).toBeGreaterThan(0);
    expect(card.regexAllowed).toBe(false);
  });

  it("allowed: promptOnly strips the placeholder before sending and markdownOnly renders the interface", () => {
    const setup = setupFor({ ...card, regexAllowed: true });
    const entries = [opening(setup)];
    const history = composePrompt(setup, entries).messages.find((message) => message.role === "assistant");
    expect(history?.content).toBe("「歡迎光臨，旅人。」灰燼旅店的莫拉 放下帳本。\n");
    expect(displayText(setup, entries, 0)).toContain("<!DOCTYPE html>");
    expect(userText(setup, entries, "*揮手*")).toBe("（揮手）");
  });

  it("refused: no script runs at any point, promptOnly and markdownOnly included", () => {
    const setup = setupFor(card);
    const entries = [opening(setup)];
    const history = composePrompt(setup, entries).messages.find((message) => message.role === "assistant");
    expect(history?.content).toContain("<StatusPlaceHolderImpl/>");
    const shown = displayText(setup, entries, 0);
    expect(shown).toContain("<StatusPlaceHolderImpl/>");
    expect(shown).not.toContain("<!DOCTYPE html>");
    expect(userText(setup, entries, "*揮手*")).toBe("*揮手*");
    expect(editedText(setup, entries, "user", "*點頭*")).toBe("*點頭*");
  });
});

describe("ST macro evaluation order inside the prompt", () => {
  const base = setupFor(SAMPLE_PLAY_CARD);

  it("history macros run newest to oldest (populateChatHistory): an older getvar sees a newer setvar", () => {
    const older: ChatEntry = { id: "a", role: "char", text: "值={{getvar::x}}" };
    const newer: ChatEntry = { id: "b", role: "char", text: "{{setvar::x::NEW}}reply" };
    // 第 0 則在歷史之前就寫回了（見下一個 describe），這裡墊一則開場讓兩則都走歷史代換
    const opening: ChatEntry = { id: "o", role: "char", text: "開場", opening: true };
    const messages = composePrompt(base, [opening, older, newer]).messages;
    expect(messages.slice(-2)).toEqual([
      { role: "assistant", content: "值=NEW" },
      { role: "assistant", content: "reply" },
    ]);
  });

  it("a personality with setvar runs twice like ST (each {{personality}} re-resolves the lazy card field)", () => {
    const card = { ...SAMPLE_PLAY_CARD, text: { ...SAMPLE_PLAY_CARD.text, personality: "{{addvar::n::1}}冷淡" } };
    const setup = setupFor(card);
    const messages = composePrompt(setup, []).messages;
    expect(messages.some((message) => message.content === "冷淡")).toBe(true);
    expect(setup.variables.local.get("n")).toBe(2);
  });

  it("{{model}}, {{input}} and the limits come from the real call", () => {
    const card = {
      ...SAMPLE_PLAY_CARD,
      text: { ...SAMPLE_PLAY_CARD.text, scenario: "{{model}}|{{input}}|{{maxContext}}|{{maxResponse}}|{{maxPrompt}}" },
    };
    const messages = composePrompt(setupFor(card), [], {
      model: "alpha/one:free",
      input: "你好",
      limits: { maxContext: 65536, maxResponse: 4096 },
    }).messages;
    expect(messages.some((message) => message.content === "alpha/one:free|你好|65536|4096|61440")).toBe(true);
  });
});

describe("ST prompt assembly with the built-in sample card", () => {
  it("fills names and skips empty sections", () => {
    const setup = setupFor(SAMPLE_PLAY_CARD);
    const opening: ChatEntry = { id: "o", role: "char", text: "開場", opening: true };
    const user: ChatEntry = { id: "u", role: "user", text: "你好" };
    const messages = composePrompt(setup, [opening, user]).messages;
    expect(messages[0]).toEqual({ role: "system", content: "Write 瑟拉's next reply in a fictional chat between 瑟拉 and 旅人." });
    expect(messages.some((message) => message.content.includes("{{"))).toBe(false);
    expect(messages.slice(-3)).toEqual([
      { role: "system", content: "[Start a new Chat]" },
      { role: "assistant", content: "開場" },
      { role: "user", content: "你好" },
    ]);
  });
});
