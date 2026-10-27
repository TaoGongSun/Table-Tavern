// 網頁版 zod 引擎（engine-core.ts：建置時包出的核心＋jitless zod）跑共用 41 案，輸出要與桌面基準
// （src/features/mvu-engine/fixtures/zod-corpus-golden.json，Node 與 QuickJS 共用）逐項相同。真瀏覽器裡另跑一次
// （web/e2e/zod-isolation.mjs）。基準另帶桌面入口給的回饋句（feedback），這裡用網頁同一份模組重算比對。
import { readFileSync } from "node:fs";
import YAML from "yaml";
import { describe, expect, it } from "vitest";
import { zodFeedbackLine } from "@desktop/features/mvu-engine/zod-feedback";
import { createTestZodRunner } from "./test-runner";

const fixture = (name: string) => readFileSync(new URL(`../../../../../src/features/mvu-engine/fixtures/${name}`, import.meta.url), "utf8");

describe("zod 引擎與桌面基準", () => {
  it("開局與 41 案逐項相同", async () => {
    const golden = JSON.parse(fixture("zod-corpus-golden.json")) as {
      init: { stat_data: Record<string, unknown> };
      cases: ({ name: string; message: string } & Record<string, unknown>)[];
    };
    const runner = createTestZodRunner(fixture("clean-zod-schema.txt"));
    const init = await runner.initialize(YAML.parse(fixture("clean-zod-initvar.yaml")) as Record<string, unknown>);
    expect(init).toEqual(golden.init);
    expect(golden.cases).toHaveLength(41);
    for (const { name, message, feedback, ...expected } of golden.cases) {
      const actual = await runner.parse(message, { stat_data: init.stat_data }, { user: "玩家", char: null });
      expect(actual, name).toEqual(expected);
      // 回饋句：網頁主頁用的同一份模組，要和桌面 QuickJS 入口帶出的一字不差
      expect(actual.results.map(zodFeedbackLine), name).toEqual(feedback);
    }
  });
});
