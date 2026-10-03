// AI 卡重構接管介面時產的骨架（interface-shell.html，後端 refactor_interface_shell 讀回來，讀不到是
// null）：照搬卡規定的每回合輸出格式，變動處是 `{{狀態樹路徑}}` 佔位符（例如 `{{World.Time}}`），
// 正文槽固定 `{{本回合.正文}}`。這裡把佔位符換成狀態樹目前的值；填完的骨架交給卡自己的顯示
// 腳本渲染（選路見 card-interface/card-shell-route.ts）。純函式、零 UI／invoke 依賴。

/** 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）。 */
export type StateNode = string | { [key: string]: StateNode };

import yaml from "js-yaml";
import stMacros from "../../shared/contracts/st-macros.json";

// 佔位符只認 `{{...}}`：內容不含花括號或換行的簡單形式。
const PLACEHOLDER_REGEX = /\{\{([^{}\n]+)\}\}/g;

/** 正文槽：App 每回合拿模型的訊息正文填進去。 */
export const BODY_PLACEHOLDER = "本回合.正文";

/**
 * 骨架佔位符契約（解析驗證與填值共用，後端 result_parse.rs 同一套規則、同一份巨集清單
 * src/shared/contracts/st-macros.json）：
 * - `body`：正文槽 `{{本回合.正文}}`，永遠照正文原樣填，不做任何格式轉換；
 * - `path`：狀態樹裡有這個葉子就是狀態引用（優先於巨集名，`{{Time}}` 有葉子就填值）；
 * - `macro`：不是狀態葉子、而且是已知的酒館巨集（無參名稱，或 `名稱::參數`／`名稱:參數` 的已知帶參名稱），
 *   原樣保留；
 * - 其餘一律當狀態路徑（含未知的冒號寫法），查不到就是缺欄位（後端驗證會拒收）。
 */
export type PlaceholderKind = "body" | "macro" | "path";

export function isStMacro(token: string): boolean {
  const trimmed = token.trim().toLowerCase();
  if (stMacros.names.includes(trimmed)) return true;
  const colon = trimmed.indexOf(":");
  return colon > 0 && stMacros.argument_names.includes(trimmed.slice(0, colon));
}

export function classifyPlaceholder(token: string, isLeaf: (path: string) => boolean = () => false): PlaceholderKind {
  const trimmed = token.trim();
  if (trimmed === BODY_PLACEHOLDER) return "body";
  if (isLeaf(trimmed)) return "path";
  return isStMacro(trimmed) ? "macro" : "path";
}

// 逐層查狀態樹；查不到節點、或路徑中途／終點落在分支（非葉子）都回 undefined——殼只讀不寫，缺值就是沒東西可顯示。
function lookupLeaf(tree: Record<string, StateNode>, path: string): string | undefined {
  const keys = path.split(".");
  let node: StateNode | undefined = tree[keys[0]];
  for (const key of keys.slice(1)) {
    if (typeof node !== "object" || node === null) return undefined;
    node = node[key];
  }
  return typeof node === "string" ? node : undefined;
}

/** 佔位符的值；巨集回 null＝原樣保留，查不到的路徑回空字串 */
function resolve(tree: Record<string, StateNode>, token: string): string | null {
  const path = token.trim();
  const kind = classifyPlaceholder(path, (candidate) => lookupLeaf(tree, candidate) !== undefined);
  if (kind === "macro") return null;
  return lookupLeaf(tree, path) ?? "";
}

function isBody(token: string): boolean {
  return token.trim() === BODY_PLACEHOLDER;
}

function fillRaw(text: string, tree: Record<string, StateNode>): string {
  return text.replace(PLACEHOLDER_REGEX, (match, token: string) => resolve(tree, token) ?? match);
}

