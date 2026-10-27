// zod 卡的引擎（mvu-zod-semantics 規格 1、4 節）：共用核心（virtual:mvu-engine，建置時包成模組，不必在執行時求值字串）
// 加真 zod，值解析在同一個 realm 同步跑（同桌面 QuickJS 入口）。隔離 Worker（worker-entry.ts）與 Node 測試共用。
import lodash from "lodash";
import { z } from "zod";
import { createMvuCore } from "virtual:mvu-engine";
import { MVU_EVENTS, type MvuDetailedResult, type ZodInitOutcome } from "@desktop/features/mvu-engine/host-contract";
import { isEvalOp, runEval } from "@desktop/features/mvu-engine/value-parser";

// 隔離 Worker 的 CSP 不准 eval：關掉 zod 的 JIT（不關就會先試 new Function、留下 CSP 違規），輸出與 JIT 相同
z.config({ jitless: true });

export type Macros = { user: string; char: string | null } | null;

export interface ZodEngine {
  /** 墊片模組（卡片 import 的 mvu_zod）轉呼叫：只認第一份 */
  register: (input: unknown) => void;
  status: () => { registered: boolean; duplicates: number };
  parse: (message: string, old: Record<string, unknown>, macros: Macros) => Promise<MvuDetailedResult>;
  /** 開局：先 cleanUpMetadata 再過 schema（規格 1.3） */
  initialize: (stat: unknown) => { outcome: ZodInitOutcome; stat_data: unknown };
}

const utf8Length = (text: string) => new TextEncoder().encode(text).length;

export function createZodEngine(): ZodEngine {
  let macros: Macros = null;
  const core = createMvuCore({
    z,
    lodash,
    events: MVU_EVENTS,
    emit: async () => {},
    host: {
      evaluate: (op, text, done) => {
        done(isEvalOp(op) && typeof text === "string" ? runEval(op, text) : { ok: false, error: "bad-request" });
        return null;
      },
      utf8Length,
      macros: () => macros,
    },
  });
  return {
    register: (input) => core.registerMvuSchema(input),
    status: () => core.zodStatus(),
    async parse(message, old, current) {
      macros = current;
      try {
        return await core.parseMessageDetailed(message, old);
      } finally {
        macros = null;
      }
    },
    initialize(stat) {
      core.cleanUpMetadata(stat);
      const variables: Record<string, unknown> = { stat_data: stat };
      const outcome = core.zodInitialize(variables);
      return { outcome, stat_data: variables.stat_data };
    },
  };
}

/** 卡片腳本可用的全域：z、_、`$(fn)`（上游是 jQuery 的頁面就緒，這裡沒有頁面，直接執行）、安靜的 console */
export function cardGlobals(): Record<string, unknown> {
  const quiet = () => {};
  return {
    z,
    _: lodash,
    $: (fn: unknown) => {
      if (typeof fn === "function") fn();
    },
    console: { log: quiet, info: quiet, warn: quiet, error: quiet, debug: quiet },
  };
}
