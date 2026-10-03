// 真實 WebKit 裡跑真的值解析 Worker（npm run test:webkit，不進 verify）：開機、算式、逾時終止並重建。
import { afterEach, describe, expect, it } from "vitest";
import { createEvalHost, type EvalHost } from "./card-mvu-eval-host";

let host: EvalHost | null = null;
afterEach(() => host?.dispose());

describe("值解析 Worker（真實 WebKit）", () => {
  it("字面量、JSON5、數學式、JSONPatch 區塊都在 Worker 裡算", async () => {
    host = createEvalHost();
    expect(await host.run("value", "7 * 6")).toEqual({ ok: true, value: 42 });
    expect(await host.run("value", "{a: 1,}")).toEqual({ ok: true, value: { a: 1 } });
    expect(await host.run("value", "Math.floor(2.7)")).toEqual({ ok: true, value: 2 });
    expect(await host.run("patch", "- op: add\n  path: /a\n  value: 1")).toEqual({
      ok: true,
      value: [{ op: "add", path: "/a", value: 1 }],
    });
  });

  it("重算式超過 200 ms：該筆 timeout、Worker 被終止，下一筆由新 Worker 照常算", async () => {
    host = createEvalHost();
    expect(await host.run("value", "1+1")).toEqual({ ok: true, value: 2 });
    const started = performance.now();
    expect(await host.run("value", "inv(ones(700, 700) + identity(700))")).toEqual({ ok: false, error: "timeout" });
    expect(performance.now() - started).toBeLessThan(2000);
    expect(await host.run("value", "2+2")).toEqual({ ok: true, value: 4 });
  });
});
