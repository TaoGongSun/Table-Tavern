import { describe, expect, it } from "vitest";
import stMacros from "@desktop/shared/contracts/st-macros.json";
import { LIBRARY } from "./macro-library";
import { formatUtcOffset } from "./macro-time";
import { seedrandom } from "./seedrandom";
import stCasesJson from "./st-macro-cases.json";
import { substituteParams, type CardText, type MacroContext } from "./substitute";
import { createChatVariables } from "./variables";

const card: CardText = {
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

function context(overrides: Partial<MacroContext> = {}): MacroContext {
  return {
    card,
    userName: "旅人",
    chat: [
      { isUser: false, text: "歡迎" },
      { isUser: true, text: "我要一間房" },
      { isUser: false, text: "二樓" },
    ],
    variables: createChatVariables(),
    chatId: "chat-1",
    now: () => new Date(Date.UTC(2026, 9, 7, 15, 5, 9)),
    random: () => 0,
    ...overrides,
  };
}

const stCases = stCasesJson as {
  cases: { name: string; input: string; expected: string; variables: { local?: Record<string, string>; global?: Record<string, string> } }[];
};

const run = (text: string, ctx = context(), options = {}) => substituteParams(text, ctx, options);

describe("macro names and card fields", () => {
  it("names, legacy markers and card fields (card fields get one macro pass first)", () => {
    expect(run("{{user}}／{{char}}／<USER>／<BOT>／{{charIfNotGroup}}")).toBe("旅人／莫拉／旅人／莫拉／莫拉");
    // {{personality}} 在欄位第一輪（不代卡欄位）就被換成空字串，跟 ST baseChatReplace 一樣
    expect(run("{{description}}")).toBe("莫拉 是老闆娘，旅人 是客人。");
    expect(run("{{charJailbreak}}|{{charInstruction}}|{{version}}|{{greeting::1}}")).toBe("{{charJailbreak}}|記得押韻|2.1|第二個開場");
    expect(run("{{mesExamples}}")).toBe("<START>\n旅人: 嗨\n莫拉: 你好\n");
    expect(run("{{description}}", context(), { replaceCharacterCard: false })).toBe("");
  });

  it("is case-insensitive and keeps unknown macros with nested parts resolved", () => {
    expect(run("{{USER}} {{Char}}")).toBe("旅人 莫拉");
    expect(run("{{unknownMacro::{{user}}}}")).toBe("{{unknownMacro::旅人}}");
    expect(run("{{ user }}")).toBe("旅人");
  });

  it("wrong arity or type keeps the raw macro", () => {
    expect(run("{{user::extra}}")).toBe("{{user::extra}}");
    expect(run("{{newline::abc}}")).toBe("{{newline::abc}}");
    expect(run("a{{newline::2}}b{{space::3}}c")).toBe("a\n\nb   c");
  });

  it("original is one-shot and stays raw without an original", () => {
    expect(run("{{original}} 加上 {{original}}", context(), { original: "預設" })).toBe("預設 加上 ");
    expect(run("{{original}}")).toBe("{{original}}");
  });
});

describe("engine syntax", () => {
  it("comments, escapes, trim and noop", () => {
    expect(run("a{{// 註解 {{user}} }}b")).toBe("ab");
    expect(run("\\{\\{user\\}\\}")).toBe("{{user}}");
    expect(run("上一行\n\n{{trim}}\n下一行")).toBe("上一行下一行");
    expect(run("x{{noop}}y")).toBe("xy");
    expect(run("{{{user}}}")).toBe("{旅人}");
    expect(run("{{user")).toBe("{{user");
  });

  it("scoped if/else with nesting and inversion", () => {
    expect(run("{{if user}}有名字{{else}}沒有{{/if}}")).toBe("有名字");
    expect(run("{{if !user}}有名字{{else}}沒有{{/if}}")).toBe("沒有");
    expect(run("{{if 0}}A{{/if}}|{{if off}}B{{else}}C{{/if}}")).toBe("|C");
    expect(run("{{if 1}}外{{if 0}}內{{else}}內否{{/if}}{{/if}}")).toBe("外內否");
    expect(run("{{if::1::行內}}")).toBe("行內");
    expect(run("{{if 1}}\n    縮排\n    兩行\n{{/if}}")).toBe("縮排\n兩行");
  });

  it("scoped trim and unmatched closings", () => {
    expect(run("[{{trim}}  內容  {{/trim}}]")).toBe("[內容]");
    expect(run("{{/if}}多餘")).toBe("{{/if}}多餘");
  });
});

describe("variables", () => {
  it("setvar/getvar/addvar/incvar/decvar with ST number handling", () => {
    const ctx = context();
    expect(run("{{setvar::金::10}}{{getvar::金}}", ctx)).toBe("10");
    expect(run("{{addvar::金::5}}{{getvar::金}}", ctx)).toBe("15");
    expect(run("{{incvar::金}}|{{decvar::金}}", ctx)).toBe("16|15");
    expect(run("{{addvar::名::旅}}{{addvar::名::人}}{{getvar::名}}", ctx)).toBe("旅人");
    expect(run("{{hasvar::金}}|{{hasvar::無}}|{{deletevar::金}}{{hasvar::金}}", ctx)).toBe("true|false|false");
    expect(run("{{setglobalvar::g::1}}{{getglobalvar::g}}|{{getvar::g}}", ctx)).toBe("1|");
    expect(run("{{setvarkey::表::甲::1}}{{getvarkey::表::甲}}|{{getvar::表}}", ctx)).toBe('1|{"甲":"1"}');
  });

  it("variable shorthand operators", () => {
    const ctx = context();
    expect(run("{{.hp=10}}{{.hp}}|{{.hp++}}|{{.hp-=3}}{{.hp}}|{{.hp>5}}|{{.hp==8}}", ctx)).toBe("10|11|8|true|true");
    expect(run("{{.missing??預設}}|{{.missing||後備}}|{{$g??=7}}|{{$g}}", ctx)).toBe("預設|後備|7|7");
    expect(run("{{if .hp}}有血{{/if}}", ctx)).toBe("有血");
  });
});

describe("chat, random and time macros", () => {
  it("chat history macros", () => {
    expect(run("{{lastMessage}}|{{lastUserMessage}}|{{lastCharMessage}}|{{lastMessageId}}|{{allChatRange}}")).toBe(
      "二樓|我要一間房|二樓|2|0-2",
    );
  });

  it("random/pick/roll/reverse", () => {
    expect(run("{{random::甲::乙::丙}}|{{random 甲, 乙}}|{{random:甲::乙}}")).toBe("甲|甲|甲");
    expect(run("{{roll 6}}|{{roll::2d6+1}}|{{roll::nope}}")).toBe("1|3|");
    expect(run("{{reverse::abc}}")).toBe("cba");
    const first = run("{{pick::甲::乙::丙::丁}}");
    expect(run("{{pick::甲::乙::丙::丁}}")).toBe(first);
  });

  it("time formatting in the default en locale", () => {
    const ctx = context({ now: () => new Date(Date.UTC(2026, 9, 7, 15, 5, 9)) });
    expect(run("{{time_UTC+8}}|{{time::UTC-2}}", ctx)).toBe("11:05 PM|1:05 PM");
    expect(run("{{datetimeformat YYYY/MM/DD [at] HH:mm}}", ctx)).toMatch(/^2026\/10\/0[78] at \d\d:05$/);
    expect(run("{{timeDiff::2026-01-01::2026-01-03}}", ctx)).toBe("2 days ago");
  });
});

describe("moment tokens, seedrandom and pick", () => {
  it("formats Z/ZZ/Q/X/x/S/SS/SSS like moment", () => {
    const instant = new Date(Date.UTC(2026, 9, 7, 15, 5, 9, 123));
    expect(formatUtcOffset(instant, 8, "YYYY-MM-DD HH:mm:ss.SSS Z ZZ [Q]Q X x S SS")).toBe(
      "2026-10-07 23:05:09.123 +08:00 +0800 Q4 1791385509 1791385509123 1 12",
    );
    expect(formatUtcOffset(instant, -5, "Z")).toBe("-05:00");
  });

  it("datetimeformat supports remaining valid moment tokens", () => {
    // 2026-10-07 23:05:09.123 +08:00，星期三
    const instant = new Date(Date.UTC(2026, 9, 7, 15, 5, 9, 123));
    expect(formatUtcOffset(instant, 8, "DDD [week] WW")).toBe("280 week 41");
    expect(formatUtcOffset(instant, 8, "DDDD Do E e w wo gg gggg GG GGGG k kk N Qo YYYYYY")).toBe(
      "280 7th 3 3 41 41st 26 2026 26 2026 23 23 AD 4th +002026",
    );
    expect(formatUtcOffset(instant, 8, "X x Z ZZ [Do X Z]")).toBe("1791385509 1791385509123 +08:00 +0800 Do X Z");
    expect(formatUtcOffset(new Date(Date.UTC(2027, 0, 1, 12)), 0, "YYYY-MM-DD W GGGG")).toBe("2027-01-01 53 2026");
    const ctx = context({ now: () => instant });
    expect(run("{{datetimeformat::DDD [week] WW}}", ctx)).toMatch(/^28[01] week 41$/);
  });

  it("seedrandom matches the published ARC4 test vector", () => {
    expect(seedrandom("hello.")()).toBe(0.9282578795792454);
  });

  it("{{pick}} is stable per chat and position, and differs across chats", () => {
    const text = "{{pick::甲::乙::丙::丁::戊::己::庚::辛}}{{pick::甲::乙::丙::丁::戊::己::庚::辛}}";
    const first = run(text, context({ chatId: "chat-a" }));
    expect(run(text, context({ chatId: "chat-a" }))).toBe(first);
    const others = ["b", "c", "d", "e", "f"].map((id) => run(text, context({ chatId: `chat-${id}` })));
    expect(others.some((value) => value !== first)).toBe(true);
  });
});

describe("ST MacroEngine test vectors (st-macro-cases.json)", () => {
  const stCard = { ...card, name: "Character" };
  for (const test of stCases.cases) {
    it(test.name, () => {
      const variables = createChatVariables({ ...test.variables.local }, { ...test.variables.global });
      const ctx: MacroContext = { card: stCard, userName: "User", chat: [], variables, chatId: "c", random: () => 0 };
      expect(substituteParams(test.input, ctx, { replaceCharacterCard: false })).toBe(test.expected);
    });
  }
});

describe("contract name table", () => {
  it("every name in src/shared/contracts/st-macros.json resolves to a built-in (except the removed charJailbreak)", () => {
    const names = [...stMacros.names, ...stMacros.argument_names].filter((name) => name !== "charjailbreak");
    expect(names.filter((name) => !LIBRARY.get(name))).toEqual([]);
  });
});
