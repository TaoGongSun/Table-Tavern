// 字典字串的單複數：只認 ICU MessageFormat 的一小塊子集，類別交給平台 Intl.PluralRules。
//
// 文法：{arg, plural, 類別 {分支} 類別 {分支} …}
// - 類別只收 zero／one／two／few／many／other，必須有 other，不得重複；
// - 分支內 # 代該數字，可放一般佔位符 {name}，不得再有其他大括號（巢狀 plural 不收）；
// - =n、offset:、ICU 單引號跳脫（'{ '} '' '#）都不支援，單引號一律是普通字元。
// 字典語法由 scripts/check-i18n.mjs 擋；執行期遇到壞語法就原樣回傳，不半解析。

export const PLURAL_CATEGORIES = ["zero", "one", "two", "few", "many", "other"] as const;
type PluralCategory = (typeof PLURAL_CATEGORIES)[number];

export type PluralBlock = {
  start: number;
  end: number;
  arg: string;
  branches: Partial<Record<PluralCategory, string>>;
};

export type ParsedMessage = { ok: true; blocks: PluralBlock[] } | { ok: false; error: string };

const PLACEHOLDER = /\{\w+\}/y;
const HEAD = /\{(\w+)\s*,\s*plural\s*,/y;
const SELECTOR = /\s*([^\s{}]+)\s*\{/y;
const BLOCK_END = /\s*\}/y;
const ESCAPE_LIKE = /'[{}'#]/;

function matchAt(pattern: RegExp, text: string, at: number) {
  pattern.lastIndex = at;
  return pattern.exec(text);
}

/** 解析整串：一般佔位符跳過，plural 區塊收起來；其他任何大括號都算語法錯。 */
export function parseMessage(text: string): ParsedMessage {
  const blocks: PluralBlock[] = [];
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (ch === "}") return { ok: false, error: `第 ${i} 字有多出來的 }` };
    if (ch !== "{") {
      i += 1;
      continue;
    }
    const plain = matchAt(PLACEHOLDER, text, i);
    if (plain) {
      i += plain[0].length;
      continue;
    }
    const head = matchAt(HEAD, text, i);
    if (!head) return { ok: false, error: `第 ${i} 字的 { 不是佔位符也不是 plural` };
    const block = parseBlock(text, i, head[1], i + head[0].length);
    if ("error" in block) return { ok: false, error: block.error };
    blocks.push(block);
    i = block.end;
  }
  return { ok: true, blocks };
}

function parseBlock(text: string, start: number, arg: string, from: number): PluralBlock | { error: string } {
  const branches: PluralBlock["branches"] = {};
  let i = from;
  for (;;) {
    const close = matchAt(BLOCK_END, text, i);
    if (close) {
      if (branches.other === undefined) return { error: `plural {${arg}} 缺 other` };
      return { start, end: i + close[0].length, arg, branches };
    }
    const selector = matchAt(SELECTOR, text, i);
    if (!selector) return { error: `plural {${arg}} 括號不成對或分支格式錯` };
    const category = selector[1] as PluralCategory;
    if (!PLURAL_CATEGORIES.includes(category)) {
      return { error: `plural {${arg}} 不支援的類別 ${selector[1]}` };
    }
    if (branches[category] !== undefined) return { error: `plural {${arg}} 類別 ${category} 重複` };
    i += selector[0].length;
    const bodyStart = i;
    for (;;) {
      if (i >= text.length) return { error: `plural {${arg}} 括號不成對` };
      if (text[i] === "}") break;
      if (text[i] === "{") {
        const plain = matchAt(PLACEHOLDER, text, i);
        if (!plain) return { error: `plural {${arg}} 分支內只能放一般佔位符（不支援巢狀）` };
        i += plain[0].length;
        continue;
      }
      i += 1;
    }
    const body = text.slice(bodyStart, i);
    if (ESCAPE_LIKE.test(body)) return { error: `plural {${arg}} 分支不支援 ICU 單引號跳脫` };
    branches[category] = body;
    i += 1;
  }
}

