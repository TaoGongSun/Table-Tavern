// ST 巨集求值（釘版本 06bde939：macros/engine/MacroEngine.js、MacroCstWalker.js、MacroRegistry.js）。
// 參數先求值（內層先）、成對的 `{{x}}…{{/x}}` 把中間當最後一個參數、不認得的巨集原樣留著（內層已換）、
// 參數個數或型別不合也原樣留著；前處理換舊式 <USER> 等標記，後處理還原 `\{`、處理 {{trim}}。
import { parseDocument, type MacroNode, type Range } from "./macro-parser";
import type { ChatVariables } from "./variables";

export const ELSE_MARKER = "\u0000\u001FELSE\u001F\u0000";

export type ArgType = "string" | "integer" | "number" | "boolean";

export interface ArgSpec {
  type: ArgType | ArgType[];
  optional?: boolean;
}

export interface MacroCall {
  name: string;
  args: string[];
  unnamedArgs: string[];
  list: string[] | null;
  flags: string[];
  isScoped: boolean;
  globalOffset: number;
  env: MacroEnv;
  /** 在同一個環境下對一段文字完整求值（含前後處理），給 if 這類延後求值的巨集用。 */
  resolve: (text: string) => string;
}

export interface MacroDef {
  name: string;
  aliases?: string[];
  args?: ArgSpec[];
  list?: boolean;
  strictArgs?: boolean;
  delayArgResolution?: boolean;
  handler: (call: MacroCall) => unknown;
}

export interface ChatLine {
  isUser: boolean;
  text: string;
  /** 送出時間（毫秒）；idle_duration 用 */
  sentAt?: number;
}

export interface CharacterEnv {
  charPrompt: string;
  charInstruction: string;
  description: string;
  personality: string;
  scenario: string;
  persona: string;
  mesExamplesRaw: string;
  charDepthPrompt: string;
  creatorNotes: string;
  firstMessage: string;
  alternateGreetings: string[];
  version: string;
}

/** 一次求值看得到的世界：名字、卡欄位、對話、變數、時間。 */
export interface MacroEnv {
  contentHash: number;
  names: { user: string; char: string; group: string; groupNotMuted: string; notChar: string };
  character: Partial<CharacterEnv>;
  model: string;
  original: (() => string) | null;
  postProcess: (value: string) => string;
  dynamicMacros: Record<string, string | (() => string)>;
  chat: ChatLine[];
  variables: ChatVariables;
  input: string;
  generationType: string;
  limits: { maxContext: number; maxResponse: number };
  now: () => Date;
  chatId: string;
  random: () => number;
  isMobile: boolean;
}

export class MacroRegistry {
  private readonly defs = new Map<string, MacroDef>();
  private readonly primary = new Map<string, MacroDef>();

  register(def: MacroDef): void {
    this.defs.set(def.name.toLowerCase(), def);
    this.primary.set(def.name.toLowerCase(), def);
    for (const alias of def.aliases ?? []) this.defs.set(alias.toLowerCase(), def);
  }

  get(name: string): MacroDef | undefined {
    return this.defs.get(name.trim().toLowerCase());
  }
}

export function argBounds(def: MacroDef): { min: number; max: number } {
  const specs = def.args ?? [];
  const firstOptional = specs.findIndex((spec) => spec.optional);
  return { min: firstOptional < 0 ? specs.length : firstOptional, max: specs.length };
}

function argsValid(def: MacroDef, count: number): boolean {
  const { min, max } = argBounds(def);
  return def.list ? count >= min : count >= min && count <= max;
}

const isTrueBoolean = (value: string) => ["on", "true", "1"].includes(value.trim().toLowerCase());
export const isFalseBoolean = (value: string) => ["off", "false", "0"].includes(value.trim().toLowerCase());

function valueOfType(value: string, type: ArgType): boolean {
  const trimmed = value.trim();
  if (type === "string") return true;
  if (type === "integer") return /^-?\d+$/.test(trimmed);
  if (type === "number") return Number.isFinite(Number(trimmed));
  return isTrueBoolean(trimmed) || isFalseBoolean(trimmed);
}

