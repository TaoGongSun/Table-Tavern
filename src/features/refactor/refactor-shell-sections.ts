// 骨架的集合區段（takeover-list-structured）：`{{#集合}}…{{/集合}}` 依項目順序展開成多份模板，區段內的
// 佔位符改綁成項目的絕對路徑（`{{身份}}`→`{{在場人物.林远.身份}}`），`{{@key}}`／`{{@n}}` 綁成虛擬葉子。
// 這裡只複製模板、不放任何值；值由 refactor-shell.ts 的填值器一次替換與跳脫，值裡的 `{{…}}`、引號、換行
// 不會被二次解讀。集合形狀看後端的集合描述（mechanism.collections），不從樹或模板推。

import type { StateNode } from "./refactor-shell";

export type CollectionFieldType = "text" | "number" | "bool";

/** 一個集合的形狀（後端 `Mechanism.collections` 的值）：具名（以名字為鍵）或序列（0..n-1）、項目是物件或純量 */
export interface CollectionSpec {
  keyed: boolean;
  item: "object" | "scalar";
  /** 物件項目的欄位 → 型別 */
  fields?: Record<string, CollectionFieldType>;
  /** 純量項目的型別 */
  type?: CollectionFieldType;
}

/** 點分路徑 → 集合描述 */
export type Collections = Record<string, CollectionSpec>;

/** 具名項目的順序葉子：app 管、GM 唯讀，殼與狀態欄都不顯示 */
export const ORDER_KEY = "_序";

