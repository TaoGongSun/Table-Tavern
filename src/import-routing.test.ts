import { describe, expect, it } from "vitest";
import { decideImportRoute } from "./import-routing";

describe("decideImportRoute", () => {
  it("收據為空＋世界書也空＝直接匯（不論身分）", () => {
    expect(decideImportRoute("character", false, [], false)).toBe("direct");
    expect(decideImportRoute("worldbook", true, [], false)).toBe("direct");
  });

  it("收據只有 character 筆＋純世界書檔＝companion（配套世界書零打擾）", () => {
    expect(decideImportRoute("worldbook", true, ["character"], false)).toBe("companion");
    // 收據說了話就聽收據：那張角色卡帶進來的條目不會把配套世界書變成要問
    expect(decideImportRoute("worldbook", true, ["character"], true)).toBe("companion");
  });

  it("收據只有 character 筆＋世界書卡身分（非純世界書檔）＝ask", () => {
    expect(decideImportRoute("worldbook", false, ["character"], false)).toBe("ask");
  });

  it("收據已有 worldbook 筆＋世界書身分＝提醒會合成一本", () => {
    expect(decideImportRoute("worldbook", false, ["worldbook"], true)).toBe("merge_worldbook");
    // 純世界書檔也一樣要問：companion 零打擾只在桌上還沒有 worldbook 收據時才成立
    expect(decideImportRoute("worldbook", true, ["character", "worldbook"], true)).toBe("merge_worldbook");
  });

  it("收據為空但桌上已有條目＝保險生效（舊桌／手建桌／範例桌）", () => {
    expect(decideImportRoute("worldbook", true, [], true)).toBe("merge_worldbook");
    // 保險只管世界書：匯角色卡不會蓋掉既有條目，照舊零打擾
    expect(decideImportRoute("character", false, [], true)).toBe("direct");
  });

  it("第二張角色卡（身分 character，收據非空）＝ask", () => {
    expect(decideImportRoute("character", false, ["character"], false)).toBe("ask");
    expect(decideImportRoute("character", false, ["worldbook"], false)).toBe("ask");
  });
});
