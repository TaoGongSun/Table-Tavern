// 字典體檢（照網頁版 features/language/dictionaries.test.ts）：十個語系的鍵集合與正典（繁中）一樣、佔位符一致、單複數語法看得懂。
// 根目錄的 `npm run check:i18n` 只掃桌面版 src/i18n，不掃 party/，所以這裡自己查。
import { describe, expect, it } from "vitest";
import { LANGS } from "@desktop/i18n/languages";
import { messagePlaceholders } from "@desktop/i18n/plural";
import { MESSAGES, type MsgKey } from "../../i18n";
import { zhTW } from "../../i18n/zh-TW";

const canon = Object.keys(zhTW).sort();
const shape = (text: string) => {
  const r = messagePlaceholders(text);
  return "error" in r ? `語法錯：${r.error}` : [...new Set(r.names)].sort().join(",");
};

describe("party dictionaries", () => {
  it("cover exactly the ten desktop languages", () => {
    expect(Object.keys(MESSAGES).sort()).toEqual([...LANGS].sort());
  });

  it.each([...LANGS])("%s has every key of the canon, same placeholders, nothing blank", (lang) => {
    expect(Object.keys(MESSAGES[lang]).sort()).toEqual(canon);
    for (const key of canon as MsgKey[]) {
      expect(MESSAGES[lang][key].trim(), key).not.toBe("");
      expect(shape(MESSAGES[lang][key]), `${lang}.${key}`).toBe(shape(zhTW[key]));
    }
  });
});
