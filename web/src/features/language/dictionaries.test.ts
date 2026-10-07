// 字典體檢：十個語系的鍵集合跟正典（繁中）一模一樣、佔位符一致、單複數語法看得懂；查不到鍵退回繁中；
// 範例卡各語系齊全；靜態分享 meta 與正典同步。
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LANGS } from "@desktop/i18n/languages";
import { messagePlaceholders } from "@desktop/i18n/plural";
import { SAMPLE_TEXTS, samplePlayCard } from "../cards/sample-card";
import { MemoryStorage } from "../openrouter/memory-storage";
import { MESSAGES, setLang, t, type MsgKey } from "../../i18n";
import { initialLang, rememberLang } from "../../i18n/lang-store";
import { zhTW } from "../../i18n/zh-TW";

const canonKeys = Object.keys(zhTW).sort();
const shape = (text: string) => {
  const result = messagePlaceholders(text);
  return "error" in result ? `語法錯：${result.error}` : [...new Set(result.names)].sort().join(",");
};

afterEach(() => {
  setLang("zh-TW");
  vi.unstubAllGlobals();
});

describe("web dictionaries", () => {
  it("cover exactly the ten desktop languages", () => {
    expect(Object.keys(MESSAGES).sort()).toEqual([...LANGS].sort());
  });

  it.each([...LANGS])("%s has every key of the canon and nothing else", (lang) => {
    const keys = Object.keys(MESSAGES[lang]).sort();
    expect(canonKeys.filter((key) => !keys.includes(key)), "缺的鍵").toEqual([]);
    expect(keys.filter((key) => !canonKeys.includes(key)), "多的鍵").toEqual([]);
    for (const key of keys) expect(MESSAGES[lang][key as MsgKey].trim(), key).not.toBe("");
  });

  it.each([...LANGS])("%s uses the same placeholders as the canon, in valid plural syntax", (lang) => {
    const mismatched = canonKeys.filter((key) => shape(MESSAGES[lang][key as MsgKey]) !== shape(zhTW[key as MsgKey]));
    expect(mismatched.map((key) => `${key}: ${shape(MESSAGES[lang][key as MsgKey])} ≠ ${shape(zhTW[key as MsgKey])}`)).toEqual([]);
  });

  it("picks plural branches by the language's own rules", () => {
    setLang("en");
    expect(t("importRegex", { count: 1 })).toBe("This card has 1 built-in regex script");
    expect(t("importRegex", { count: 3 })).toBe("This card has 3 built-in regex scripts");
    setLang("ru");
    expect(t("savesMeta", { count: 21, time: "t" })).toBe("21 сообщение · t");
    expect(t("savesMeta", { count: 5, time: "t" })).toBe("5 сообщений · t");
    for (const count of [2, 3, 4, 22]) expect(t("savesMeta", { count, time: "t" })).toBe(`${count} сообщения · t`);
    expect(t("importRegex", { count: 3 })).toBe("В этой карточке 3 встроенных regex-скрипта");
  });

  it("falls back to Traditional Chinese when a language is missing a key at runtime", () => {
    const en = MESSAGES.en as Partial<Record<MsgKey, string>>;
    const saved = en.logout;
    delete en.logout;
    try {
      setLang("en");
      expect(t("logout")).toBe(zhTW.logout);
      expect(t("chatSend")).toBe("Send");
    } finally {
      en.logout = saved;
    }
  });
});

describe("language choice", () => {
  it("uses the stored choice first, then the browser's languages like the desktop app", () => {
    const storage = new MemoryStorage();
    vi.stubGlobal("localStorage", storage);
    vi.stubGlobal("navigator", { languages: ["pt-PT", "en"], language: "pt-PT" });
    expect(initialLang()).toBe("pt-BR");
    storage.setItem("tt-web:lang", "ja");
    expect(initialLang()).toBe("ja");
    storage.setItem("tt-web:lang", "xx");
    expect(initialLang()).toBe("zh-TW");
  });

  it("tells Simplified from Traditional Chinese the same way as the desktop app", () => {
    vi.stubGlobal("localStorage", new MemoryStorage());
    const cases: [string, string][] = [
      ["zh-Hans-CN", "zh-CN"],
      ["zh-Hant-HK", "zh-TW"],
      ["zh-SG", "zh-CN"],
      ["zh-TW", "zh-TW"],
      ["zh", "zh-TW"],
      ["xx-YY", "en"],
    ];
    for (const [tag, expected] of cases) {
      vi.stubGlobal("navigator", { languages: [tag], language: tag });
      expect(initialLang(), tag).toBe(expected);
    }
  });

  it("works when the browser throws on storage access (blocked site data)", () => {
    const blocked = {
      getItem: () => {
        throw new DOMException("blocked", "SecurityError");
      },
      setItem: () => {
        throw new DOMException("blocked", "SecurityError");
      },
    };
    vi.stubGlobal("localStorage", blocked);
    vi.stubGlobal("navigator", { languages: ["ko-KR"], language: "ko-KR" });
    expect(initialLang()).toBe("ko");
    expect(() => rememberLang("ja")).not.toThrow();
  });
});

describe("built-in sample card", () => {
  it.each([...LANGS])("%s has a complete one-line-named card with the same macros as the canon", (lang) => {
    const text = SAMPLE_TEXTS[lang];
    const canon = SAMPLE_TEXTS["zh-TW"];
    expect(Object.keys(text).sort()).toEqual(Object.keys(canon).sort());
    expect(text.name).not.toMatch(/[\r\n]/);
    for (const field of ["description", "personality", "scenario", "first_mes"] as const) {
      expect(text[field].trim(), field).not.toBe("");
      const macros = (value: string) => (value.match(/\{\{\w+\}\}/g) ?? []).sort();
      expect(macros(text[field]), field).toEqual(macros(canon[field]));
    }
    expect(text.tags).toHaveLength(canon.tags.length);
    const card = samplePlayCard(lang);
    expect(card.text.name).toBe(text.name);
    expect(card.openings).toEqual([text.first_mes]);
  });
});

describe("static share meta", () => {
  it("index.html carries the canon title and description (crawlers do not run scripts)", () => {
    const html = readFileSync(new URL("../../../index.html", import.meta.url), "utf8");
    const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    expect(html).toMatch(new RegExp(`<title>${escape(zhTW.metaTitle)}</title>`));
    expect(html.match(/content="([^"]*)"/g)?.filter((attr) => attr.includes(zhTW.metaDescription))).toHaveLength(2);
    expect(html).toContain(`property="og:title" content="${zhTW.metaTitle}"`);
  });
});
