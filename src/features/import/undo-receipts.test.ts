// AI 重構套用的復原屏障（refactor-undo-ai-apply）：陣容欄能不能撤、第二張卡路由看哪些收據。
import { describe, expect, it } from "vitest";
import { decideImportRoute } from "./import-routing";
import {
  canUndoLastReceipt,
  importRouteKinds,
  REFACTOR_AI_BARRIER_KIND,
  type ImportReceiptSummary,
} from "./useImportController";

const receipt = (kind: ImportReceiptSummary["kind"]): ImportReceiptSummary => ({
  kind,
  label: kind,
  timestamp: "2026-10-29T10:00:00",
});

describe("canUndoLastReceipt", () => {
  it("沒有收據、或最後一筆是屏障：撤不動", () => {
    expect(canUndoLastReceipt([])).toBe(false);
    expect(canUndoLastReceipt([receipt("worldbook"), receipt(REFACTOR_AI_BARRIER_KIND)])).toBe(false);
  });

  it("屏障上面還有匯入：撤得動", () => {
    expect(
      canUndoLastReceipt([receipt("worldbook"), receipt(REFACTOR_AI_BARRIER_KIND), receipt("character")]),
    ).toBe(true);
    expect(canUndoLastReceipt([receipt("refactor")])).toBe(true);
  });
});

describe("importRouteKinds", () => {
  it("只扣掉屏障：只剩屏障的桌照沒匯過走 direct", () => {
    const kinds = importRouteKinds([receipt(REFACTOR_AI_BARRIER_KIND)]);
    expect(kinds).toEqual([]);
    expect(decideImportRoute("character", kinds, false)).toBe("direct");
  });

  it("匯入重構卡的 refactor 收據是真匯入，照算：照舊 ask", () => {
    const kinds = importRouteKinds([receipt("refactor"), receipt(REFACTOR_AI_BARRIER_KIND)]);
    expect(kinds).toEqual(["refactor"]);
    expect(decideImportRoute("character", kinds, false)).toBe("ask");
    expect(decideImportRoute("worldbook", kinds, false)).toBe("ask");
  });
});
