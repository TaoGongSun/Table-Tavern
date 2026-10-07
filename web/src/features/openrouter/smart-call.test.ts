import { describe, expect, it } from "vitest";
import { failureFromHttp, failureStalled, failureStream, type ApiFailure } from "./api-failure";
import { FailoverRuntime } from "./failover";
import { isDailyExhausted, runSmartCall, type CallEnv, type CallPlan } from "./smart-call";
import type { StreamResult } from "./stream-chat";

const plan: CallPlan = {
  account: "acct",
  lineup: ["m1", "m2", "m3"],
  others: [],
  fits: (model) => ["m1", "m2", "m3"].includes(model),
  names: new Map([
    ["m1", "Model One"],
    ["m2", "Model Two"],
  ]),
};

const busy = (emittedText = false): ApiFailure => ({
  ...failureFromHttp(503, new Headers(), '{"error":{"code":503,"message":"busy"}}'),
  emittedText,
});

function env(results: StreamResult[], extra: Partial<CallEnv> = {}) {
  const sent: string[] = [];
  const controller = new AbortController();
  const value: CallEnv = {
    signal: controller.signal,
    now: () => 1_000,
    dailyRemaining: async () => 10,
    send: async (model) => {
      sent.push(model);
      return results.shift() ?? { kind: "ok", text: "fallback", model, truncated: null };
    },
    ...extra,
  };
  return { value, sent, controller };
}

const failed = (failure: ApiFailure): StreamResult => ({ kind: "failed", failure });

describe("runSmartCall", () => {
  it("counts the first failure without switching", async () => {
    const runtime = new FailoverRuntime();
    const { value, sent } = env([failed(busy())]);
    const outcome = await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1"]);
    expect(outcome).toMatchObject({ kind: "error", failover: null });
    expect(outcome.kind === "error" && outcome.display).toMatch(/^AI_FREE_MODEL_BUSY:/);
  });

  it("switches on the second failure and retries once when no text was emitted", async () => {
    const runtime = new FailoverRuntime();
    await runSmartCall(plan, runtime, env([failed(busy())]).value);
    const { value, sent } = env([failed(busy()), { kind: "ok", text: "好", model: "m2", truncated: null }]);
    const outcome = await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1", "m2"]);
    expect(outcome).toMatchObject({ kind: "ok", text: "好", failover: { from: "Model One", to: "Model Two", retried: true } });
    expect(runtime.model).toBe("m2");
  });

  it("switches but does not resend once text was emitted", async () => {
    const runtime = new FailoverRuntime();
    await runSmartCall(plan, runtime, env([failed(busy())]).value);
    const { value, sent } = env([failed(failureStalled(120, true))]);
    const outcome = await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1"]);
    expect(outcome).toMatchObject({ kind: "error", failover: { retried: false } });
    expect(outcome.kind === "error" && outcome.display).toMatch(/^AI_STREAM_STALLED:/);
  });

  it("does not send a second shot after the player stopped", async () => {
    const runtime = new FailoverRuntime();
    runtime.reconcile("acct", plan.lineup, 0);
    runtime.count = 1;
    const { value, sent, controller } = env([]);
    value.send = async (model) => {
      sent.push(model);
      controller.abort();
      return failed(busy());
    };
    const outcome = await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1"]);
    expect(outcome).toMatchObject({ kind: "error", failover: { retried: false } });
  });

  it("switches immediately when the model is gone (404)", async () => {
    const runtime = new FailoverRuntime();
    const gone = failureFromHttp(404, new Headers(), "{}");
    const { value, sent } = env([failed(gone), { kind: "ok", text: "ok", model: "m2", truncated: null }]);
    await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1", "m2"]);
  });

  it("treats a 429 confirmed by /key as the daily limit, not a model problem", async () => {
    const runtime = new FailoverRuntime();
    const limited = failureFromHttp(429, new Headers(), '{"error":{"code":429,"message":"Rate limit"}}');
    const { value, sent } = env([failed(limited)], { dailyRemaining: async () => 0 });
    const outcome = await runSmartCall(plan, runtime, value);
    expect(sent).toEqual(["m1"]);
    expect(isDailyExhausted(outcome)).toBe(true);
  });

  it("daily 25 left + per-minute 429: not counted as daily exhausted", async () => {
    const perMinute = failureFromHttp(
      429,
      new Headers({ "x-ratelimit-remaining": "0" }),
      '{"error":{"code":429,"message":"Rate limit exceeded: free-models-per-min."}}',
    );
    const { value } = env([failed(perMinute)], { dailyRemaining: async () => 25 });
    const outcome = await runSmartCall(plan, new FailoverRuntime(), value);
    expect(outcome).toMatchObject({ kind: "error", cls: "account", daily: false });
    expect(isDailyExhausted(outcome)).toBe(false);
  });

  it("daily 25 left + 429 without platform evidence: not counted as daily exhausted", async () => {
    const limited = failureFromHttp(429, new Headers(), '{"error":{"code":429,"message":"Rate limit"}}');
    const { value } = env([failed(limited)], { dailyRemaining: async () => 25 });
    const outcome = await runSmartCall(plan, new FailoverRuntime(), value);
    expect(isDailyExhausted(outcome)).toBe(false);
  });

  it("platform says free-models-per-day: counted as daily exhausted", async () => {
    const perDay = failureFromHttp(429, new Headers(), '{"error":{"code":429,"message":"Rate limit exceeded: free-models-per-day."}}');
    const outcome = await runSmartCall(plan, new FailoverRuntime(), env([failed(perDay)]).value);
    expect(isDailyExhausted(outcome)).toBe(true);
  });

  it("ignores content problems for switching", async () => {
    const runtime = new FailoverRuntime();
    for (let round = 0; round < 3; round += 1) {
      await runSmartCall(plan, runtime, env([failed(failureStream("AI_EMPTY_RESPONSE: x", null, false))]).value);
    }
    expect(runtime.model).toBe("m1");
    expect(runtime.count).toBe(0);
  });

  it("reports all-busy when every lineup model is exhausted", async () => {
    const small: CallPlan = { ...plan, lineup: ["m1"], fits: (model) => model === "m1" };
    const runtime = new FailoverRuntime();
    await runSmartCall(small, runtime, env([failed(busy())]).value);
    const outcome = await runSmartCall(small, runtime, env([failed(busy())]).value);
    expect(outcome.kind === "error" && outcome.display).toMatch(/^AI_FREE_ALL_BUSY:/);
  });

  it("drops late results after logout (ticket epoch changed)", async () => {
    const runtime = new FailoverRuntime();
    await runSmartCall(plan, runtime, env([failed(busy())]).value);
    const { value } = env([]);
    value.send = async () => {
      runtime.invalidate();
      return failed(busy());
    };
    await runSmartCall(plan, runtime, value);
    expect(runtime.model).toBe("m1");
  });
});
