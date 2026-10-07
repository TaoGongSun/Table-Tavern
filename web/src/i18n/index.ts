// 網頁版文案入口：語系清單與首開偵測沿用桌面版（@desktop/i18n/languages），單複數沿用桌面版 plural.ts。
// 字典各自一檔，型別要求鍵與正典（zh-TW）一致；萬一執行期查不到鍵就退回繁中。
// 目前語系是模組層狀態：App 換語系時 setLang 後整棵重繪，元件一律經 t() 取字串。
import { expandPlural } from "@desktop/i18n/plural";
import type { Lang } from "@desktop/i18n/languages";
import { de } from "./de";
import { en } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import { ja } from "./ja";
import { ko } from "./ko";
import { ptBR } from "./pt-BR";
import { ru } from "./ru";
import { zhCN } from "./zh-CN";
import { zhTW, type WebMessages } from "./zh-TW";

export type MsgKey = keyof typeof zhTW;
export type { Lang };

export const MESSAGES: Record<Lang, WebMessages> = {
  "zh-TW": zhTW,
  "zh-CN": zhCN,
  en,
  ja,
  ko,
  es,
  "pt-BR": ptBR,
  de,
  fr,
  ru,
};

let current: Lang = "zh-TW";

export function setLang(next: Lang): void {
  current = next;
}

export function getLang(): Lang {
  return current;
}

/** 取文案：單複數照該語系的規則選分支，再代入 `{name}` 佔位；佔位沒給值就原樣留著，方便發現漏傳。 */
export function t(key: MsgKey, vars: Record<string, string | number> = {}): string {
  const template = (MESSAGES[current] as Partial<WebMessages>)[key] ?? zhTW[key];
  const text = expandPlural(template, current, vars);
  // 單複數語法壞掉：整串原樣回傳，不半代入（字典語法由 features/language/dictionaries.test.ts 擋）
  if (text === null) return template;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : whole));
}
