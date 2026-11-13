// 內建規則書目錄：桌設定的選書入口、規則指南與世界書列表共用。書本體是 ST 世界書 JSON，
// 多帶一份 keys_by_lang（匯入時忽略）給列表只顯示目前語系＋英文的關鍵字。
import copy from "./rules-copy.json";
import brp from "./brp.json";
import cairn from "./cairn.json";
import dungeonWorld from "./dungeon-world.json";
import pf2 from "./pf2-remaster.json";
import srd51 from "./srd-5.1.json";
import { currentLang } from "../../../i18n";

export const rulesCopy = () => copy[currentLang()] ?? copy.en;

type KeysByLang = Record<string, Record<string, string[]>>;

export type RuleBookId = "srd51" | "dungeonWorld" | "cairn" | "brp" | "pf2";

type BookFile = {
  name: string;
  entries: Record<string, { comment: string; content: string }>;
  keys_by_lang: KeysByLang;
};

export interface RuleBook {
  id: RuleBookId;
  /** 系統代稱。畫面上的粗體名是語系文案，不用出版商商標當標題 */
  name: string;
  credit: string;
  /** 署名條目內文必有、而且別本沒有的字串 */
  fingerprint: string;
  json: string;
  titles: ReadonlySet<string>;
  /** 標題 → 原始內文：移除時只認標題與內文都沒動過的條目 */
  contents: ReadonlyMap<string, string>;
  keysByLang: KeysByLang;
}

const book = (id: RuleBookId, credit: string, fingerprint: string, data: BookFile): RuleBook => ({
  id,
  name: data.name,
  credit,
  fingerprint,
  json: JSON.stringify(data),
  titles: new Set(Object.values(data.entries).map((entry) => entry.comment)),
  contents: new Map(Object.values(data.entries).map((entry) => [entry.comment, entry.content])),
  keysByLang: data.keys_by_lang,
});

export const RULE_BOOKS: RuleBook[] = [
  book("srd51", "Wizards of the Coast · CC BY 4.0", "dnd.wizards.com/resources/systems-reference-document", srd51),
  book("dungeonWorld", "github.com/Sagelt/Dungeon-World · CC BY 3.0 · includes SRD 5.1 (CC BY 4.0)", "github.com/Sagelt/Dungeon-World", dungeonWorld),
  book("cairn", "Yochai Gal · CC BY-SA 4.0", "github.com/yochaigal/cairn", cairn),
  book("brp", "Chaosium Inc. · Jason Durall, Steve Perrin · ORC License", "BasicRoleplaying-ORC-Content-Document", brp),
  book("pf2", "Paizo Inc. · Pathfinder Player Core · ORC License", "Pathfinder Player Core © 2023 Paizo Inc.", pf2),
];

/** 標題也要屬於這本。Dungeon World 的授權原文含 5e 署名網址，只看內文會把兩本都當成已加入。 */
export const bookAdded = (rules: RuleBook, entries: readonly { title: string; content: string }[]) =>
  entries.some((entry) => rules.titles.has(entry.title) && entry.content.includes(rules.fingerprint));

/** 條目標題對得上內建書就算規則書條目（只影響列表的標籤、排序與關鍵字摺疊，不碰注入）。 */
export const ruleBookOf = (entry: { title: string }) =>
  RULE_BOOKS.find((rules) => rules.titles.has(entry.title));

/** 規則書條目的列表關鍵字：目前語系＋英文＋玩家自加的；其餘語系只回種數。 */
export function ruleKeys(rules: RuleBook, entry: { title: string; keys: string[] }, lang: string) {
  const byLang = rules.keysByLang[entry.title] ?? {};
  const visible = new Set([...(byLang[lang] ?? []), ...(byLang.en ?? [])]);
  const known = new Set(Object.values(byLang).flat());
  // 同一詞在多語系都出現、或玩家自己重複加過：列表只列一次（掃描本來就不分大小寫）
  const seen = new Set<string>();
  const shown = entry.keys.filter((key) => {
    const folded = key.toLowerCase();
    if (seen.has(folded) || !(visible.has(key) || !known.has(key))) return false;
    seen.add(folded);
    return true;
  });
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
