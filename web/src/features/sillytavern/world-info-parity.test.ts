// 桌面版對拍案例（src/shared/contracts/world-info/）：網頁版實作重跑要得到同一份預期值。
// 預期值由 web/scripts/gen-world-info-fixtures.mjs 產生；這裡只比對，不改寫檔案。
import { describe, expect, it } from "vitest";
import entryCases from "@desktop/shared/contracts/world-info/entry-cases.json";
import regexCases from "@desktop/shared/contracts/world-info/regex-cases.json";
import scanCases from "@desktop/shared/contracts/world-info/scan-cases.json";
import sortCases from "@desktop/shared/contracts/world-info/sort-cases.json";
import { runEntryCase, runRegexCase, runScanCase, runSortCase, type EntryCase, type RegexCase, type ScanCase, type SortCase } from "./world-info-parity-runner";

type WithExpected<T> = T & { expected: unknown };

/** JSON 來回一次再比（undefined 欄位、數字寫法與檔案一致）。 */
const asJson = (value: unknown) => JSON.parse(JSON.stringify(value));

describe("world info parity fixtures", () => {
  it.each((scanCases.cases as unknown as WithExpected<ScanCase>[]).map((item) => [item.name, item] as const))("scan: %s", (_, item) => {
    expect(asJson(runScanCase(item))).toEqual(item.expected);
  });

  it.each((regexCases.cases as unknown as WithExpected<RegexCase>[]).map((item) => [item.name, item] as const))("regex: %s", (_, item) => {
    expect(runRegexCase(item)).toEqual(item.expected);
  });

  it.each((sortCases.cases as unknown as WithExpected<SortCase>[]).map((item) => [item.name, item] as const))("sort: %s", (_, item) => {
    expect(runSortCase(item)).toEqual(item.expected);
  });

  it.each((entryCases.cases as unknown as WithExpected<EntryCase>[]).map((item) => [item.name, item] as const))("entry: %s", (_, item) => {
    expect(asJson(runEntryCase(item))).toEqual(item.expected);
  });
});
