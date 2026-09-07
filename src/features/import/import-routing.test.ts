import { describe, expect, it } from "vitest";
import { decideImportRoute } from "./import-routing";

describe("decideImportRoute", () => {
  it("收據為空＋世界書也空＝直接匯（不論身分）", () => {
    expect(decideImportRoute("character", [], false)).toBe("direct");
    expect(decideImportRoute("worldbook", [], false)).toBe("direct");
  });

  it("收據只有 character 筆＋世界書身分＝ask（不猜配套，讓玩家自己決定）", () => {
    expect(decideImportRoute("worldbook", ["character"], false)).toBe("ask");
    expect(decideImportRoute("worldbook", ["character"], true)).toBe("ask");
  });

  it("收據已有 worldbook 筆＋世界書身分＝提醒會合成一本", () => {
    expect(decideImportRoute("worldbook", ["worldbook"], true)).toBe("merge_worldbook");
    expect(decideImportRoute("worldbook", ["character", "worldbook"], true)).toBe("merge_worldbook");
  });

  it("收據為空但桌上已有條目＝保險生效（舊桌／手建桌／範例桌）", () => {
    expect(decideImportRoute("worldbook", [], true)).toBe("merge_worldbook");
    // 保險只管世界書：匯角色卡不會蓋掉既有條目，照舊零打擾
    expect(decideImportRoute("character", [], true)).toBe("direct");
  });

  it("第二張角色卡（身分 character，收據非空）＝ask", () => {
    expect(decideImportRoute("character", ["character"], false)).toBe("ask");
    expect(decideImportRoute("character", ["worldbook"], false)).toBe("ask");
  });
});
