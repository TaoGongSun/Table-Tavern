// 佔位容器讓位（takeover-interface-conflict）：一條介面條目的原文只規定某個容器放哪、容器裡只有 ${…}／{{…}}
// 佔位（例如「格式增强」的 <Status_block>${必须输出行动选项}</Status_block>），另一條條目才真正定義這個容器的
// 內容。展開時模型常把佔位說明當成欄位、自己長出一份（RPGImmortal 的 `行動選項`），合併就撞衝突或多印一個容器。
// 誰是佔位、誰是定義看來源條目原文；佔位方骨架裡同名容器若看起來像佔位（固定文字最多一行、不同狀態路徑最多一個）就整段
// 讓位給定義方的容器，只出現在那裡的路徑連同規則、guide 行剪掉。純函式。
import { classifyPlaceholder } from "./refactor-shell";
import type { RefactorInterface } from "./refactor-review";

/** 一條來源條目原文裡的頂層容器標籤：只有佔位的（placeholder）與含固定文字的（defining） */
export interface SourceContainerRoles {
  placeholder: string[];
  defining: string[];
}

interface Container {
  tag: string;
  start: number;
  end: number;
  inner: string;
}

const OPEN_TAG = /<([A-Za-z_][\w.-]*)>/g;
const SOURCE_PLACEHOLDER = /\$\{[^}]*\}|\{\{[^{}]*\}\}/g;
const SHELL_PLACEHOLDER = /\{\{([^{}\n]+)\}\}/g;

// 依出現順序掃不帶屬性的 `<Tag>…</Tag>`，容器內不再往下找（＝只取頂層）；沒有結尾的開標籤略過。與 frame.rs 同一套掃法。
function topContainers(text: string): Container[] {
  const found: Container[] = [];
  let cursor = 0;
  for (;;) {
    OPEN_TAG.lastIndex = cursor;
    const match = OPEN_TAG.exec(text);
    if (match === null) break;
    const innerStart = match.index + match[0].length;
    const close = `</${match[1]}>`;
    const innerEnd = text.indexOf(close, innerStart);
    if (innerEnd < 0) {
      cursor = innerStart;
      continue;
    }
    found.push({ tag: match[1], start: match.index, end: innerEnd + close.length, inner: text.slice(innerStart, innerEnd) });
    cursor = innerEnd + close.length;
  }
  return found;
}

function placeholderOnly(inner: string): boolean {
  return inner.replace(SOURCE_PLACEHOLDER, "").trim() === "";
}

