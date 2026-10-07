import { describe, expect, it } from "vitest";
import { getRegexedString, REGEX_PLACEMENT, regexFromString, regexScriptsFrom, runRegexScript, type RegexScript } from "./regex-scripts";
import type { CardText, MacroContext } from "./substitute";
import { createChatVariables } from "./variables";

const context: MacroContext = {
  card: { name: "莫拉" } as CardText,
  userName: "旅人",
  chat: [],
  variables: createChatVariables(),
  chatId: "c",
};

const script = (overrides: Partial<RegexScript>): RegexScript => ({
  ...regexScriptsFrom({ regex_scripts: [{}] })[0],
  placement: [1, 2],
  ...overrides,
});

describe("regexFromString (ST utils)", () => {
  it("parses /pattern/flags and bare patterns; invalid flags make the whole input the pattern", () => {
    expect(regexFromString("/a+b/gi")?.flags).toBe("gi");
    expect(regexFromString("a+b")?.source).toBe("a+b");
    expect(regexFromString("/x/zz")?.source).toBe("\\/x\\/zz");
    expect(regexFromString("/(/g")).toBeNull();
  });
});

describe("runRegexScript", () => {
  it("replaces {{match}}, numbered and named groups, trims trimStrings and fills macros", () => {
    expect(runRegexScript(script({ findRegex: "/(\\d+)元/g", replaceString: "[{{match}}|$1|{{user}}]" }), "付 30元", context)).toBe(
      "付 [30元|30|旅人]",
    );
    expect(runRegexScript(script({ findRegex: "/(?<who>莫拉)說/", replaceString: "$<who>：" }), "莫拉說嗨", context)).toBe("莫拉：嗨");
    expect(runRegexScript(script({ findRegex: "/<b>(.*?)<\\/b>/g", replaceString: "$1", trimStrings: ["{{char}}"] }), "<b>莫拉笑了</b>", context)).toBe(
      "笑了",
    );
    // 超出群組數的 $n 照 ST 取到 replace 回呼的 offset／整串（offset 1、整串 "ax"）
    expect(runRegexScript(script({ findRegex: "/x/", replaceString: "[$1|$2]" }), "ax", context)).toBe("a[1|ax]");
  });

  it("substituteRegex: raw macros vs escaped macros in the find pattern", () => {
    expect(runRegexScript(script({ findRegex: "/{{char}}/g", replaceString: "她", substituteRegex: 1 }), "莫拉來了", context)).toBe("她來了");
    const dotted = { ...context, card: { name: "A.B" } as CardText };
    expect(runRegexScript(script({ findRegex: "/{{char}}/g", replaceString: "她", substituteRegex: 2 }), "AxB A.B", dotted)).toBe("AxB 她");
    expect(runRegexScript(script({ findRegex: "/{{char}}/g", replaceString: "她", substituteRegex: 0 }), "莫拉", context)).toBe("莫拉");
  });
});

describe("getRegexedString routing", () => {
  const plain = script({ scriptName: "plain", findRegex: "/甲/g", replaceString: "乙" });
  const promptOnly = script({ scriptName: "prompt", findRegex: "/甲/g", replaceString: "丙", promptOnly: true });
  const markdownOnly = script({ scriptName: "md", findRegex: "/甲/g", replaceString: "丁", markdownOnly: true });
  const scripts = [plain, promptOnly, markdownOnly];

  it("splits plain / prompt-only / display-only scripts", () => {
    expect(getRegexedString("甲", REGEX_PLACEMENT.USER_INPUT, scripts, context)).toBe("乙");
    expect(getRegexedString("甲", REGEX_PLACEMENT.USER_INPUT, scripts, context, { isPrompt: true })).toBe("丙");
    expect(getRegexedString("甲", REGEX_PLACEMENT.AI_OUTPUT, scripts, context, { isMarkdown: true })).toBe("丁");
  });

  it("honours placement, disabled, runOnEdit and depth bounds", () => {
    expect(getRegexedString("甲", REGEX_PLACEMENT.WORLD_INFO, scripts, context)).toBe("甲");
    expect(getRegexedString("甲", 1, [{ ...plain, disabled: true }], context)).toBe("甲");
    expect(getRegexedString("甲", 1, [plain], context, { isEdit: true })).toBe("甲");
    expect(getRegexedString("甲", 1, [{ ...plain, runOnEdit: true }], context, { isEdit: true })).toBe("乙");
    const ranged = { ...promptOnly, minDepth: 1, maxDepth: 2 };
    expect([0, 1, 2, 3].map((depth) => getRegexedString("甲", 1, [ranged], context, { isPrompt: true, depth }))).toEqual(["甲", "丙", "丙", "甲"]);
  });
});
