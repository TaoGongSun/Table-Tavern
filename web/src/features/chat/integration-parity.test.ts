// 巨集×世界書掃描的跨引擎對拍案例（src/shared/contracts/world-info/integration-cases.json）：網頁版重跑要得到同一份預期值。
// 預期值由 web/scripts/gen-world-info-fixtures.mjs 產生；這裡只比對，不改寫檔案。
import { describe, expect, it } from "vitest";
import integrationCases from "@desktop/shared/contracts/world-info/integration-cases.json";
import { runIntegrationCase, type IntegrationCase } from "./integration-parity-runner";

type WithExpected<T> = T & { expected: unknown };

describe("macro × world info integration fixtures", () => {
  it.each((integrationCases.cases as unknown as WithExpected<IntegrationCase>[]).map((item) => [item.name, item] as const))("%s", (_, item) => {
    expect(JSON.parse(JSON.stringify(runIntegrationCase(item)))).toEqual(item.expected);
  });
});
