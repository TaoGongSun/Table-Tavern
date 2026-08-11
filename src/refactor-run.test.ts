import { describe, expect, it } from "vitest";
import { isRateLimitError, runRefactorCalls, withRateLimitRetry } from "./refactor-run";

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe("runRefactorCalls", () => {
  it("T1 上限：pool 8 項、limit 4，同時在途數峰值 ≤4 且 >1", async () => {
    let active = 0;
    let peak = 0;
    const run = async () => {
      active++;
      peak = Math.max(peak, active);
      await new Promise((resolve) => setTimeout(resolve, 5));
      active--;
    };
    const pool = Array.from({ length: 8 }, (_, i) => i);
    // chain 塞一個無害的先行項，讓 pool 8 項從頭就整批送進工人迴圈（不被「pool[0] 先行」規則吃掉一項）。
    await runRefactorCalls({ chain: [-1], pool, limit: 4, isCancelled: () => false, run });
    expect(peak).toBeGreaterThan(1);
    expect(peak).toBeLessThanOrEqual(4);
  });

  it("T2 首發 gate（chain 非空）：chain[0] resolve 前 pool 零發出，resolve 後 pool 全開且 chain[1] 接續", async () => {
    const events: string[] = [];
    const gate = deferred<void>();
    const run = async (item: string) => {
      events.push(`start:${item}`);
      if (item === "c0") await gate.promise;
      events.push(`end:${item}`);
    };
    const done = runRefactorCalls({
      chain: ["c0", "c1", "c2"],
      pool: ["p0", "p1", "p2", "p3"],
      limit: 4,
      isCancelled: () => false,
      run,
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(events).toEqual(["start:c0"]);

    gate.resolve();
    await done;

    const gateEnd = events.indexOf("end:c0");
    expect(gateEnd).toBeGreaterThanOrEqual(0);
    for (const item of ["p0", "p1", "p2", "p3", "c1"]) {
      expect(events.indexOf(`start:${item}`)).toBeGreaterThan(gateEnd);
    }
    // 鏈上順序不變：c2 要等 c1 結束才開始。
    expect(events.indexOf("start:c2")).toBeGreaterThan(events.indexOf("end:c1"));
  });

  it("T2 首發 gate（chain 空）：pool[0] 先行，resolve 前其餘零發出", async () => {
    const events: string[] = [];
    const gate = deferred<void>();
    const run = async (item: string) => {
      events.push(`start:${item}`);
      if (item === "p0") await gate.promise;
      events.push(`end:${item}`);
    };
    const done = runRefactorCalls({
      chain: [],
      pool: ["p0", "p1", "p2", "p3"],
      limit: 4,
      isCancelled: () => false,
      run,
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(events).toEqual(["start:p0"]);

    gate.resolve();
    await done;

    for (const item of ["p1", "p2", "p3"]) {
      expect(events).toContain(`start:${item}`);
    }
  });

  it("T3 取消不發新：第 1 項 settle 後 isCancelled 轉 true，未發出的項目永不執行，Promise 正常 resolve", async () => {
    const started: string[] = [];
    let cancelled = false;
    const run = async (item: string) => {
      started.push(item);
      if (item === "c0") cancelled = true;
    };
    await runRefactorCalls({
      chain: ["c0", "c1"],
      pool: ["p0", "p1", "p2"],
      limit: 4,
      isCancelled: () => cancelled,
      run,
    });
    expect(started).toEqual(["c0"]);
  });

  it("T3b 並行階段取消：已在途的項目跑完，worker 迴圈不再取下一項", async () => {
    const started: string[] = [];
    let cancelled = false;
    const run = async (item: string) => {
      started.push(item);
      if (item === "p0") cancelled = true;
      await new Promise((resolve) => setTimeout(resolve, 1));
    };
    await runRefactorCalls({
      chain: [],
      pool: ["p0", "p1", "p2", "p3", "p4"],
      limit: 1,
      isCancelled: () => cancelled,
      run,
    });
    // chain 空＝pool[0] 先行；它一結束就置換取消旗標，後續 worker 迴圈永遠取不到下一項。
    expect(started).toEqual(["p0"]);
  });
});

describe("isRateLimitError", () => {
  it("命中 429／rate limit／overloaded（大小寫、底線變體皆算）", () => {
    expect(isRateLimitError("429 Too Many Requests")).toBe(true);
    expect(isRateLimitError("Rate Limit exceeded")).toBe(true);
    expect(isRateLimitError("rate_limit")).toBe(true);
    expect(isRateLimitError("model overloaded, try again")).toBe(true);
  });

  it("其餘錯誤訊息不算限流", () => {
    expect(isRateLimitError("invalid api key")).toBe(false);
    expect(isRateLimitError("network error")).toBe(false);
  });
});

describe("withRateLimitRetry", () => {
  it("T4 限流訊息失敗一次→等待後重試成功", async () => {
    let calls = 0;
    const fn = async () => {
      calls++;
      if (calls === 1) throw new Error("429 rate limited");
      return "ok";
    };
    const result = await withRateLimitRetry(fn, () => false, 0);
    expect(result).toBe("ok");
    expect(calls).toBe(2);
  });

  it("T4 非限流錯誤不重試，原樣 throw", async () => {
    let calls = 0;
    const fn = async () => {
      calls++;
      throw new Error("500 internal error");
    };
    await expect(withRateLimitRetry(fn, () => false, 0)).rejects.toThrow("500 internal error");
    expect(calls).toBe(1);
  });

  it("T4 已取消不重試，原樣 throw", async () => {
    let calls = 0;
    const fn = async () => {
      calls++;
      throw new Error("429 rate limited");
    };
    await expect(withRateLimitRetry(fn, () => true, 0)).rejects.toThrow("429 rate limited");
    expect(calls).toBe(1);
  });

  it("T4 重試後第二次仍失敗，原樣 throw 不再重試", async () => {
    let calls = 0;
    const fn = async () => {
      calls++;
      throw new Error("429 rate limited");
    };
    await expect(withRateLimitRetry(fn, () => false, 0)).rejects.toThrow("429 rate limited");
    expect(calls).toBe(2);
  });
});