export function normalizeResult(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (value instanceof Date) return value.toISOString();
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

/** 成對巨集裡的內容：去頭尾空白，並以第一行非空白的縮排為準整段退縮排。 */
export function trimScopedContent(content: string): string {
  if (!content) return "";
  const lines = content.split("\n");
  const first = lines.find((line) => line.trim() !== "");
  const baseIndent = first ? (/^[ \t]*/.exec(first)?.[0].length ?? 0) : 0;
  if (baseIndent === 0) return content.trim();
  return lines
    .map((line) => {
      const indent = /^[ \t]*/.exec(line)?.[0].length ?? 0;
      return indent >= baseIndent ? line.slice(baseIndent) : line.trimStart();
    })
    .join("\n")
    .trim();
}

interface Item {
  node: MacroNode;
  keepRaw?: boolean;
  scoped?: Range & { closingEnd: number };
}

export class MacroEngine {
  constructor(readonly registry: MacroRegistry) {}

  evaluate(input: string, env: MacroEnv, contextOffset = 0): string {
    if (!input) return "";
    const pre = input
      .replace(/{{time_(UTC[+-]\d+)}}/gi, (_match, offset: string) => `{{time::${offset}}}`)
      .replace(/<USER>/gi, "{{user}}")
      .replace(/<BOT>/gi, "{{char}}")
      .replace(/<CHAR>/gi, "{{char}}")
      .replace(/<GROUP>/gi, "{{group}}")
      .replace(/<CHARIFNOTGROUP>/gi, "{{charIfNotGroup}}");
    let result: string;
    try {
      result = this.evaluateText(pre, contextOffset, env);
    } catch {
      return input;
    }
    return result
      .replace(/\\([{}])/g, "$1")
      .replace(/(?:\r?\n)*{{trim}}(?:\r?\n)*/gi, "")
      .split(ELSE_MARKER)
      .join("");
  }

  /** 一段文字當成獨立文件剖析求值（參數、成對內容都走這支）。 */
  private evaluateText(text: string, offset: number, env: MacroEnv): string {
    if (!text) return "";
    const items = this.pairScopes(parseDocument(text));
    let result = "";
    let cursor = 0;
    for (const item of items) {
      result += text.slice(cursor, item.node.start);
      if (item.keepRaw) {
        result += text.slice(item.node.start, item.node.end);
        cursor = item.node.end;
      } else {
        result += this.evaluateNode(item.node, text, offset, env, item.scoped);
        cursor = item.scoped ? item.scoped.closingEnd : item.node.end;
      }
    }
    return result + text.slice(cursor);
  }

  private info(node: MacroNode): { name: string; closing: boolean } | null {
    if (node.variable || !node.name) return null;
    const closing = node.flags.includes("/") || (node.name === "//" && node.firstArgChar === "/");
    return { name: node.name, closing };
  }

  private canTakeScope(node: MacroNode): boolean {
    const def = this.registry.get(node.name);
    if (!def) return true;
    if (def.list) return false;
    const { min, max } = argBounds(def);
    const count = node.args.length + 1;
    return count >= min && count <= max;
  }

  /** MacroCstWalker #processScopedMacros：只配最外層，內層等內容重剖時再配。 */
  private pairScopes(nodes: MacroNode[]): Item[] {
    const items: Item[] = nodes.map((node) => ({ node }));
    const infos = items
      .map((item, index) => ({ index, item, info: this.info(item.node), matched: false }))
      .filter((entry): entry is { index: number; item: Item; info: { name: string; closing: boolean }; matched: boolean } => entry.info !== null);
    const inside = new Set<number>();
    const remove = new Set<number>();
    for (let i = 0; i < infos.length; i++) {
      const open = infos[i];
      if (open.info.closing || open.matched || inside.has(open.index)) continue;
      let depth = 1;
      let closeAt = -1;
      for (let j = i + 1; j < infos.length; j++) {
        const other = infos[j];
        if (other.info.name.toLowerCase() !== open.info.name.toLowerCase() || other.matched) continue;
        if (other.info.closing) {
          depth -= 1;
          if (depth === 0) {
            closeAt = j;
            break;
          }
        } else if (this.canTakeScope(other.item.node)) depth += 1;
      }
      if (closeAt < 0) continue;
      const close = infos[closeAt];
      open.matched = true;
      close.matched = true;
      if (!this.canTakeScope(open.item.node)) {
        open.item.keepRaw = true;
        close.item.keepRaw = true;
        continue;
      }
      open.item.scoped = { start: open.item.node.end, end: close.item.node.start, closingEnd: close.item.node.end };
      for (let k = open.index + 1; k <= close.index; k++) {
        if (k < close.index) inside.add(k);
        remove.add(k);
      }
    }
    for (const entry of infos) if (entry.info.closing && !entry.matched) entry.item.keepRaw = true;
    return items.filter((_, index) => !remove.has(index));
  }

  private evaluateNode(node: MacroNode, text: string, offset: number, env: MacroEnv, scoped?: Range): string {
    if (node.variable) return this.evaluateVariable(node, text, offset, env);
    const def = this.registry.get(node.name);
    const delay = def?.delayArgResolution === true;
    const args: string[] = [];
    const placed: (Range & { value: string })[] = [];
    for (const range of node.args) {
      if (range.start < 0) {
        args.push("");
        continue;
      }
      const raw = text.slice(range.start, range.end);
      const value = delay ? raw : this.evaluateText(raw, offset + range.start, env);
      args.push(value);
      placed.push({ ...range, value });
    }
    if (scoped) {
      if (scoped.start >= scoped.end) {
        args.push("");
      } else {
        const raw = text.slice(scoped.start, scoped.end);
        let value = delay ? raw : this.evaluateText(raw, offset + scoped.start, env);
        if (!delay && !node.flags.includes("#")) value = trimScopedContent(value);
        args.push(value);
        placed.push({ start: scoped.start, end: scoped.end, value });
      }
    }
    placed.sort((a, b) => a.start - b.start);
    let rawInner = "";
    let cursor = node.innerStart;
    for (const entry of placed) {
      if (entry.start > cursor) rawInner += text.slice(cursor, entry.start);
      rawInner += entry.value;
      cursor = entry.end;
    }
    if (cursor < node.innerEnd) rawInner += text.slice(cursor, node.innerEnd);
    const raw = `{{${rawInner}}}`;

    const dynamic = env.dynamicMacros[node.name.toLowerCase()];
    const effective: MacroDef | undefined =
      dynamic !== undefined
        ? { name: node.name, handler: () => (typeof dynamic === "function" ? dynamic() : String(dynamic ?? "")) }
        : def;
    if (!effective) return raw;
    if (!argsValid(effective, args.length) && effective.strictArgs !== false) return raw;
    const { max } = argBounds(effective);
    const unnamedArgs = args.slice(0, Math.min(args.length, max));
    const specs = effective.args ?? [];
    for (let index = 0; index < Math.min(specs.length, unnamedArgs.length); index++) {
      const types = Array.isArray(specs[index].type) ? (specs[index].type as ArgType[]) : [specs[index].type as ArgType];
      if (!types.some((type) => valueOfType(unnamedArgs[index], type)) && effective.strictArgs !== false) return raw;
    }
    try {
      const result = effective.handler({
        name: effective.name,
        args,
        unnamedArgs,
        list: effective.list ? (args.length > max ? args.slice(max) : []) : null,
        flags: node.flags,
        isScoped: scoped !== undefined,
        globalOffset: offset + node.start,
        env,
        resolve: (content) => this.evaluate(content, env, offset + node.start),
      });
      return env.postProcess(normalizeResult(result));
    } catch {
      return raw;
    }
  }

  private evaluateVariable(node: MacroNode, text: string, offset: number, env: MacroEnv): string {
    const variable = node.variable!;
    const scope = variable.global ? env.variables.global : env.variables.local;
    let cached: string | null = null;
    const value = () => {
      if (cached === null) {
        const range = variable.value;
        cached = range ? this.evaluateText(text.slice(range.start, range.end), offset + range.start, env).trim() : "";
      }
      return cached;
    };
    const falsy = (current: unknown) => !current || isFalseBoolean(normalizeResult(current));
    const numeric = (compare: (a: number, b: number) => boolean) => {
      const a = Number(scope.get(variable.name));
      const b = Number(value());
      return Number.isNaN(a) || Number.isNaN(b) ? "false" : String(compare(a, b));
    };
    switch (variable.operator) {
      case null:
        return normalizeResult(scope.get(variable.name));
      case "=":
        scope.set(variable.name, value());
        return "";
      case "++":
        return normalizeResult(scope.inc(variable.name));
      case "--":
        return normalizeResult(scope.dec(variable.name));
      case "+=":
        scope.add(variable.name, value());
        return "";
      case "-=": {
        const amount = Number(value());
        if (!Number.isNaN(amount)) scope.add(variable.name, -amount);
        return "";
      }
      case "||": {
        const current = scope.get(variable.name);
        return falsy(current) ? normalizeResult(value()) : normalizeResult(current);
      }
      case "??":
        return scope.has(variable.name) ? normalizeResult(scope.get(variable.name)) : normalizeResult(value());
      case "||=": {
        const current = scope.get(variable.name);
        if (!falsy(current)) return normalizeResult(current);
        scope.set(variable.name, value());
        return normalizeResult(value());
      }
      case "??=":
        if (scope.has(variable.name)) return normalizeResult(scope.get(variable.name));
        scope.set(variable.name, value());
        return normalizeResult(value());
      case "==":
        return normalizeResult(scope.get(variable.name)) === normalizeResult(value()) ? "true" : "false";
      case "!=":
        return normalizeResult(scope.get(variable.name)) !== normalizeResult(value()) ? "true" : "false";
      case ">":
        return numeric((a, b) => a > b);
      case ">=":
        return numeric((a, b) => a >= b);
      case "<":
        return numeric((a, b) => a < b);
      case "<=":
        return numeric((a, b) => a <= b);
      default:
        return "";
    }
  }
}
