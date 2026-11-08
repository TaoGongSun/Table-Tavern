// 手機客戶端文案入口：照網頁版的機制（web/src/i18n/index.ts）——語系清單與首開偵測沿用桌面版，單複數沿用桌面版 plural.ts；
// 字典各自一檔，型別要求鍵與正典（zh-TW）一致，執行期查不到鍵就退回繁中。目前語系是模組層狀態，換語系後整棵重繪。
import { detectLang, normalizeLang, type Lang } from "@desktop/i18n/languages";
import { expandPlural } from "@desktop/i18n/plural";
import { de } from "./de";
import { en } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import { ja } from "./ja";
import { ko } from "./ko";
import { ptBR } from "./pt-BR";
import { ru } from "./ru";
import { zhCN } from "./zh-CN";
import { zhTW, type PartyMessages } from "./zh-TW";

export type MsgKey = keyof typeof zhTW;
export type { Lang };

export const MESSAGES: Record<Lang, PartyMessages> = {
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
const KEY = "tt-party:lang";

export function t(key: MsgKey, vars: Record<string, string | number> = {}): string {
  const template = (MESSAGES[current] as Partial<PartyMessages>)[key] ?? zhTW[key];
  const text = expandPlural(template, current, vars) ?? template;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    Object.hasOwn(vars, name) ? String(vars[name]) : whole,
  );
}

/** 執行端沒有固定模型名（智慧免費、CLI 預設）時送 "default"，顯示時在地化 */
export const currentLang = () => current;

export const modelName = (label: string) => (label === "default" ? t("modelDefault") : label);

export function initialLang(): Lang {
  try {
    const stored = globalThis.localStorage?.getItem(KEY);
    if (stored) return normalizeLang(stored);
  } catch {
    // 讀不到就照偵測
  }
  return detectLang();
}

/** 只換文案語系（桌面版嵌房間畫面用：不動桌面版的 <html lang>、視窗標題與儲存） */
export function setLangOnly(lang: Lang): void {
  current = lang;
}

/** 切語系：文案、<html lang>、標題；存不了就只在這次有效 */
export function applyLang(lang: Lang): void {
  current = lang;
  document.documentElement.lang = lang;
  document.title = t("metaTitle");
  try {
    globalThis.localStorage?.setItem(KEY, lang);
  } catch {
    // 存不了就算了
  }
}