const RULES = new Map<string, Intl.PluralRules>();
function rulesFor(lang: string) {
  let rules = RULES.get(lang);
  if (!rules) {
    rules = new Intl.PluralRules(lang);
    RULES.set(lang, rules);
  }
  return rules;
}

/** 數字或純數字字串才算數；其他（缺、NaN、亂字）一律走 other。 */
function countOf(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "string" && /^-?\d+(\.\d+)?$/.test(value.trim())) {
    const count = Number(value);
    return Number.isFinite(count) ? count : null;
  }
  return null;
}

const LOOKS_PLURAL = /\{\w+\s*,\s*plural\s*,/;

/**
 * 把 plural 區塊換成選中的分支，# 換成 {arg}：只動模板，參數代入仍由 t() 單次掃描完成，
 * 代入值裡的 {…}／# 不會再被解讀；參數沒給時跟一般缺參數一樣原樣留著。
 * 帶 plural 語法卻解析失敗時回傳 null，呼叫端整串原樣回傳、不再代入。
 */
export function expandPlural(
  text: string,
  lang: string,
  params: Record<string, string | number> | undefined,
): string | null {
  // 查不到鍵時舊行為是原樣回傳 undefined，這裡不改變它
  if (typeof text !== "string" || !LOOKS_PLURAL.test(text)) return text;
  const parsed = parseMessage(text);
  if (!parsed.ok) return null;
  let out = "";
  let last = 0;
  for (const block of parsed.blocks) {
    const given = params !== undefined && Object.prototype.hasOwnProperty.call(params, block.arg);
    const count = given ? countOf(params[block.arg]) : null;
    const category = count === null ? "other" : (rulesFor(lang).select(count) as PluralCategory);
    const branch = block.branches[category] ?? block.branches.other ?? "";
    out += text.slice(last, block.start) + branch.replace(/#/g, `{${block.arg}}`);
    last = block.end;
  }
  return out + text.slice(last);
}

/** 一般佔位符名（含重複），不看 plural 語法。 */
const plainNames = (text: string) => (text.match(/\{\w+\}/g) ?? []).map((p) => p.slice(1, -1));

/**
 * 字典體檢用：整串用到的佔位符（含重複、已排序）。plural 區塊算一次參數，
 * 分支內的一般佔位符各分支須相同、只算一次；語法錯或分支不一致回傳 error。
 */
export function messagePlaceholders(text: string): { names: string[] } | { error: string } {
  const parsed = parseMessage(text);
  if (!parsed.ok) return { error: parsed.error };
  const names: string[] = [];
  let rest = "";
  let last = 0;
  for (const block of parsed.blocks) {
    rest += text.slice(last, block.start);
    last = block.end;
    const shapes = Object.values(block.branches).map((branch) => plainNames(branch).sort().join(","));
    if (new Set(shapes).size > 1) return { error: `plural {${block.arg}} 各分支佔位符不一致` };
    names.push(block.arg, ...plainNames(block.branches.other ?? ""));
  }
  names.push(...plainNames(rest + text.slice(last)));
  return { names: names.sort() };
}

/** 字典體檢用：每個 plural 區塊都取量起來最寬的分支（# 以 {參數} 計），語法錯就照原字串量。 */
export function widestRendering(text: string, measure: (text: string) => number): string {
  const parsed = parseMessage(text);
  if (!parsed.ok) return text;
  let out = "";
  let last = 0;
  for (const block of parsed.blocks) {
    const options = Object.values(block.branches).map((b) => b.replace(/#/g, `{${block.arg}}`));
    const widest = options.reduce((a, b) => (measure(b) > measure(a) ? b : a));
    out += text.slice(last, block.start) + widest;
    last = block.end;
  }
  return out + text.slice(last);
}
