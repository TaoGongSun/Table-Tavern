import { describe, expect, it } from "vitest";
import { zh } from "../../i18n/zh-TW";
import { zhCN } from "../../i18n/zh-CN";
import { en } from "../../i18n/en";
import { ja } from "../../i18n/ja";
import { ko } from "../../i18n/ko";
import { es } from "../../i18n/es";
import { ptBR } from "../../i18n/pt-BR";
import { de } from "../../i18n/de";
import { fr } from "../../i18n/fr";
import { ru } from "../../i18n/ru";

// 原生確認窗（plugin-dialog）由 macOS 決定換行：西式引號包住的名稱兩側要有 U+2060，
// 不然 CJK 名稱後的收尾引號會被擠到下一行；fr 引號內側空白用 U+00A0。
const WJ = "\u2060";
const KEYS = [
  "deleteCharacterConfirm",
  "deleteTableConfirm",
  "renameConfirm",
  "undoLastImportConfirm",
  "worldbookDeleteConfirm",
] as const;
const DICTS = { "zh-TW": zh, "zh-CN": zhCN, en, ja, ko, es, "pt-BR": ptBR, de, fr, ru };
const OPEN = "“„«‘\"";
const CLOSE = "”“»’\"";

describe("原生確認窗：引號包住的名稱不被拆行", () => {
  it.each(Object.entries(DICTS))("%s 每個西式引號包的佔位符內側都有 word joiner", (_, dict) => {
    for (const key of KEYS) {
      const text = dict[key];
      for (const match of text.matchAll(/\{\w+\}/g)) {
        const at = match.index;
        const before = text.slice(0, at).replace(/[\u00a0\u2060]+$/, "").slice(-1);
        const after = text.slice(at + match[0].length).replace(/^[\u00a0\u2060]+/, "").slice(0, 1);
        if (!OPEN.includes(before) && !CLOSE.includes(after)) continue;
        expect(text[at - 1], `${key} ${match[0]} 前`).toBe(WJ);
        expect(text[at + match[0].length], `${key} ${match[0]} 後`).toBe(WJ);
      }
    }
  });

  it("de 刪角色／刪桌是 „\\u2060{name}\\u2060“", () => {
    expect(de.deleteCharacterConfirm).toContain(`„${WJ}{name}${WJ}“`);
    expect(de.deleteTableConfirm).toContain(`„${WJ}{name}${WJ}“`);
  });

  it("rename 重複的 {from} 兩處都有", () => {
    for (const dict of [en, de, es, ptBR, ru, ko, zhCN]) {
      expect(dict.renameConfirm.split(`${WJ}{from}${WJ}`)).toHaveLength(3);
      expect(dict.renameConfirm).toContain(`${WJ}{to}${WJ}`);
    }
  });

  it("fr 引號內側是 NBSP＋word joiner，問號前也是 NBSP", () => {
    for (const key of KEYS) {
      for (const match of fr[key].matchAll(/«(.)(.)\{(\w+)\}(.)(.)»/gu)) {
        expect([match[1], match[2], match[4], match[5]].map((c) => c.codePointAt(0))).toEqual([
          0xa0, 0x2060, 0x2060, 0xa0,
        ]);
      }
      expect(fr[key]).not.toMatch(/« /);
      expect(fr[key]).not.toMatch(/ [»?]/);
    }
    expect(fr.renameConfirm.match(/«\u00a0\u2060\{\w+\}\u2060\u00a0»/g)).toHaveLength(3);
  });
});
