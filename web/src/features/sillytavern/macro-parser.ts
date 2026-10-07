// ST 巨集語法剖析（SillyTavern 釘版本 06bde939：public/scripts/macros/engine/MacroLexer.js、MacroParser.js）。
// 手寫成同一套文法：`{{` 旗標* （變數簡寫 | 名字 參數?） `}}`；參數是 `::` 分隔的清單，或可帶一個 `:`
// 起頭、可含 `::` 的單一參數；參數裡可以巢狀巨集，前後空白不算進參數。沒收尾的 `{{` 當純文字。

export interface Range {
  start: number;
  /** 不含 */
  end: number;
}

export interface MacroNode extends Range {
  /** `{{` 之後、`}}` 之前 */
  innerStart: number;
  innerEnd: number;
  flags: string[];
  /** 變數簡寫（`.x`／`$x`）時為空字串 */
  name: string;
  /** 每個參數去掉前後空白的範圍；空參數 start＝end＝-1 */
  args: Range[];
  /** 第一個參數的第一個字（`{{///}}` 判定收尾用） */
  firstArgChar: string | null;
  variable: VariableExpr | null;
}

export interface VariableExpr {
  global: boolean;
  name: string;
  operator: string | null;
  value: Range | null;
}

const FLAG_CHARS = "!?~#/>";
const IDENTIFIER = /[a-zA-Z][\w-]*/y;
const VAR_IDENTIFIER = /[a-zA-Z](?:[\w-]*\w)?/y;
const OPERATORS = ["++", "--", "??=", "??", "||=", "||", "-=", "==", "!=", ">=", ">", "<=", "<", "+=", "="];
const isSpace = (char: string | undefined) => char !== undefined && /\s/.test(char);

function matchAt(pattern: RegExp, text: string, at: number): string | null {
  pattern.lastIndex = at;
  return pattern.exec(text)?.[0] ?? null;
}

interface Token extends Range {
  kind: "sep" | "colon" | "text";
}

/** 參數區：讀到同層的 `}}` 為止；巢狀巨集整段算一個 token。剖不出來回 null。 */
function scanArgs(text: string, from: number): { tokens: Token[]; end: number } | null {
  const tokens: Token[] = [];
  let at = from;
  while (at < text.length) {
    if (text.startsWith("}}", at)) return { tokens, end: at };
    if (text.startsWith("{{", at)) {
      const nested = parseMacroAt(text, at);
      if (!nested) return null;
      tokens.push({ kind: "text", start: nested.start, end: nested.end });
      at = nested.end;
      continue;
    }
    if (isSpace(text[at])) {
      at += 1;
      continue;
    }
    if (text.startsWith("::", at)) {
      tokens.push({ kind: "sep", start: at, end: at + 2 });
      at += 2;
      continue;
    }
    tokens.push({ kind: text[at] === ":" ? "colon" : "text", start: at, end: at + 1 });
    at += 1;
  }
  return null;
}

const span = (tokens: Token[]): Range =>
  tokens.length === 0 ? { start: -1, end: -1 } : { start: tokens[0].start, end: tokens[tokens.length - 1].end };

function splitArgs(tokens: Token[]): Range[] {
  if (tokens.length === 0) return [];
  if (tokens[0].kind === "sep") {
    const args: Range[] = [];
    let current: Token[] = [];
    for (const token of tokens.slice(1)) {
      if (token.kind === "sep") {
        args.push(span(current));
        current = [];
      } else current.push(token);
    }
    args.push(span(current));
    return args;
  }
  const rest = tokens[0].kind === "colon" ? tokens.slice(1) : tokens;
  return rest.length === 0 ? [] : [span(rest)];
}

function parseVariable(text: string, at: number, start: number, flags: string[]): MacroNode | null {
  const global = text[at] === "$";
  let cursor = at + 1;
  while (isSpace(text[cursor])) cursor += 1;
  const name = matchAt(VAR_IDENTIFIER, text, cursor);
  if (!name) return null;
  cursor += name.length;
  while (isSpace(text[cursor])) cursor += 1;
  let operator: string | null = null;
  let value: Range | null = null;
  if (!text.startsWith("}}", cursor)) {
    operator = OPERATORS.find((candidate) => text.startsWith(candidate, cursor)) ?? null;
    if (!operator) return null;
    cursor += operator.length;
    if (operator === "++" || operator === "--") {
      while (isSpace(text[cursor])) cursor += 1;
      if (!text.startsWith("}}", cursor)) return null;
    } else {
      const scanned = scanArgs(text, cursor);
      if (!scanned) return null;
      value = span(scanned.tokens);
      cursor = scanned.end;
    }
  }
  return {
    start,
    end: cursor + 2,
    innerStart: start + 2,
    innerEnd: cursor,
    flags,
    name: "",
    args: [],
    firstArgChar: null,
    variable: { global, name, operator, value: value && value.start >= 0 ? value : null },
  };
}

/** 從 `{{` 起剖一個完整巨集；語法不成立或沒收尾回 null（呼叫端把 `{{` 當純文字）。 */
export function parseMacroAt(text: string, start: number): MacroNode | null {
  let at = start + 2;
  const flags: string[] = [];
  let name: string | null = null;
  while (at < text.length && name === null) {
    if (text.startsWith("}}", at)) return null;
    if (text.startsWith("//", at)) {
      name = "//";
      at += 2;
      break;
    }
    const char = text[at];
    if (char === "." || char === "$") return parseVariable(text, at, start, flags);
    if (FLAG_CHARS.includes(char)) {
      flags.push(char);
      at += 1;
    } else if (isSpace(char)) {
      at += 1;
    } else {
      name = matchAt(IDENTIFIER, text, at);
      if (!name) return null;
      at += name.length;
      const next = text[at];
      if (!text.startsWith("}}", at) && !isSpace(next) && next !== ":" && next !== "|" && next !== "}") return null;
    }
  }
  if (name === null) return null;
  const scanned = scanArgs(text, at);
  if (!scanned) return null;
  const args = splitArgs(scanned.tokens);
  const firstToken = scanned.tokens.find((token) => token.kind !== "sep" && token.kind !== "colon");
  return {
    start,
    end: scanned.end + 2,
    innerStart: start + 2,
    innerEnd: scanned.end,
    flags,
    name,
    args,
    firstArgChar: firstToken ? text[firstToken.start] : null,
    variable: null,
  };
}

/** 文件裡同層的完整巨集（依位置）；巢狀在參數裡的不列（求值時重剖參數文字）。 */
export function parseDocument(text: string): MacroNode[] {
  const nodes: MacroNode[] = [];
  let at = 0;
  while (at < text.length) {
    const open = text.indexOf("{{", at);
    // 後面已經沒有 `}}`：不可能再有完整巨集（也避免一長串沒收尾的 `{{` 逐個掃到底）
    if (open < 0 || text.indexOf("}}", open + 2) < 0) break;
    // `{{{x}}`：最前面的 `{` 是純文字
    let start = open;
    while (text.startsWith("{{{", start)) start += 1;
    const node = parseMacroAt(text, start);
    if (node) {
      nodes.push(node);
      at = node.end;
    } else {
      at = start + 2;
    }
  }
  return nodes;
}
