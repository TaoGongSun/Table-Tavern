// 玩家選的語系存在這個瀏覽器（localStorage）；沒選過就照瀏覽器語系偵測（同桌面版首開）。
// 存不了（瀏覽器不讓存）就只在這次有效。也負責把語系反映到頁面：<html lang>、標題、分享描述。
import { detectLang, normalizeLang, type Lang } from "@desktop/i18n/languages";
import { setLang, t } from "./index";

const KEY = "tt-web:lang";

export function initialLang(): Lang {
  try {
    const stored = globalThis.localStorage?.getItem(KEY);
    if (stored) return normalizeLang(stored);
  } catch {
    // 讀不到就照偵測
  }
  return detectLang();
}

export function rememberLang(lang: Lang): void {
  try {
    globalThis.localStorage?.setItem(KEY, lang);
  } catch {
    // 存不了就只在這次有效
  }
}

/** 切到這個語系：文案、<html lang>、頁面標題與描述（搜尋與分享卡片讀得到的那幾個 meta） */
export function applyLang(lang: Lang, doc: Document = document): void {
  setLang(lang);
  doc.documentElement.lang = lang;
  doc.title = t("metaTitle");
  for (const selector of ['meta[name="description"]', 'meta[property="og:description"]']) {
    doc.querySelector(selector)?.setAttribute("content", t("metaDescription"));
  }
  doc.querySelector('meta[property="og:title"]')?.setAttribute("content", t("metaTitle"));
}
