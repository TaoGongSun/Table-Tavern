// 桌面版巨集對拍案例（src/shared/contracts/st-macros/web-macro-cases.json）：網頁版實作重跑要得到同一份預期值。
// 預期值由 web/scripts/gen-st-macro-fixtures.mjs 產生；這裡只比對，不改寫檔案。時區要與產生時相同。
process.env.TZ = "Asia/Taipei";
import { describe, expect, it } from "vitest";
import webCases from "@desktop/shared/contracts/st-macros/web-macro-cases.json";
import { FIXTURE_TIMEZONE, runMacroCase, type MacroCase, type MacroExpected } from "./macro-parity-runner";

describe("macro parity fixtures", () => {
  it("fixture timezone matches the runner", () => {
    expect(webCases.timezone).toBe(FIXTURE_TIMEZONE);
  });

  it.each((webCases.cases as unknown as (MacroCase & { expected: MacroExpected })[]).map((item) => [item.name, item] as const))("%s", (_, item) => {
    expect(runMacroCase(item)).toEqual(item.expected);
  });
});
