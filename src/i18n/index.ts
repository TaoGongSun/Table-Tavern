// UI 語系字典入口：zh-TW 為正典（zh-TW.ts），其他語系逐鍵對應，缺鍵時 TypeScript 直接報錯。
// 語系存 config.preferences.language；App 每次 render 前呼叫 setLang 同步，元件一律經 t() 取字串。
// 模型輸出語言規範在後端 transport.rs，不在這裡。
//
// 新增語系：建 <code>.ts（照 en.ts 格式）→ 在 MESSAGES 與 LANGUAGE_OPTIONS 各加一行。
// 同一語系另需後端 language_rule(transport.rs) 與範例桌內容(src-tauri/samples/<code>.json)，
// 三處缺一律不上該語系。

import { zh, type MsgKey } from "./zh-TW";
import { en } from "./en";
import { zhCN } from "./zh-CN";
import { ja } from "./ja";
import { ko } from "./ko";
import { es } from "./es";
import { ptBR } from "./pt-BR";
import { de } from "./de";
import { fr } from "./fr";
import { ru } from "./ru";

export type { MsgKey };

const MESSAGES = {
  "zh-TW": zh,
  "zh-CN": zhCN,
  en,
  ja,
  ko,
  es,
  "pt-BR": ptBR,
  de,
  fr,
  ru,
} satisfies Record<string, Record<MsgKey, string>>;

export type Lang = keyof typeof MESSAGES;

/** 下拉選單用；label 一律寫該語言自己的名字，外語使用者才認得。 */
export const LANGUAGE_OPTIONS: { value: Lang; label: string }[] = [
  { value: "zh-TW", label: "繁體中文" },
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "es", label: "Español" },
  { value: "pt-BR", label: "Português (Brasil)" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "ru", label: "Русский" },
];

// 系統語系帶地區（pt-PT、es-419、fr-CA…）一律收斂到本 app 有的那一份
const BY_BASE: Record<string, Lang> = {
  en: "en",
  ja: "ja",
  ko: "ko",
  es: "es",
  pt: "pt-BR",
  de: "de",
  fr: "fr",
  ru: "ru",
};

/** 首開語系：依序比對系統偏好語系，中文分繁簡，都對不到就英文 */
export function detectLang(): Lang {
  const tags = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const tag of tags) {
    const lower = tag.toLowerCase();
    if (lower.startsWith("zh")) return /hans|-cn|-sg|-my/.test(lower) ? "zh-CN" : "zh-TW";
    const mapped = BY_BASE[lower.split("-")[0]];
    if (mapped) return mapped;
  }
  return "en";
}

let lang: Lang = "zh-TW";

export function normalizeLang(value: unknown): Lang {
  return typeof value === "string" && value in MESSAGES ? (value as Lang) : "zh-TW";
}

export function setLang(next: Lang) {
  lang = next;
}

export function t(key: MsgKey, params?: Record<string, string | number>): string {
  let text: string = MESSAGES[lang][key];
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.split(`{${name}}`).join(String(value));
    }
  }
  return text;
}