// 虛擬葉子的根鍵：`{{@key}}`／`{{@n}}` 綁到這底下的流水號，值只存在填值用的樹副本裡
const VIRTUAL_ROOT = "@";
const PLACEHOLDER = /\{\{([^{}\n]+)\}\}/g;
// 區段標記各自獨佔一行的寫法（YAML 區要求）：標記行本身丟掉，中間的行整段照項目重複
const BLOCK_SECTION = /^[ \t]*\{\{#\s*([^{}\n]+?)\s*\}\}[ \t]*\n([\s\S]*?)^[ \t]*\{\{\/\s*\1\s*\}\}[ \t]*(?:\n|$)/gm;
// 行內寫法（非 YAML 區，例：`<道具>{{#道具}}<i>{{.}}</i>{{/道具}}</道具>`）
const INLINE_SECTION = /\{\{#\s*([^{}\n]+?)\s*\}\}([\s\S]*?)\{\{\/\s*\1\s*\}\}/g;
// 值為空的 YAML 鍵（`人物列表:`、`- 选项:`）：底下的區段展開為零項時補 ` []`，卡讀到空陣列而不是 null
const EMPTY_YAML_KEY = /^[ \t]*(?:-[ \t]+)*[^\s#][^#]*:[ \t]*$/;
// 項目鍵含這些字元就綁不成點分路徑佔位符（後端也會拒收），略過不展開
const UNBINDABLE_KEY = /[.{}\n]/;

function orderRank(key: string, node: StateNode | undefined): number {
  if (typeof node === "object" && node !== null) {
    const order = node[ORDER_KEY];
    if (typeof order === "string" && /^-?\d+$/.test(order.trim())) return Number(order.trim());
  }
  return /^\d+$/.test(key) ? Number(key) : Number.POSITIVE_INFINITY;
}

/**
 * 集合項目的顯示順序：具名項目看 `_序`、序列看索引數值（不照字碼，10 排在 9 後面）；排不出名次的接在最後、
 * 依鍵排。底線開頭的鍵不是項目。
 */
export function orderedItemKeys(node: Record<string, StateNode>): string[] {
  return Object.keys(node)
    .filter((key) => !key.startsWith("_"))
    .map((key) => ({ key, rank: orderRank(key, node[key]) }))
    .sort((a, b) => {
      if (a.rank !== b.rank) return a.rank < b.rank ? -1 : 1;
      return a.key < b.key ? -1 : a.key > b.key ? 1 : 0;
    })
    .map(({ key }) => key);
}

function branchAt(tree: Record<string, StateNode>, path: string): Record<string, StateNode> | null {
  let node: StateNode | undefined = tree;
  for (const key of path.split(".")) {
    if (typeof node !== "object" || node === null) return null;
    node = node[key];
  }
  return typeof node === "object" && node !== null ? node : null;
}

/**
 * 一份骨架的區段綁定：`bind` 逐區（YAML 區、一般區）展開區段，`tree`／`valueTypes` 給之後的填值器用
 * （加上虛擬葉子與項目欄位的型別）。沒有集合描述時原文不動。
 */
export class SectionBinder {
  private readonly virtual: Record<string, string> = {};
  private readonly types: Record<string, string> = {};
  private count = 0;

  constructor(
    private readonly source: Record<string, StateNode>,
    private readonly collections: Collections,
  ) {}

  bind(text: string, yaml: boolean): string {
    if (Object.keys(this.collections).length === 0 || !text.includes("{{#")) return text;
    let out = "";
    let last = 0;
    for (const found of text.matchAll(BLOCK_SECTION)) {
      const [match, path, inner] = found;
      const head = out + text.slice(last, found.index);
      const items = this.expand(path, inner);
      if (items === null) out = head + match;
      // 空集合：前一行是值為空的 YAML 鍵就接上 ` []`（`人物列表:` → `人物列表: []`）
      else if (items === "" && yaml && emptyListMark(head)) out = `${head.slice(0, -1)} []${match.endsWith("\n") ? "\n" : ""}`;
      else out = head + items;
      last = found.index + match.length;
    }
    out += text.slice(last);
    return out.replace(INLINE_SECTION, (match: string, path: string, inner: string) => this.expand(path, inner) ?? match);
  }

  /** 填值用的樹：原樹加上虛擬葉子 */
  tree(): Record<string, StateNode> {
    return this.count === 0 ? this.source : { ...this.source, [VIRTUAL_ROOT]: { ...this.virtual } };
  }

  /** 填值用的型別表：原表加上項目欄位與序號的型別 */
  valueTypes(base: Record<string, string>): Record<string, string> {
    return Object.keys(this.types).length === 0 ? base : { ...base, ...this.types };
  }

  // 一個區段依項目展開；沒有這個集合的描述回 null（標記原樣留著）
  private expand(path: string, inner: string): string | null {
    const spec = Object.prototype.hasOwnProperty.call(this.collections, path) ? this.collections[path] : undefined;
    if (spec === undefined) return null;
    const node = branchAt(this.source, path);
    if (node === null) return "";
    return orderedItemKeys(node)
      .filter((key) => !UNBINDABLE_KEY.test(key))
      .map((key, index) => this.bindItem(path, spec, key, index, inner))
      .join("");
  }

  private bindItem(path: string, spec: CollectionSpec, key: string, index: number, inner: string): string {
    const itemPath = `${path}.${key}`;
    return inner.replace(PLACEHOLDER, (match: string, raw: string) => {
      const token = raw.trim();
      if (token === "@key") return this.virtualLeaf(key, "text");
      if (token === "@n") return this.virtualLeaf(String(index + 1), "number");
      if (token === "." && spec.item === "scalar") return this.bound(itemPath, spec.type);
      const fields = spec.item === "object" ? (spec.fields ?? {}) : {};
      if (Object.prototype.hasOwnProperty.call(fields, token)) return this.bound(`${itemPath}.${token}`, fields[token]);
      return match;
    });
  }

  private bound(path: string, type: CollectionFieldType | undefined): string {
    if (type === "number" || type === "bool") this.types[path] = type;
    return `{{${path}}}`;
  }

  private virtualLeaf(value: string, type: CollectionFieldType): string {
    const id = String(this.count);
    this.count += 1;
    this.virtual[id] = value;
    return this.bound(`${VIRTUAL_ROOT}.${id}`, type);
  }
}

// 區段前一行是值為空的 YAML 鍵（而且區段正好從新的一行開始）
function emptyListMark(before: string): boolean {
  if (!before.endsWith("\n")) return false;
  const lines = before.slice(0, -1).split("\n");
  return EMPTY_YAML_KEY.test(lines[lines.length - 1] ?? "");
}
