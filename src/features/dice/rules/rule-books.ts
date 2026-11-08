// 內建規則書目錄：桌設定的選書入口、規則指南與世界書列表共用。書本體是 ST 世界書 JSON，
// 多帶一份 keys_by_lang（匯入時忽略）給列表只顯示目前語系＋英文的關鍵字。
import copy from "./rules-copy.json";
import srd51 from "./srd-5.1.json";
import { currentLang } from "../../../i18n";

export const rulesCopy = () => copy[currentLang()] ?? copy.en;

type KeysByLang = Record<string, Record<string, string[]>>;

export interface RuleBook {
  id: "srd51";
  /** 書名與標題一律用系統代稱，不用出版商商標 */
  name: string;
  credit: string;
  /** 署名條目內文必有的字串：世界書裡有它就算已加入 */
  fingerprint: string;
  json: string;
  titles: ReadonlySet<string>;
  /** 標題 → 原始內文：移除時只認標題與內文都沒動過的條目 */
  contents: ReadonlyMap<string, string>;
  keysByLang: KeysByLang;
}

const book = (id: RuleBook["id"], credit: string, fingerprint: string, data: typeof srd51): RuleBook => ({
  id,
  name: data.name,
  credit,
  fingerprint,
  json: JSON.stringify(data),
  titles: new Set(Object.values(data.entries).map((entry) => entry.comment)),
  contents: new Map(Object.values(data.entries).map((entry) => [entry.comment, entry.content])),
  keysByLang: data.keys_by_lang as KeysByLang,
});

export const RULE_BOOKS: RuleBook[] = [
  book("srd51", "Wizards of the Coast · CC BY 4.0", "dnd.wizards.com/resources/systems-reference-document", srd51),
];

export const bookAdded = (rules: RuleBook, contents: readonly { content: string }[]) =>
  contents.some((entry) => entry.content.includes(rules.fingerprint));

/** 條目標題對得上內建書就算規則書條目（只影響列表的標籤、排序與關鍵字摺疊，不碰注入）。 */
export const ruleBookOf = (entry: { title: string }) =>
  RULE_BOOKS.find((rules) => rules.titles.has(entry.title));

/** 規則書條目的列表關鍵字：目前語系＋英文＋玩家自加的；其餘語系只回種數。 */
export function ruleKeys(rules: RuleBook, entry: { title: string; keys: string[] }, lang: string) {
  const byLang = rules.keysByLang[entry.title] ?? {};
  const visible = new Set([...(byLang[lang] ?? []), ...(byLang.en ?? [])]);
  const known = new Set(Object.values(byLang).flat());
  const shown = entry.keys.filter((key) => visible.has(key) || !known.has(key));
  const hidden = Object.entries(byLang).filter(
    ([code, keys]) => code !== lang && code !== "en" && keys.some((key) => entry.keys.includes(key) && !visible.has(key)),
  ).length;
  return { shown, hidden };
}

/** 移除時要刪的條目：標題與內文都跟內建書一字不差才算這本書的；同名但內文改過的保留並回報條數。 */
export function bookEntries(rules: RuleBook, entries: readonly { uid: number; title: string; content: string }[]) {
  const owned: number[] = [];
  let kept = 0;
  for (const entry of entries) {
    const original = rules.contents.get(entry.title);
    if (original === undefined) continue;
    if (original === entry.content) owned.push(entry.uid);
    else kept++;
  }
  return { owned, kept };
}
