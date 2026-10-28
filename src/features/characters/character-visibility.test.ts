import { describe, expect, it } from "vitest";
import { effectiveSpeaker, firstSpeakerCandidate, isCharacterHidden } from "./character-visibility";

describe("isCharacterHidden", () => {
  const base = { id: "c1", archived: false, auto_hidden: false };

  it("一般卡（沒封存也沒自動隱藏）＝主區", () => {
    expect(isCharacterHidden(base, new Set())).toBe(false);
  });

  it("手動封存＝隱藏，不管本幕有沒有出場", () => {
    expect(isCharacterHidden({ ...base, archived: true }, new Set(["c1"]))).toBe(true);
  });

  it("自動隱藏且本幕沒出場＝隱藏", () => {
    expect(isCharacterHidden({ ...base, auto_hidden: true }, new Set())).toBe(true);
  });

  it("自動隱藏但本幕已出場＝主區（劇情帶上場，立刻從隱藏區移回）", () => {
    expect(isCharacterHidden({ ...base, auto_hidden: true }, new Set(["c1"]))).toBe(false);
  });

  it("自動隱藏＋已封存＝隱藏（封存優先，不因為出場而跑進主區）", () => {
    expect(isCharacterHidden({ ...base, archived: true, auto_hidden: true }, new Set(["c1"]))).toBe(true);
  });
});

describe("舞台卡", () => {
  const base = { id: "c1", archived: false, auto_hidden: false };

  it("舞台卡留著 auto_hidden（舊桌）也在主區；封存仍優先", () => {
    expect(isCharacterHidden({ ...base, stage: true, auto_hidden: true }, new Set())).toBe(false);
    expect(isCharacterHidden({ ...base, stage: true, archived: true }, new Set())).toBe(true);
  });

  it("預設發言對象跳過舞台卡與隱藏區的卡", () => {
    const cast = [
      { ...base, id: "stage", stage: true },
      { ...base, id: "hidden", auto_hidden: true },
      { ...base, id: "fox" },
    ];
    expect(firstSpeakerCandidate(cast, new Set())?.id).toBe("fox");
    expect(firstSpeakerCandidate([cast[0]], new Set())).toBeUndefined();
  });

  it("對話流程的發言對象指著舞台卡時當沒有對象；一般角色與 GM 原樣", () => {
    const metas: Record<string, { stage?: boolean }> = { stage: { stage: true }, fox: {} };
    const metaOf = (id: string) => metas[id];
    expect(effectiveSpeaker("stage", metaOf)).toBe("");
    expect(effectiveSpeaker("fox", metaOf)).toBe("fox");
    expect(effectiveSpeaker("__gm__", metaOf)).toBe("__gm__");
  });
});