/**
 * 把骨架裡的佔位符換成狀態樹的值。骨架填完是餵給卡自己顯示腳本（regex＋模板）的「每回合輸出」：
 * - 一般區域：值原文放回。清單欄位的值照卡原文含內層標籤（如 `<Item>…</Item>`），escape 會讓卡的殼
 *   解析不到；信任模型與直玩餵 event.raw 相同——模型原文進沙盒 iframe，不多做一層。
 * - `yamlTags` 指定的容器（例如卡用 YAML 解析器讀的 `<Status_block>`）之內：值照所在位置的 YAML 語法
 *   表示，卡讀到的才是原值（規則見 fillYamlRegion）。容器外（正文槽、其他格式）照原樣填。
 */
export function fillSkeletonPlaceholders(
  skeleton: string,
  tree: Record<string, StateNode>,
  yamlTags: string[] = [],
  valueTypes: Record<string, string> = {},
): string {
  const ranges = yamlRanges(skeleton, yamlTags);
  let out = "";
  let cursor = 0;
  for (const [start, end] of ranges) {
    out += fillRaw(skeleton.slice(cursor, start), tree);
    out += fillYamlRegion(skeleton.slice(start, end), tree, valueTypes);
    cursor = end;
  }
  return out + fillRaw(skeleton.slice(cursor), tree);
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// 容器內文是不是 YAML 結構：略過空行、註解（#）、文件標記（--- 與 ...）與只有佔位符的行之後，
// 第一行是「鍵:」或「- 」開頭。像 `<maintext>{{本回合.正文}}</maintext>` 這種只裝正文的容器不算。
function looksLikeYaml(content: string): boolean {
  const first = content
    .split("\n")
    .map((line) => line.trim())
    .find(
      (line) =>
        line !== "" &&
        !line.startsWith("#") &&
        !/^(---|\.\.\.)(\s|$)/.test(line) &&
        !/^\{\{[^{}\n]+\}\}$/.test(line),
    );
  return first !== undefined && /^(-\s|-$|[^\s:#{][^:#]*:(\s|$))/.test(first);
}

// 每個 YAML 容器 `<tag>…</tag>` 的內文範圍（內文確實是 YAML 結構才算），依位置排序、不重疊
function yamlRanges(skeleton: string, tags: string[]): [number, number][] {
  const ranges: [number, number][] = [];
  for (const tag of new Set(tags)) {
    const name = escapeRegExp(tag);
    const regex = new RegExp(`<${name}>([\\s\\S]*?)</${name}>`, "g");
    let match: RegExpExecArray | null;
    while ((match = regex.exec(skeleton)) !== null) {
      if (!looksLikeYaml(match[1])) continue;
      const start = match.index + tag.length + 2;
      ranges.push([start, start + match[1].length]);
    }
  }
  ranges.sort((a, b) => a[0] - b[0]);
  return ranges.filter((range, i) => i === 0 || range[0] >= ranges[i - 1][1]);
}

// 會被 YAML（js-yaml 預設 schema：core＋timestamp＋merge）讀成非字串的純量：null、布林、整數（含 0x／0o／
// 0b／底線）、浮點（含科學記號、.inf、.nan）、日期時間、合併鍵。狀態樹的值都是字串，照原樣放會改型別，一律加引號。
// YAML 1.1 的 yes／no／on／off 也一併加引號，避免別的解析器轉型。
const YAML_IMPLICIT = [
  /^(~|null|Null|NULL)$/,
  /^(true|True|TRUE|false|False|FALSE|yes|Yes|YES|no|No|NO|on|On|ON|off|Off|OFF|y|Y|n|N)$/,
  /^[-+]?(0b[01_]+|0x[0-9a-fA-F_]+|0o?[0-7_]+|[0-9][0-9_]*)$/,
  /^[-+]?(\.[0-9]+|[0-9][0-9_]*(\.[0-9_]*)?)([eE][-+]?[0-9]+)?$/,
  /^[-+]?\.(inf|Inf|INF)$|^\.(nan|NaN|NAN)$/,
  /^[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}/,
  /^<<$/,
];
// 純量開頭不能是指示字元、不能含「: 」「 #」、前後不能有空白、不能含控制字元，否則讀到的值會變
// eslint-disable-next-line no-control-regex
const YAML_UNSAFE_PLAIN = /^[\s\-?:,[\]{}#&*!|>'"%@`]|\s$|: |:$| #|[\u0000-\u001f]/;
// 多行值本身是 YAML 結構（清單或鍵值）時照結構插入，否則當成多行文字
const YAML_BLOCK_STRUCTURE = /^\s*(-\s|-$|[^\s:#][^:#]*:(\s|$))/;
// 開啟區塊純量的那一行：`鍵: |`、`- >-` 之類
const YAML_BLOCK_OPENER = /^\s*(?:-\s+|[^#\s][^#]*?:\s+)[|>][+-]?[0-9]?\s*$/;
// 一行的開頭：縮排、清單記號、鍵（鍵不能以引號、#、{ 開頭）
const YAML_LINE = /^(\s*(?:-\s+)?)((?:[^\s"'#{][^:#]*?|"(?:[^"\\]|\\.)*"|'(?:[^']|'')*'):(?:\s+|$))?(.*)$/;
const SINGLE_PLACEHOLDER = /^\{\{([^{}\n]+)\}\}$/;

function plainScalar(text: string): string {
  if (text === "" || YAML_IMPLICIT.some((regex) => regex.test(text)) || YAML_UNSAFE_PLAIN.test(text)) {
    return JSON.stringify(text);
  }
  return text;
}

// 欄位型別表（mechanism.value_types，套用時由後端記下）：number／bool 取自模型產出的 STATE 初始 JSON 葉子型別，
// list 取自欄位規則 `kind: list`。number／bool 欄的值是合法字面就照原樣寫，卡讀回數字／布林；其餘一律照字串
// 規則（會被轉型就加引號），所以一般欄與行內集合裡的同一個值讀回同一個型別。
const YAML_NUMBER_LITERAL = /^[-+]?([0-9]+(\.[0-9]+)?|\.[0-9]+)([eE][-+]?[0-9]+)?$/;

function typedScalar(value: string, path: string | null, valueTypes: Record<string, string>): string {
  const type = path === null ? undefined : valueTypes[path];
  if (type === "number" && YAML_NUMBER_LITERAL.test(value)) return value;
  if (type === "bool" && (value === "true" || value === "false")) return value;
  return plainScalar(value);
}

// 雙引號字串內的值：JSON 字串跳脫就是合法的 YAML 雙引號跳脫
function escapeDouble(value: string): string {
  return JSON.stringify(value).slice(1, -1);
}

/**
 * YAML 容器內逐行填值，依完整純量與區塊上下文決定值怎麼寫：
 * - 區塊純量（`鍵: |` 底下縮排較深的行）：值原文放進去，多行值每行補上同樣縮排；
 * - 雙引號純量：值以 YAML 雙引號跳脫寫入；
 * - 單引號純量：填好後內容含換行就整個改寫成雙引號純量，否則照單引號規則（`'`→`''`）；
 * - 一般純量：佔位符是整個值且值有多行時，YAML 結構照結構縮排插入、文字寫成 `|-` 區塊；其餘把固定文字
 *   與值組成完整純量後再判斷要不要加引號（`📍 北境 # 後院` 這種會被讀成註解的就整個加引號）；
 *   行尾原有的註解保留。
 * 巨集原樣保留、當成固定文字。
 */
function fillYamlRegion(
  region: string,
  tree: Record<string, StateNode>,
  valueTypes: Record<string, string>,
): string {
  const out: string[] = [];
  let blockParent: number | null = null;
  for (const line of region.split("\n")) {
    const indent = /^ */.exec(line)?.[0].length ?? 0;
    if (blockParent !== null) {
      if (line.trim() === "" || indent > blockParent) {
        out.push(fillBlockLine(line, indent, tree));
        continue;
      }
      blockParent = null;
    }
    if (!/\{\{[^{}\n]+\}\}/.test(line)) {
      out.push(line);
      if (YAML_BLOCK_OPENER.test(line)) blockParent = indent;
      continue;
    }
    // 正文槽走正文契約：整行照原樣填，永不 YAML 化
    if ([...line.matchAll(PLACEHOLDER_REGEX)].some((match) => isBody(match[1]))) {
      out.push(fillRaw(line, tree));
      continue;
    }
    out.push(fillYamlLine(line, indent, tree, valueTypes));
  }
  return out.join("\n");
}

function fillBlockLine(line: string, indent: number, tree: Record<string, StateNode>): string {
  const pad = " ".repeat(indent);
  return line.replace(PLACEHOLDER_REGEX, (match, token: string) => {
    const value = resolve(tree, token);
    return value === null ? match : value.replace(/\r?\n/g, `\n${pad}`);
  });
}

function fillYamlLine(
  line: string,
  indent: number,
  tree: Record<string, StateNode>,
  valueTypes: Record<string, string>,
): string {
  const parsed = YAML_LINE.exec(line);
  const lead = parsed?.[1] ?? "";
  const key = parsed?.[2] ?? "";
  const rest = parsed?.[3] ?? line;
  if (rest.startsWith('"')) {
    let end = 1;
    while (end < rest.length && rest[end] !== '"') end += rest[end] === "\\" ? 2 : 1;
    const content = rest.slice(1, end).replace(PLACEHOLDER_REGEX, (match, token: string) => {
      const value = resolve(tree, token);
      return value === null ? match : escapeDouble(value);
    });
    return `${lead}${key}"${content}"${rest.slice(end + 1)}`;
  }
  if (rest.startsWith("'")) {
    let end = 1;
    while (end < rest.length) {
      if (rest[end] === "'" && rest[end + 1] === "'") end += 2;
      else if (rest[end] === "'") break;
      else end += 1;
    }
    const full = fillRaw(rest.slice(1, end).replace(/''/g, "'"), tree);
    const scalar = full.includes("\n") ? JSON.stringify(full) : `'${full.replace(/'/g, "''")}'`;
    return `${lead}${key}${scalar}${rest.slice(end + 1)}`;
  }
  if (/^\[|^\{(?!\{)/.test(rest)) return `${lead}${key}${fillFlowCollection(rest, tree, valueTypes)}`;
  // 一般純量：先分出行尾註解（固定文字裡的「 #」；佔位符裡的不算）
  const masked = rest.replace(PLACEHOLDER_REGEX, (match) => "x".repeat(match.length));
  const commentAt = masked.search(/\s#/);
  const scalarText = (commentAt === -1 ? rest : rest.slice(0, commentAt)).trimEnd();
  const comment = commentAt === -1 ? "" : rest.slice(commentAt);
  const single = SINGLE_PLACEHOLDER.exec(scalarText);
  const singleValue = single ? resolve(tree, single[1]) : null;
  if (single && singleValue !== null) {
    const value = singleValue;
    const lines = value.replace(/\r\n/g, "\n").split("\n");
    if (lead.trim() === "" && key === "") {
      // 整行只有佔位符：這個位置放的是結構層級的內容（例如一整串清單），值照原樣插在這個縮排，
      // 多行時後續各行補上同樣縮排
      return `${" ".repeat(indent)}${lines.join(`\n${" ".repeat(indent)}`)}${comment}`;
    }
    if (lines.length > 1) {
      const child = " ".repeat(indent + 2);
      const body = lines.map((text) => (text === "" ? "" : `${child}${text}`)).join("\n");
      const head = `${lead}${key}`.trimEnd();
      const structure = YAML_BLOCK_STRUCTURE.test(lines.find((text) => text.trim() !== "") ?? "");
      return structure ? `${head}${comment}\n${body}` : `${head} |-${comment}\n${body}`;
    }
  }
  if (single && singleValue !== null) {
    return `${lead}${key}${typedScalar(singleValue, single[1].trim(), valueTypes)}${comment}`;
  }
  const full = fillRaw(scalarText, tree);
  const scalar = full.includes("\n") ? JSON.stringify(full) : plainScalar(full);
  return `${lead}${key}${scalar}${comment}`;
}

const PLACEHOLDER_AT = /\{\{([^{}\n]+)\}\}/y;
const FLOW_INDICATOR = ",[]{}";

// 行內集合裡的單一元素：型別照型別表；含行內集合指示字元（`,[]{}`）或換行就整個加引號，值不會被拆開或截斷
function flowElement(value: string, path: string | null, valueTypes: Record<string, string>): string {
  if (/[,[\]{}]/.test(value)) return JSON.stringify(value);
  return typedScalar(value, path, valueTypes);
}

// 集合片段解析出來的值要能可靠地嵌回：只收 JSON 型別（字串、有限數字、布林、null、陣列、一般物件），物件只看
// 自有屬性、原型必須是 Object.prototype，循環參照與展開後過大（alias 炸彈）一律拒收
const FRAGMENT_NODE_BUDGET = 10000;

function isPlainData(value: unknown): boolean {
  let budget = FRAGMENT_NODE_BUDGET;
  const ancestors = new Set<object>();
  const walk = (node: unknown): boolean => {
    if ((budget -= 1) < 0) return false;
    if (node === null || typeof node === "string" || typeof node === "boolean") return true;
    if (typeof node === "number") return Number.isFinite(node);
    if (typeof node !== "object" || ancestors.has(node)) return false;
    const isArray = Array.isArray(node);
    if (!isArray && Object.getPrototypeOf(node) !== Object.prototype) return false;
    ancestors.add(node);
    const ok = (isArray ? node : Object.values(node)).every(walk);
    ancestors.delete(node);
    return ok;
  };
  return walk(value);
}

function sameData(a: unknown, b: unknown): boolean {
  if (typeof a !== "object" || a === null || typeof b !== "object" || b === null) return Object.is(a, b);
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const keysA = Object.keys(a);
  const keysB = Object.keys(b);
  return (
    keysA.length === keysB.length &&
    keysA.every(
      (key) =>
        Object.prototype.hasOwnProperty.call(b, key) &&
        sameData((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]),
    )
  );
}

// 片段用 YAML core schema 解析：沒有合併鍵（<<）與日期型別，結果只有 JSON 型別；解析不了回 undefined
function parseFlow(text: string): unknown {
  try {
    return yaml.load(text, { schema: yaml.CORE_SCHEMA });
  } catch {
    return undefined;
  }
}

/**
 * 集合片段（只有 `kind: list` 欄位的值、填在 `[{{x}}]` 裡才是）：
 * - 值本身是明確的 YAML 序列（`[a, b]` 或 `- a` 區塊清單）就取它的元素；
 * - 普通多行文字是一個字串元素，保留換行；
 * - 單行值照 YAML 行內序列的內容解析：引號與巢狀照 YAML 規則，`"a, b", c` 是兩個元素，全形「甲，乙」不是分隔
 *   符號（一個元素）。
 * 各元素保留讀到的型別，逐個寫成 JSON（JSON 是合法的 YAML 行內寫法），寫完重新解析核對與原元素相同才採用。
 * 解析不了、含循環或非 JSON 型別、核對不符的值，整份原值當一個字串元素。
 */
function flowFragment(value: string): string {
  const whole = JSON.stringify(value);
  const own = parseFlow(value);
  let elements: unknown;
  if (Array.isArray(own)) elements = own;
  else if (/\r?\n/.test(value)) return whole;
  else elements = parseFlow(`[${value}]`);
  if (!Array.isArray(elements) || !isPlainData(elements)) return whole;
  const written = elements.map((element) => JSON.stringify(element)).join(", ");
  return sameData(parseFlow(`[${written}]`), elements) ? written : whole;
}

// 一般純量（不在引號裡）的結尾：行內集合指示字元、後面接空白或指示字元的冒號、空白後的 #；佔位符整個跳過
function plainEnd(rest: string, from: number): number {
  let j = from;
  while (j < rest.length) {
    PLACEHOLDER_AT.lastIndex = j;
    const placeholder = PLACEHOLDER_AT.exec(rest);
    if (placeholder !== null) {
      j += placeholder[0].length;
      continue;
    }
    const char = rest[j];
    const next = rest[j + 1] ?? "";
    if (FLOW_INDICATOR.includes(char)) break;
    if (char === ":" && (next === "" || /\s/.test(next) || FLOW_INDICATOR.includes(next))) break;
    if (char === "#" && j > from && /\s/.test(rest[j - 1])) break;
    j += 1;
  }
  return j;
}

/**
 * 行內集合（`[…]`／`{…}`）逐個元素掃描，結構照卡格式保留，每個含佔位符的元素都以完整元素表示：
 * - 雙引號純量：值以 YAML 雙引號跳脫寫入；
 * - 單引號純量：填好後內容含換行就整個改寫成雙引號純量（保留換行），否則照單引號規則（`'`→`''`）；
 * - 一般純量只有一個佔位符：是 `[{{x}}]` 唯一內容、而且欄位型別是 list 時當集合片段（見 flowFragment），
 *   否則是單一元素（flowElement，遵守型別表）；
 * - 一般純量是固定文字夾佔位符（`前綴{{x}}`）：整個元素填好後當字串，會被轉型、含指示字元或換行就整個加引號；
 *   原卡巨集 token 在引號裡原樣保留（`"{{user}}前綴…"`）；目前合成顯示流程（填骨架→卡片 regex→沙盒快照→
 *   卡片腳本解析）不自動替換巨集。日後若加入巨集替換，須在值替換完成後再做 YAML 跳脫；
 * - 行內註解（空白後的 #）以後原樣保留。
 * 引號只在元素開頭才算開始引號純量，`it's` 這種一般純量裡的引號不算。
 */
function fillFlowCollection(
  rest: string,
  tree: Record<string, StateNode>,
  valueTypes: Record<string, string>,
): string {
  let out = "";
  let i = 0;
  while (i < rest.length) {
    const char = rest[i];
    PLACEHOLDER_AT.lastIndex = i;
    const startsWithPlaceholder = PLACEHOLDER_AT.test(rest);
    if (!startsWithPlaceholder && (FLOW_INDICATOR.includes(char) || char === ":" || /\s/.test(char))) {
      out += char;
      i += 1;
      continue;
    }
    if (char === "#" && /\s/.test(rest[i - 1] ?? "")) {
      out += rest.slice(i);
      break;
    }
    if (char === '"') {
      let end = i + 1;
      while (end < rest.length && rest[end] !== '"') end += rest[end] === "\\" ? 2 : 1;
      const content = rest.slice(i + 1, end).replace(PLACEHOLDER_REGEX, (match, token: string) => {
        const value = resolve(tree, token);
        return value === null ? match : escapeDouble(value);
      });
      out += `"${content}"`;
      i = end + 1;
      continue;
    }
    if (char === "'") {
      let end = i + 1;
      while (end < rest.length) {
        if (rest[end] === "'" && rest[end + 1] === "'") end += 2;
        else if (rest[end] === "'") break;
        else end += 1;
      }
      const full = fillRaw(rest.slice(i + 1, end).replace(/''/g, "'"), tree);
      out += full.includes("\n") ? JSON.stringify(full) : `'${full.replace(/'/g, "''")}'`;
      i = end + 1;
      continue;
    }
    const end = plainEnd(rest, i);
    const token = rest.slice(i, end);
    const element = token.trimEnd();
    const trailing = token.slice(element.length);
    const previous = rest.slice(0, i).trimEnd().slice(-1);
    const after = rest.slice(end).trimStart().slice(0, 1);
    i = end;
    const tokens = [...element.matchAll(PLACEHOLDER_REGEX)].map((match) => match[1]);
    if (tokens.length === 0) {
      out += token;
      continue;
    }
    const single = SINGLE_PLACEHOLDER.exec(element);
    const value = single ? resolve(tree, single[1]) : null;
    if (single && value === null) {
      out += token;
    } else if (single && value !== null) {
      const path = single[1].trim();
      const fragment = previous === "[" && after === "]" && valueTypes[path] === "list";
      out += (fragment ? flowFragment(value) : flowElement(value, path, valueTypes)) + trailing;
    } else {
      out += flowElement(fillRaw(element, tree), null, valueTypes) + trailing;
    }
  }
  return out;
}
