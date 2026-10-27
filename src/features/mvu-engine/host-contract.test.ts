// 宿主契約（host-contract.ts）：直接呼叫 createMvuCore，驗計時器有給／沒給、同步／非同步回覆、宿主不回應與拋錯。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { buildMvuEngineSource } from "./engine-source";
import type { EvalReply, MvuCore, MvuHost } from "./host-contract";
import { runEval } from "./value-parser";

const lodashName = "lodash";
const lodash: unknown = ((await import(/* @vite-ignore */ lodashName)) as { default: unknown }).default;

const createMvuCore = new Function(`${buildMvuEngineSource()}\nreturn createMvuCore;`)() as (env: unknown) => MvuCore;

beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "error").mockImplementation(() => {});
});

type Evaluate = MvuHost["evaluate"];
const syncEval: Evaluate = (op, text, done) => {
  done(runEval(op, text));
  return null;
};
const asyncEval: Evaluate = (op, text, done) => {
  setTimeout(() => done(runEval(op, text)), 0);
  return null;
};

/** 手動計時器：記下保險，測試自己決定要不要觸發 */
function manualTimers() {
  const pending = new Map<number, () => void>();
  const cleared: unknown[] = [];
  let next = 0;
  return {
    pending,
    cleared,
    timers: {
      set: (fn: () => void) => {
        next += 1;
        pending.set(next, fn);
        return next;
      },
      clear: (id: unknown) => {
        cleared.push(id);
        pending.delete(id as number);
      },
    },
    fireAll: () => [...pending.values()].forEach((fn) => fn()),
  };
}

function core(evaluate: Evaluate, timers?: MvuHost["timers"]): MvuCore {
  const host: MvuHost = {
    evaluate,
    utf8Length: (text) => new TextEncoder().encode(text).length,
    macros: () => ({ user: "玩家", char: null }),
    ...(timers ? { timers } : {}),
  };
  return createMvuCore({ host, emit: async () => {}, events: {}, lodash });
}

const old = () => ({ stat_data: { 金: 10, 名: "阿明" } });
const MESSAGE = "_.set('金', 2 * 3);_.set('名', '小明');";

describe("宿主契約：計時器有給／沒給 × 同步／非同步回覆", () => {
  for (const [label, evaluate] of [
    ["同步", syncEval],
    ["非同步", asyncEval],
  ] as const) {
    it(`沒給計時器、${label}回覆：照常套用`, async () => {
      const result = await core(evaluate).parseMessage(MESSAGE, old());
      expect(result.stat_data).toEqual({ 金: 6, 名: "小明" });
    });

    it(`有給計時器、${label}回覆：照常套用，每筆保險都收掉`, async () => {
      const manual = manualTimers();
      const result = await core(evaluate, manual.timers).parseMessage(MESSAGE, old());
      expect(result.stat_data).toEqual({ 金: 6, 名: "小明" });
      expect(manual.cleared).toEqual([1, 2]);
      expect(manual.pending.size).toBe(0);
    });
  }

  it("宿主回錯：那一條失敗、其餘照套", async () => {
    let calls = 0;
    const evaluate: Evaluate = (op, text, done) => {
      calls += 1;
      done(calls === 1 ? ({ ok: false, error: "timeout" } as EvalReply) : runEval(op, text));
      return null;
    };
    const result = await core(evaluate).parseMessage(MESSAGE, old());
    expect(result.stat_data).toEqual({ 金: 10, 名: "小明" });
  });
});

describe("宿主契約：不回應與拋錯", () => {
  it("有計時器、宿主不回：保險觸發就整次以 eval-timeout 失敗、呼叫取消函式，晚到的結果不認", async () => {
    const manual = manualTimers();
    const cancel = vi.fn();
    let late: ((reply: EvalReply) => void) | null = null;
    const pending = core((_op, _text, done) => {
      late = done;
      return cancel;
    }, manual.timers).parseMessage(MESSAGE, old());
    await vi.waitFor(() => expect(manual.pending.size).toBe(1));
    manual.fireAll();
    await expect(pending).rejects.toMatchObject({ code: "eval-timeout" });
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(() => late!({ ok: true, value: 1 })).not.toThrow();
  });

  it("取消函式拋錯：照樣以 eval-timeout 結算", async () => {
    const manual = manualTimers();
    const pending = core(
      () => () => {
        throw new Error("cancel 壞了");
      },
      manual.timers,
    ).parseMessage(MESSAGE, old());
    await vi.waitFor(() => expect(manual.pending.size).toBe(1));
    expect(() => manual.fireAll()).toThrow("cancel 壞了");
    await expect(pending).rejects.toMatchObject({ code: "eval-timeout" });
  });

  it("送請求時拋錯：整次照拋，保險收掉", async () => {
    const manual = manualTimers();
    const pending = core(() => {
      throw new Error("中斷");
    }, manual.timers).parseMessage(MESSAGE, old());
    await expect(pending).rejects.toThrow("中斷");
    expect(manual.pending.size).toBe(0);
    await expect(
      core(() => {
        throw new Error("中斷");
      }).parseMessage(MESSAGE, old()),
    ).rejects.toThrow("中斷");
  });
});
