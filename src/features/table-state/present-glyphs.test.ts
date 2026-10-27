// present 顯示轉字形：OpenCC 詞組轉換＋保護字、已知人名整名鎖定、逐字折疊歸位，名字之間互不影響。
import { describe, expect, it } from "vitest";
import {
  buildNameLock,
  convertName,
  displayPresent,
  glyphTarget,
  loadGlyphEngine,
  splitPresentNames,
  type GlyphEngine,
} from "./present-glyphs";

describe("glyphTarget", () => {
  it("繁中轉繁、簡中轉簡、其他語系不轉", () => {
    expect(glyphTarget("zh-TW")).toBe("tw");
    expect(glyphTarget("zh-HK")).toBe("hk");
    expect(glyphTarget("zh-CN")).toBe("cn");
    for (const lang of ["en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"]) expect(glyphTarget(lang)).toBeNull();
  });
});

describe("splitPresentNames", () => {
  it("與後端同一組分隔符", () => {
    expect(splitPresentNames(" 林远、贺铮，铁三,秦岚／慧明/阿古拉；周掌柜; ")).toEqual([
      "林远", "贺铮", "铁三", "秦岚", "慧明", "阿古拉", "周掌柜",
    ]);
  });
});

describe("convertName（真 OpenCC）", () => {
  it("簡→繁：常見人名與一對多保護字", async () => {
    const tw = await loadGlyphEngine("tw");
    const cases: [string, string][] = [
      ["贺铮", "賀錚"], ["周掌柜", "周掌櫃"], ["余华", "余華"], ["沈郁", "沈郁"],
      ["于谦", "于謙"], ["范冰冰", "范冰冰"], ["铁三", "鐵三"], ["秦岚", "秦嵐"], ["阿古拉", "阿古拉"],
    ];
    for (const [from, to] of cases) expect(convertName(from, tw)).toBe(to);
    expect(convertName("周掌柜", await loadGlyphEngine("hk"))).toBe("周掌櫃");
  });

  it("繁→簡：沈鬱顯示沈郁，不得出現「沉」", async () => {
    const cn = await loadGlyphEngine("cn");
    expect(convertName("沈鬱", cn)).toBe("沈郁");
    expect(convertName("賀錚", cn)).toBe("贺铮");
    expect(convertName("周掌櫃", cn)).toBe("周掌柜");
  });
});

describe("displayPresent", () => {
  it("已知人名鎖定字形；模型半轉的名字逐字折疊後唯一對上就歸位；無條目的人照轉", async () => {
    const tw = await loadGlyphEngine("tw");
    const lock = buildNameLock(["林远", "贺铮", "周掌柜", "阿古拉"], tw);
    expect(displayPresent("林遠、周掌柜、周掌櫃、商队领头", lock, tw)).toBe("林遠、周掌櫃、周掌櫃、商隊領頭");
  });

  it("折疊後對上多個已知人名（沈郁／沈鬱並存）不歸位；同名卡與條目只算一個候選", async () => {
    const tw = await loadGlyphEngine("tw");
    const both = buildNameLock(["沈郁", "沈鬱"], tw);
    expect(displayPresent("沈郁、沈鬱、瀋鬱", both, tw)).toBe("沈郁、沈鬱、瀋鬱");
    const dup = buildNameLock(["铁三叔", "铁三叔"], tw);
    expect(displayPresent("铁三叔", dup, tw)).toBe("鐵三叔");
  });

  it("歸位是整名相等：短名不吃長名", async () => {
    const tw = await loadGlyphEngine("tw");
    const lock = buildNameLock(["沈", "沈郁"], tw);
    expect(displayPresent("沈郁", lock, tw)).toBe("沈郁");
    expect(displayPresent("亚历山大", buildNameLock(["亚历山大・冯・史特劳斯"], tw), tw)).toBe("亞歷山大");
  });

  it("某個名字轉換前後字數不同只保留它的原字，其他名字照轉", () => {
    const fake: GlyphEngine = {
      target: "tw",
      convert: (text) => (text === "怪名" ? "怪名字" : text.replace("贺", "賀").replace("铮", "錚")),
      fold: (text) => text,
    };
    expect(displayPresent("怪名、贺铮", new Map(), fake)).toBe("怪名、賀錚");
  });

  it("空名單原樣", async () => {
    const tw = await loadGlyphEngine("tw");
    expect(displayPresent("", new Map(), tw)).toBe("");
  });
});
