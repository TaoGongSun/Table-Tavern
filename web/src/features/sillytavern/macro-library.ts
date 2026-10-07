// ST 內建巨集（釘版本 06bde939：macros/definitions/{env,chat,core,time,state,variable}-macros.js）。
// 名單涵蓋 src/shared/contracts/st-macros.json；網頁版沒有的東西（群組、instruct、擴充）
// 照 ST 在單人 Chat Completion 下的值回（空字串或 false）。
import { ELSE_MARKER, isFalseBoolean, MacroEngine, MacroRegistry, trimScopedContent, type MacroCall, type MacroDef } from "./macro-engine";
import { parseDocument } from "./macro-parser";
import { formatLocal, formatUtcOffset, humanize, timeDiff } from "./macro-time";
import { parseMesExamples } from "./mes-examples";
import { seedrandom } from "./seedrandom";

/** ST utils.getStringHash（cyrb53）。 */
export function getStringHash(text: string, seed = 0): number {
  let h1 = 0xdeadbeef ^ seed;
  let h2 = 0x41c6ce57 ^ seed;
  for (let index = 0; index < text.length; index++) {
    const char = text.charCodeAt(index);
    h1 = Math.imul(h1 ^ char, 2654435761);
    h2 = Math.imul(h2 ^ char, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return 4294967296 * (2097151 & h2) + (h1 >>> 0);
}

/** 單一參數的舊寫法 {{random a,b}}：有 `::` 優先用它切，否則用逗號（`\,` 是字面逗號）。 */
function singleArgList(text: string): string[] {
  if (text.includes("::")) return text.split("::").map((item) => item.trim());
  return text
    .replace(/\\,/g, "\u0000COMMA\u0000")
    .split(",")
    .map((item) => item.trim().replace(/\u0000COMMA\u0000/g, ","));
}

/** droll 子集：`XdY`、`dY`、`XdY±Z`。 */
function rollDice(formula: string, random: () => number): string {
  const match = /^(\d*)d(\d+)([+-]\d+)?$/i.exec(formula.trim());
  if (!match) return "";
  const count = match[1] ? Number(match[1]) : 1;
  const sides = Number(match[2]);
  if (count < 1 || sides < 1 || count > 1000) return "";
  let total = Number(match[3] ?? 0);
  for (let index = 0; index < count; index++) total += 1 + Math.floor(random() * sides);
  return String(total);
}

function splitOnTopLevelElse(content: string): { then: string; otherwise?: string } {
  let depth = 0;
  for (const node of parseDocument(content)) {
    if (node.variable) continue;
    const closing = node.flags.includes("/");
    if (node.name === "if" && !closing && node.args.length === 1) depth += 1;
    else if (node.name === "if" && closing) depth -= 1;
    else if (node.name === "else" && depth === 0) {
      return { then: content.slice(0, node.start), otherwise: content.slice(node.end) };
    }
  }
  return { then: content };
}

const lastIndex = (call: MacroCall, filter?: (isUser: boolean) => boolean): number | null => {
  for (let index = call.env.chat.length - 1; index >= 0; index--) {
    if (!filter || filter(call.env.chat[index].isUser)) return index;
  }
  return null;
};

const str = () => ({ type: "string" as const });
const optionalInt = { type: "integer" as const, optional: true };
const strOrNum = { type: ["string", "number"] as ("string" | "number")[] };

function variableMacros(prefix: "" | "global"): MacroDef[] {
  const scope = (call: MacroCall) => (prefix ? call.env.variables.global : call.env.variables.local);
  const name = (base: string) => base.replace("var", `${prefix}var`);
  return [
    { name: name("setvar"), args: [str(), strOrNum], handler: (c) => (scope(c).set(c.unnamedArgs[0], c.unnamedArgs[1]), "") },
    { name: name("addvar"), args: [str(), strOrNum], handler: (c) => (scope(c).add(c.unnamedArgs[0], c.unnamedArgs[1]), "") },
    { name: name("incvar"), args: [str()], handler: (c) => scope(c).inc(c.unnamedArgs[0]) },
    { name: name("decvar"), args: [str()], handler: (c) => scope(c).dec(c.unnamedArgs[0]) },
    { name: name("getvar"), args: [str()], handler: (c) => scope(c).get(c.unnamedArgs[0]) },
    {
      name: name("hasvar"),
      aliases: [name("varexists")],
      args: [str()],
      handler: (c) => (scope(c).has(c.unnamedArgs[0]) ? "true" : "false"),
    },
    { name: name("deletevar"), aliases: [name("flushvar")], args: [str()], handler: (c) => scope(c).del(c.unnamedArgs[0]) },
    {
      name: name("setvarkey"),
      aliases: [name("setvarindex")],
      args: [str(), strOrNum, strOrNum],
      handler: (c) => (scope(c).set(c.unnamedArgs[0], c.unnamedArgs[2], { index: c.unnamedArgs[1] }), ""),
    },
    {
      name: name("getvarkey"),
      aliases: [name("getvarindex")],
      args: [str(), strOrNum],
      handler: (c) => scope(c).get(c.unnamedArgs[0], { index: c.unnamedArgs[1] }),
    },
  ];
}

const DEFINITIONS: MacroDef[] = [
  // ── 名字與卡欄位（env-macros.js）
  { name: "user", handler: (c) => c.env.names.user },
  { name: "char", handler: (c) => c.env.names.char },
  { name: "group", aliases: ["charIfNotGroup"], handler: (c) => c.env.names.group },
  { name: "groupNotMuted", handler: (c) => c.env.names.groupNotMuted },
  { name: "notChar", handler: (c) => c.env.names.notChar },
  { name: "charPrompt", handler: (c) => c.env.character.charPrompt ?? "" },
  { name: "charInstruction", handler: (c) => c.env.character.charInstruction ?? "" },
  { name: "charDescription", aliases: ["description"], handler: (c) => c.env.character.description ?? "" },
  { name: "charPersonality", aliases: ["personality"], handler: (c) => c.env.character.personality ?? "" },
  { name: "charScenario", aliases: ["scenario"], handler: (c) => c.env.character.scenario ?? "" },
  { name: "persona", handler: (c) => c.env.character.persona ?? "" },
  { name: "mesExamplesRaw", handler: (c) => c.env.character.mesExamplesRaw ?? "" },
  { name: "mesExamples", handler: (c) => parseMesExamples(c.env.character.mesExamplesRaw ?? "").join("") },
  { name: "charDepthPrompt", handler: (c) => c.env.character.charDepthPrompt ?? "" },
  { name: "charCreatorNotes", aliases: ["creatorNotes"], handler: (c) => c.env.character.creatorNotes ?? "" },
  {
    name: "charFirstMessage",
    aliases: ["greeting"],
    args: [optionalInt],
    handler: (c) => {
      const index = Number(c.unnamedArgs[0] ?? 0);
      if (index === 0) return c.env.character.firstMessage ?? "";
      return c.env.character.alternateGreetings?.[index - 1] ?? "";
    },
  },
  { name: "charVersion", aliases: ["version", "char_version"], handler: (c) => c.env.character.version ?? "" },
  { name: "model", handler: (c) => c.env.model },
  {
    name: "original",
    handler: (c) => {
      // ST：沒有可代換的原文時 env.functions.original 不存在，呼叫會丟錯，巨集原樣保留
      if (!c.env.original) throw new Error("no original");
      return c.env.original();
    },
  },
  { name: "isMobile", handler: (c) => String(c.env.isMobile) },

  // ── 對話（chat-macros.js）；網頁版沒有 swipe
  { name: "lastMessage", handler: (c) => c.env.chat[lastIndex(c) ?? -1]?.text ?? "" },
  { name: "lastMessageId", handler: (c) => String(lastIndex(c) ?? "") },
  { name: "lastUserMessage", handler: (c) => c.env.chat[lastIndex(c, (user) => user) ?? -1]?.text ?? "" },
  { name: "lastCharMessage", handler: (c) => c.env.chat[lastIndex(c, (user) => !user) ?? -1]?.text ?? "" },
  { name: "firstIncludedMessageId", handler: (c) => (c.env.chat.length ? "0" : "") },
  { name: "firstDisplayedMessageId", handler: (c) => (c.env.chat.length ? "0" : "") },
  { name: "lastSwipeId", handler: () => "" },
  { name: "currentSwipeId", handler: () => "" },
  { name: "allChatRange", handler: (c) => (c.env.chat.length ? `0-${c.env.chat.length - 1}` : "") },

  // ── 工具（core-macros.js）
  { name: "space", args: [optionalInt], handler: (c) => " ".repeat(Number(c.unnamedArgs[0] ?? 1)) },
  { name: "newline", args: [optionalInt], handler: (c) => "\n".repeat(Number(c.unnamedArgs[0] ?? 1)) },
  { name: "noop", handler: () => "" },
  { name: "trim", args: [{ type: "string", optional: true }], handler: (c) => (c.isScoped ? (c.unnamedArgs[0] ?? "") : "{{trim}}") },
  {
    name: "if",
    args: [str(), str()],
    delayArgResolution: true,
    handler: (c) => {
      const [rawCondition, rawContent] = c.unnamedArgs;
      const inverted = /^\s*!/.test(rawCondition);
      let condition = c.resolve(inverted ? rawCondition.replace(/^\s*!\s*/, "") : rawCondition);
      const shorthand = /^([.$])([a-zA-Z](?:[\w-]*\w)?)$/.exec(condition);
      if (shorthand) {
        condition = c.resolve(`{{${shorthand[1] === "." ? "getvar" : "getglobalvar"}::${shorthand[2]}}}`);
      } else {
        const def = LIBRARY.get(condition);
        if (def && (def.args ?? []).every((spec) => spec.optional) && condition.trim() !== "") {
          condition = c.resolve(`{{${condition}}}`);
        }
      }
      let falsy = condition === "" || isFalseBoolean(condition);
      if (inverted) falsy = !falsy;
      const { then, otherwise } = splitOnTopLevelElse(rawContent);
      const chosen = falsy ? otherwise : then;
      if (chosen === undefined) return "";
      const result = c.resolve(chosen);
      return c.flags.includes("#") ? result : trimScopedContent(result);
    },
  },
  { name: "else", handler: () => ELSE_MARKER },
  { name: "input", handler: (c) => c.env.input },
  { name: "maxPrompt", aliases: ["maxPromptTokens"], handler: (c) => String(c.env.limits.maxContext - c.env.limits.maxResponse) },
  { name: "maxContext", aliases: ["maxContextTokens"], handler: (c) => String(c.env.limits.maxContext) },
  { name: "maxResponse", aliases: ["maxResponseTokens"], handler: (c) => String(c.env.limits.maxResponse) },
  { name: "reverse", args: [str()], handler: (c) => Array.from(c.unnamedArgs[0]).reverse().join("") },
  { name: "//", aliases: ["comment"], args: [str()], handler: () => "" },
  {
    name: "roll",
    args: [str()],
    handler: (c) => {
      const formula = c.unnamedArgs[0];
      return rollDice(/^\d+$/.test(formula) ? `1d${formula}` : formula, c.env.random);
    },
  },
  {
    name: "random",
    list: true,
    handler: (c) => {
      const list = c.list!.length === 1 ? singleArgList(c.list![0]) : c.list!;
      return list.length ? list[Math.floor(c.env.random() * list.length)] : "";
    },
  },
  {
    name: "pick",
    list: true,
    handler: (c) => {
      const list = c.list!.length === 1 ? singleArgList(c.list![0]) : c.list!;
      if (!list.length) return "";
      // 照 ST：種子＝hash(對話 id 的 hash-整段內容 hash-位置)，seedrandom 位元相同；網頁版沒有 /reroll-pick
      const seed = getStringHash([getStringHash(c.env.chatId), c.env.contentHash, c.globalOffset].join("-"));
      return list[Math.floor(seedrandom(String(seed))() * list.length)];
    },
  },
  { name: "banned", args: [str()], handler: () => "" },
  {
    name: "outlet",
    args: [str()],
    handler: (c) => (c.unnamedArgs[0] && Object.prototype.hasOwnProperty.call(c.env.outlets, c.unnamedArgs[0]) ? c.env.outlets[c.unnamedArgs[0]] : ""),
  },

  // ── 時間（time-macros.js）
  {
    name: "time",
    args: [{ type: "string", optional: true }],
    handler: (c) => {
      const match = /^UTC([+-]\d+)$/.exec(c.unnamedArgs[0] ?? "");
      return match ? formatUtcOffset(c.env.now(), Number.parseInt(match[1], 10), "LT") : formatLocal(c.env.now(), "LT");
    },
  },
  { name: "date", handler: (c) => formatLocal(c.env.now(), "LL") },
  { name: "weekday", handler: (c) => formatLocal(c.env.now(), "dddd") },
  { name: "isotime", handler: (c) => formatLocal(c.env.now(), "HH:mm") },
  { name: "isodate", handler: (c) => formatLocal(c.env.now(), "YYYY-MM-DD") },
  { name: "datetimeformat", args: [str()], handler: (c) => formatLocal(c.env.now(), c.unnamedArgs[0]) },
  {
    name: "idleDuration",
    aliases: ["idle_duration"],
    handler: (c) => {
      let takeNext = false;
      for (let index = c.env.chat.length - 1; index >= 0; index--) {
        const line = c.env.chat[index];
        if (line.isUser && takeNext) {
          return line.sentAt === undefined ? "just now" : humanize(c.env.now().getTime() - line.sentAt);
        }
        takeNext = true;
      }
      return "just now";
    },
  },
  {
    name: "timeDiff",
    args: [str(), str()],
    handler: (c) => timeDiff(c.unnamedArgs[0], c.unnamedArgs[1]),
  },

  // ── 狀態（state-macros.js）
  { name: "lastGenerationType", handler: (c) => c.env.generationType },
  { name: "hasExtension", args: [str()], handler: () => "false" },

  // ── 變數（variable-macros.js）
  ...variableMacros(""),
  ...variableMacros("global"),
];

export const LIBRARY = new MacroRegistry();
for (const def of DEFINITIONS) LIBRARY.register(def);

export const ENGINE = new MacroEngine(LIBRARY);
