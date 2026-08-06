import { describe, expect, it } from "vitest";
import { decideImportRoute } from "./import-routing";

describe("decideImportRoute", () => {
  it("收據為空＝直接匯（不論身分）", () => {
    expect(decideImportRoute("character", false, [])).toBe("direct");
    expect(decideImportRoute("worldbook", true, [])).toBe("direct");
  });

  it("收據只有 character 筆＋純世界書檔＝companion（配套世界書零打擾）", () => {
    expect(decideImportRoute("worldbook", true, ["character"])).toBe("companion");
  });

  it("收據只有 character 筆＋世界書卡身分（非純世界書檔）＝ask", () => {
    expect(decideImportRoute("worldbook", false, ["character"])).toBe("ask");
  });

  it("收據已有 worldbook 筆＋世界書身分＝提醒會合成一本", () => {
    expect(decideImportRoute("worldbook", false, ["worldbook"])).toBe("merge_worldbook");
    // 純世界書檔也一樣要問：companion 零打擾只在桌上還沒有 worldbook 收據時才成立
    expect(decideImportRoute("worldbook", true, ["character", "worldbook"])).toBe("merge_worldbook");
  });

  it("第二張角色卡（身分 character，收據非空）＝ask", () => {
    expect(decideImportRoute("character", false, ["character"])).toBe("ask");
    expect(decideImportRoute("character", false, ["worldbook"])).toBe("ask");
  });
});