/** 條目原文的容器角色：同一標籤只要有一處含固定文字就算定義，全部只有佔位（或空）才算佔位。 */
export function sourceContainerRoles(text: string): SourceContainerRoles {
  const placeholder = new Set<string>();
  const defining = new Set<string>();
  for (const container of topContainers(text)) {
    (placeholderOnly(container.inner) ? placeholder : defining).add(container.tag);
  }
  return { placeholder: [...placeholder].filter((tag) => !defining.has(tag)), defining: [...defining] };
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function leafAt(tree: unknown, path: string): boolean {
  let node: unknown = tree;
  for (const key of path.split(".")) {
    if (!isPlainObject(node) || !(key in node)) return false;
    node = node[key];
  }
  return !isPlainObject(node) && !Array.isArray(node);
}

// 骨架片段裡的狀態路徑（正文槽與原卡巨集不算）
function shellPaths(text: string, tree: unknown): Set<string> {
  const paths = new Set<string>();
  for (const match of text.matchAll(SHELL_PLACEHOLDER)) {
    if (classifyPlaceholder(match[1], (path) => leafAt(tree, path)) === "path") paths.add(match[1].trim());
  }
  return paths;
}

// 刪掉一個路徑，沿途清空的父節點一併刪
function deletePath(tree: Record<string, unknown>, keys: string[]): void {
  const [head, ...rest] = keys;
  if (!(head in tree)) return;
  if (rest.length === 0) {
    delete tree[head];
    return;
  }
  const child = tree[head];
  if (!isPlainObject(child)) return;
  deletePath(child, rest);
  if (Object.keys(child).length === 0) delete tree[head];
}

// 骨架片段有沒有正文槽（容許 `{{ 本回合.正文 }}` 這類帶空白寫法）
function hasBody(text: string): boolean {
  return [...text.matchAll(SHELL_PLACEHOLDER)].some((match) => classifyPlaceholder(match[1]) === "body");
}

// 佔位方骨架裡的同名容器要「看起來像佔位」才讓位（輕防來源端掃描盲點，例如說明文字裡的空容器加外層包住的
// 真定義）：去掉佔位後的固定文字最多一個非空行、不同狀態路徑最多一個（`{{x}}{{x}}` 算一個）。`行动选项：{{行动选项}}` 照樣讓位，
// 多行多欄位的真狀態欄、或巢狀 `<B>{{x}}</B>{{y}}` 擋下維持舊行為。
function looksLikeSlot(inner: string, tree: unknown): boolean {
  const fixedLines = inner
    .replace(SHELL_PLACEHOLDER, "")
    .split("\n")
    .filter((line) => line.trim() !== "").length;
  return fixedLines <= 1 && shellPaths(inner, tree).size <= 1;
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// guide 刪行：去掉開頭空白、-、*、反引號後以「被剪路徑：」開頭（路徑後可有收尾的 * 或反引號）的行才考慮刪；
// 同一行若還以「路徑：」提到保留的路徑（混合行）就整行留著。只認完整路徑。
function pruneGuide(guide: string, pruned: string[], kept: string[]): string {
  if (pruned.length === 0) return guide;
  const heads = pruned.map((path) => new RegExp(`^${escapeRegExp(path)}[*\`]*[：:]`));
  const mentions = kept.map((path) => new RegExp(`(?<![\\p{L}\\p{N}._])${escapeRegExp(path)}[*\`]*[：:]`, "u"));
  return guide
    .split("\n")
    .filter((line) => {
      const head = line.replace(/^[\s\-*`]+/, "");
      if (!heads.some((pattern) => pattern.test(head))) return true;
      return mentions.some((pattern) => pattern.test(line));
    })
    .join("\n");
}

function rolesOf(candidate: RefactorInterface, roles: Record<string, SourceContainerRoles>) {
  const placeholder = new Set<string>();
  const defining = new Set<string>();
  for (const uid of candidate.source_uids) {
    for (const tag of roles[uid]?.placeholder ?? []) placeholder.add(tag);
    for (const tag of roles[uid]?.defining ?? []) defining.add(tag);
  }
  for (const tag of defining) placeholder.delete(tag);
  return { placeholder, defining };
}

// 拿掉一段後只收這個接縫的空行：兩側原有的換行取多的那邊、最多留一個空行；沒換行就原樣接起來。
function joinRemoved(left: string, right: string): string {
  const trimmed = left.replace(/[ \t]*$/, "");
  const leftBreaks = /\n*$/.exec(trimmed)![0].length;
  const rightBlank = /^(?:[ \t]*\n)*/.exec(right)![0];
  const rightBreaks = rightBlank.split("\n").length - 1;
  if (leftBreaks === 0 && rightBreaks === 0) return left + right;
  const head = trimmed.slice(0, trimmed.length - leftBreaks);
  const tail = right.slice(rightBlank.length);
  if (head === "") return tail;
  if (tail === "") return head;
  return head + "\n".repeat(Math.min(2, Math.max(leftBreaks, rightBreaks))) + tail;
}

function without(text: string, ranges: { start: number; end: number; replacement: string }[]): string {
  let result = text;
  for (const range of [...ranges].sort((a, b) => b.start - a.start)) {
    const left = result.slice(0, range.start);
    const right = result.slice(range.end);
    result = range.replacement === "" ? joinRemoved(left, right) : left + range.replacement + right;
  }
  return result;
}

/**
 * 佔位容器讓位給定義容器（候選已依合併順序排好）：
 * - 某標籤的定義方＝原文含該標籤固定文字、骨架也有該頂層容器且含固定文字的第一條候選；
 * - 佔位方＝原文該標籤只有佔位的其他候選；它骨架裡同名的頂層容器，固定文字最多一行、不同狀態路徑最多一個
 *   （模型自編的標籤，見 looksLikeSlot）才是佔位容器，含正文槽而定義容器沒有的不動；
 * - 佔位方剪掉只出現在佔位容器裡的路徑（狀態樹、規則、guide 行，清空的父節點一併刪）；
 * - 第一個佔位容器換成定義容器原文、其餘佔位容器拿掉，定義方骨架移除那個容器。
 * 沒有任何可讓位的容器就原樣回傳；之後照一般合併與佔位驗證。
 */
export function resolveContainerSlots(
  ordered: RefactorInterface[],
  roles: Record<string, SourceContainerRoles>,
): RefactorInterface[] {
  const candidateRoles = ordered.map((candidate) => rolesOf(candidate, roles));
  const shells = ordered.map((candidate) => candidate.shell ?? "");
  const edits = ordered.map(() => [] as { start: number; end: number; replacement: string }[]);
  const slotsOf = ordered.map(() => [] as Container[]);
  const tags = new Set(candidateRoles.flatMap((role) => [...role.placeholder]));
  for (const tag of tags) {
    const definerIndex = ordered.findIndex(
      (_, index) =>
        candidateRoles[index].defining.has(tag) &&
        topContainers(shells[index]).some((container) => container.tag === tag && !placeholderOnly(container.inner)),
    );
    if (definerIndex < 0) continue;
    const defined = topContainers(shells[definerIndex]).find(
      (container) => container.tag === tag && !placeholderOnly(container.inner),
    )!;
    const definedHasBody = hasBody(defined.inner);
    let placed = false;
    ordered.forEach((_, index) => {
      if (index === definerIndex || !candidateRoles[index].placeholder.has(tag)) return;
      for (const container of topContainers(shells[index])) {
        if (container.tag !== tag) continue;
        if (hasBody(container.inner) && !definedHasBody) continue;
        if (!looksLikeSlot(container.inner, ordered[index].state_fields)) continue;
        slotsOf[index].push(container);
        edits[index].push({ start: container.start, end: container.end, replacement: placed ? "" : shells[definerIndex].slice(defined.start, defined.end) });
        placed = true;
      }
    });
    if (placed) edits[definerIndex].push({ start: defined.start, end: defined.end, replacement: "" });
  }
  const prunedOf: string[][] = ordered.map(() => []);
  const resolved = ordered.map((candidate, index) => {
    if (edits[index].length === 0) return candidate;
    const next: RefactorInterface = { ...candidate, shell: without(shells[index], edits[index]) };
    if (slotsOf[index].length === 0) return next;
    const tree = candidate.state_fields;
    const outside = shellPaths(
      without(shells[index], slotsOf[index].map((slot) => ({ start: slot.start, end: slot.end, replacement: "" }))),
      tree,
    );
    const pruned = [
      ...new Set(slotsOf[index].flatMap((slot) => [...shellPaths(slot.inner, tree)])),
    ].filter((path) => !outside.has(path));
    if (pruned.length === 0) return next;
    prunedOf[index] = pruned;
    const state = isPlainObject(tree) ? structuredClone(tree) : tree;
    if (isPlainObject(state)) for (const path of pruned) deletePath(state, path.split("."));
    const rules = Object.fromEntries(
      Object.entries(candidate.rules ?? {}).filter(
        ([path]) => !pruned.some((gone) => path === gone || path.startsWith(`${gone}.`)),
      ),
    );
    return { ...next, state_fields: state, ...(candidate.rules ? { rules } : {}) };
  });
  // guide 混合行保護看全部候選最後保留的路徑（佔位方 guide 也可能順帶提到定義方的欄位），但扣掉本條自己剪掉的
  // ——同名同路徑時那是定義方的欄位，佔位方自己那行說明要刪
  const kept = [...new Set(resolved.flatMap((candidate) => Object.keys(candidate.rules ?? {})))];
  return resolved.map((candidate, index) => {
    const pruned = prunedOf[index];
    if (pruned.length === 0 || candidate.guide === undefined) return candidate;
    const others = kept.filter((path) => !pruned.includes(path));
    return { ...candidate, guide: pruneGuide(candidate.guide, pruned, others) };
  });
}
