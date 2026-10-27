// zod 卡 [initvar] 收集順序的跨宿主案例（src/shared/contracts/mvu-initvar-order.json）：桌面 import/card_schema.rs 跑同一份。
import { describe, expect, it } from "vitest";
import contract from "@desktop/shared/contracts/mvu-initvar-order.json";
import { initvarEntries } from "./initvar";

describe("initvar 收集順序與桌面一致", () => {
  for (const testCase of contract.cases) {
    it(testCase.name, () => {
      const entries = initvarEntries({ character_book: { entries: testCase.entries } });
      expect(entries.map((entry) => entry.content)).toEqual(testCase.expected);
    });
  }
});
