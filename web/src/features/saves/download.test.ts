import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { importCardBytes } from "../cards/play-card";
import { SAMPLE_CARD, SAMPLE_PLAY_CARD } from "../cards/sample-card";
import { cardFile, safeFileName } from "./download";

const CARDS = new URL("../../../../src/shared/contracts/card-view/", import.meta.url);

describe("ST 匯出附的原卡檔", () => {
  it("從 PNG 匯入的卡給原 PNG（一個位元組都不改）", () => {
    const png = new Uint8Array(readFileSync(new URL("composite.png", CARDS)));
    const imported = importCardBytes(png);
    if (!imported.ok) throw new Error(imported.error);
    const file = cardFile(imported.card);
    expect(file.name).toBe("灰燼旅店的莫拉.png");
    expect(file.type).toBe("image/png");
    expect(file.body).toEqual(png);
  });

  it("其他卡給原卡 JSON 外殼", () => {
    const file = cardFile(SAMPLE_PLAY_CARD);
    expect(file.name).toBe("瑟拉.json");
    expect(JSON.parse(file.body as string)).toEqual(SAMPLE_CARD);
  });

  it("檔名去掉作業系統不收的字元", () => {
    expect(safeFileName('a/b:c*?"<>|')).toBe("a_b_c______");
    expect(safeFileName("  ")).toBe("Table Tavern");
  });
});
