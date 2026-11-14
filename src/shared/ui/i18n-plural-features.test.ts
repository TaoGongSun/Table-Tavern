import { afterEach, describe, expect, it, vi } from "vitest";

// 補充字典（features/）的字串跟主字典走同一條 t()：換一份帶 plural 的 smart-free 文案驗證
vi.mock("../../i18n/features/smart-free", async (importOriginal) => {
  const original = await importOriginal<typeof import("../../i18n/features/smart-free")>();
  return {
    ...original,
    smartFreeMessage: () => "{remaining, plural, one {# call left} other {# calls left}}",
  };
});

const { setLang, t } = await import("../../i18n");

afterEach(() => setLang("zh-TW"));

describe("features 字典經 t() 展開單複數", () => {
  it("en 1／2", () => {
    setLang("en");
    expect(t("smartFreeDailyLeft", { remaining: 1 })).toBe("1 call left");
    expect(t("smartFreeDailyLeft", { remaining: 2 })).toBe("2 calls left");
  });

  it("VN 生圖的張數照單複數（實測挖到的 1 images／1 sprites）", () => {
    setLang("en");
    expect(t("vnArtEstimate", { source: "Codex CLI", n: 1, bg: 0 })).toBe("Uses Codex CLI quota: 1 sprite + 0 backgrounds.");
    expect(t("vnArtPaused", { time: "soon", n: 1 })).toContain("1 image remains and");
    expect(t("vnArtPaused", { time: "soon", n: 2 })).toContain("2 images remain and");
    setLang("ru");
    expect(t("vnArtPausedUnknown", { n: 2 })).toContain("Осталось 2 изображения");
  });
});
