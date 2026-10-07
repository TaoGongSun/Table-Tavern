// 卡片 regex 腳本（extensions.regex_scripts），照釘版本 extensions/regex/engine.js getRegexedString／
// runRegexScript。三種時機分流：
// - 一般腳本（非 markdownOnly、非 promptOnly）：訊息落進對話時套一次，改的是存下來的原文
//   （玩家送出 placement 1、模型回覆與開場白 placement 2）。
// - promptOnly：只在組提示時對歷史訊息套（帶深度），存檔不變。
// - markdownOnly：只在畫面顯示時套（帶深度），存檔不變。
import { isObject } from "../cards/card-file";
import { substituteParams, type MacroContext } from "./substitute";

export const REGEX_PLACEMENT = { MD_DISPLAY: 0, USER_INPUT: 1, AI_OUTPUT: 2, SLASH_COMMAND: 3, WORLD_INFO: 5, REASONING: 6 } as const;

export interface RegexScript {
  scriptName: string;
  findRegex: string;
  replaceString: string;
  trimStrings: string[];
  placement: number[];
  disabled: boolean;
  markdownOnly: boolean;
  promptOnly: boolean;
  runOnEdit: boolean;
  substituteRegex: number;
  minDepth: unknown;
  maxDepth: unknown;
}

/** 卡片 extensions.regex_scripts → 腳本清單（欄位缺的照 ST 預設值補）。 */
export function regexScriptsFrom(extensions: unknown): RegexScript[] {
  const raw = isObject(extensions) ? extensions.regex_scripts : undefined;
  if (!Array.isArray(raw)) return [];
  return raw.filter(isObject).map((script) => ({
    scriptName: typeof script.scriptName === "string" ? script.scriptName : "",
    findRegex: typeof script.findRegex === "string" ? script.findRegex : "",
    replaceString: typeof script.replaceString === "string" ? script.replaceString : "",
    trimStrings: Array.isArray(script.trimStrings) ? script.trimStrings.filter((item): item is string => typeof item === "string") : [],
    placement: Array.isArray(script.placement) ? script.placement.filter((item): item is number => typeof item === "number") : [],
    disabled: script.disabled === true,
    markdownOnly: script.markdownOnly === true,
    promptOnly: script.promptOnly === true,
    runOnEdit: script.runOnEdit === true,
    substituteRegex: Number(script.substituteRegex ?? 0),
    minDepth: script.minDepth,
    maxDepth: script.maxDepth,
  }));
}

/** ST utils.regexFromString：`/樣式/旗標` 或裸樣式；旗標不合法就整串當樣式；壞樣式回 null。 */
export function regexFromString(input: string): RegExp | null {
  try {
    const match = /(\/?)(.+)\1([a-z]*)/i.exec(input)!;
    if (match[3] && !/^(?!.*?(.).*?\1)[gmixXsuUAJ]+$/.test(match[3])) return new RegExp(input);
    return new RegExp(match[2], match[3]);
  } catch {
    return null;
  }
}

/** 找式裡的巨集代換結果要當字面字串用時，把 regex 特殊字元跳脫（substituteRegex＝2）。 */
function sanitizeRegexMacro(value: string): string {
  return value.replace(/[\n\r\t\v\f\0.^$*+?{}[\]\\/|()]/g, (char) => {
    const escapes: Record<string, string> = { "\n": "\\n", "\r": "\\r", "\t": "\\t", "\v": "\\v", "\f": "\\f", "\0": "\\0" };
    return escapes[char] ?? `\\${char}`;
  });
}

export function runRegexScript(script: RegexScript, raw: string, context: MacroContext): string {
  if (script.disabled || !script.findRegex || !raw) return raw;
  const source =
    script.substituteRegex === 1
      ? substituteParams(script.findRegex, context)
      : script.substituteRegex === 2
        ? substituteParams(script.findRegex, context, { postProcess: sanitizeRegexMacro })
        : script.findRegex;
  const find = regexFromString(source);
  if (!find) return raw;
  return raw.replace(find, (...args: unknown[]) => {
    let match: unknown = args[0];
    const template = script.replaceString.replace(/{{match}}/gi, "$0");
    const withGroups = template.replace(/\$(\d+)|\$<([^>]+)>/g, (_whole, num?: string, groupName?: string) => {
      if (num) {
        match = args[Number(num)];
      } else if (groupName) {
        const groups = args[args.length - 1];
        match = groups && typeof groups === "object" && (groups as Record<string, unknown>)[groupName];
      }
      if (!match) return "";
      return script.trimStrings.reduce(
        (text, trim) => text.split(substituteParams(trim, context)).join(""),
        String(match),
      );
    });
    return substituteParams(withGroups, context);
  });
}

export interface RegexOptions {
  isMarkdown?: boolean;
  isPrompt?: boolean;
  isEdit?: boolean;
  depth?: number;
}

const hasNumber = (value: unknown) => !isNaN(value as number) && value !== null;

export function getRegexedString(
  raw: string,
  placement: number,
  scripts: RegexScript[],
  context: MacroContext,
  { isMarkdown = false, isPrompt = false, isEdit = false, depth }: RegexOptions = {},
): string {
  if (!raw) return raw;
  let result = raw;
  for (const script of scripts) {
    const applies =
      (script.markdownOnly && isMarkdown) ||
      (script.promptOnly && isPrompt) ||
      (!script.markdownOnly && !script.promptOnly && !isMarkdown && !isPrompt);
    if (!applies || (isEdit && !script.runOnEdit)) continue;
    if (typeof depth === "number") {
      const min = script.minDepth as number;
      const max = script.maxDepth as number;
      if (hasNumber(min) && min >= -1 && depth < min) continue;
      if (hasNumber(max) && max >= 0 && depth > max) continue;
    }
    if (script.placement.includes(placement)) result = runRegexScript(script, result, context);
  }
  return result;
}
