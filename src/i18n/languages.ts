// 介面語系清單與首開偵測：桌面版與網頁版共用（網頁版經 @desktop 別名引用，不帶字典本體）。
// 新增語系要同時在這裡、各自的字典表加一行。

export const LANGS = ["zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"] as const;

export type Lang = (typeof LANGS)[number];

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

/** 存著的語系值：認得就用，不認得一律繁中 */
export function normalizeLang(value: unknown): Lang {
  return typeof value === "string" && (LANGS as readonly string[]).includes(value) ? (value as Lang) : "zh-TW";
}
