// 穩定免費的一次邏輯呼叫，最多派送兩發。照桌面版 src-tauri/src/smart_free/call.rs:115 改寫：
// 第 1 發失敗讓換模成立、且還沒吐出任何正文時，才用新模型重送一次（已吐正文不重送
// 〔作者裁決 2026-10-04〕，call.rs:155-160）；被取消就不派第二發。不試打（D9）。
import {
  classifyFailure,
  dailyEvidence,
  needsKeyCheck,
  RUNAWAY_CODE,
  STALLED_CODE,
  type ApiFailure,
  type FailureClass,
  type KeyCheck,
} from "./api-failure";
import { pickForSentence, type FailoverRuntime, type Ticket } from "./failover";
import type { StreamResult } from "./stream-chat";

/** 這一句的選模素材。 */
export interface CallPlan {
  account: string;
  lineup: string[];
  /** 名單外的其他穩定候選（依名次）。 */
  others: string[];
  /** 放得下本句的模型。 */
  fits: Set<string>;
  names: Map<string, string>;
}

export interface CallEnv {
  send: (model: string) => Promise<StreamResult>;
  /** `/key` 的今日免費剩餘；查不到回 null。 */
  dailyRemaining: () => Promise<number | null>;
  signal: AbortSignal;
  now: () => number;
}

export interface FailoverNotice {
  from: string;
  to: string;
  /** 這一句已用新模型重送。 */
  retried: boolean;
}

export type CallOutcome =
  | { kind: "ok"; text: string; model: string | null; truncated: string | null; failover: FailoverNotice | null }
  | { kind: "aborted"; text: string; failover: FailoverNotice | null }
  | {
      kind: "error";
      display: string;
      failure: ApiFailure | null;
      cls: FailureClass | null;
      /** 今日免費次數用完（平台明說每日上限或 /key 剩 0）；每分鐘限流不算。 */
      daily: boolean;
      failover: FailoverNotice | null;
    };

export const NO_FREE_MODEL = "AI_NO_FREE_MODEL:";
export const ALL_BUSY = "AI_FREE_ALL_BUSY:";
export const FREE_MODEL_BUSY = "AI_FREE_MODEL_BUSY:";

const displayName = (plan: CallPlan, model: string) => plan.names.get(model) ?? model;

async function classify(failure: ApiFailure, env: CallEnv): Promise<{ cls: FailureClass; daily: boolean }> {
  let key: KeyCheck = { kind: "not-queried" };
  if (needsKeyCheck(failure)) {
    const remaining = await env.dailyRemaining();
    key = remaining === null ? { kind: "failed" } : { kind: "remaining", remaining };
  }
  return { cls: classifyFailure(failure, key), daily: dailyEvidence(failure, key) };
}

/** 給畫面的錯誤：模型層級掛 AI_FREE_MODEL_BUSY；自己判的停滯、失控保留原碼。 */
function display(failure: ApiFailure, cls: FailureClass): string {
  const own = [STALLED_CODE, RUNAWAY_CODE].some((code) => failure.display.startsWith(code));
  if (!own && (cls === "model" || cls === "gone")) return `${FREE_MODEL_BUSY} ${failure.display}`;
  return failure.display;
}

export async function runSmartCall(plan: CallPlan, runtime: FailoverRuntime, env: CallEnv): Promise<CallOutcome> {
  const fits = (model: string) => plan.fits.has(model);
  const current = runtime.reconcile(plan.account, plan.lineup, env.now());
  const firstPick = pickForSentence(current, plan.lineup, plan.others, fits, []);
  if (!firstPick) return { kind: "error", display: NO_FREE_MODEL, failure: null, cls: null, daily: false, failover: null };
  const first: Ticket = runtime.ticket(current, firstPick.model, firstPick.substitute);

  const result = await env.send(first.selectedModel);
  if (result.kind === "ok") {
    runtime.recordChat(first, null, true, plan.lineup, env.now());
    return { ...result, failover: null };
  }
  if (result.kind === "aborted") return { kind: "aborted", text: result.text, failover: null };

  const failure = result.failure;
  const { cls, daily } = await classify(failure, env);
  const record = runtime.recordChat(first, cls, true, plan.lineup, env.now());
  if (record.kind === "all-busy") {
    return { kind: "error", display: `${ALL_BUSY} ${failure.display}`, failure, cls, daily, failover: null };
  }
  if (record.kind !== "switched") return { kind: "error", display: display(failure, cls), failure, cls, daily, failover: null };

  // 換模已成立：沒吐過正文、也沒被取消，才派第二發
  const retryPick =
    !failure.emittedText && !env.signal.aborted
      ? pickForSentence(runtime.model, plan.lineup, plan.others, fits, [first.selectedModel, ...runtime.exhausted])
      : null;
  const notice: FailoverNotice = {
    from: displayName(plan, record.from),
    to: displayName(plan, record.to),
    retried: retryPick !== null,
  };
  if (!retryPick) return { kind: "error", display: display(failure, cls), failure, cls, daily, failover: notice };

  const second = runtime.ticket(runtime.model, retryPick.model, retryPick.substitute);
  const retried = await env.send(second.selectedModel);
  if (retried.kind === "ok") {
    runtime.recordChat(second, null, false, plan.lineup, env.now());
    return { ...retried, failover: notice };
  }
  if (retried.kind === "aborted") return { kind: "aborted", text: retried.text, failover: notice };
  const retryResult = await classify(retried.failure, env);
  runtime.recordChat(second, retryResult.cls, false, plan.lineup, env.now());
  return {
    kind: "error",
    display: display(retried.failure, retryResult.cls),
    failure: retried.failure,
    cls: retryResult.cls,
    daily: retryResult.daily,
    failover: notice,
  };
}

/** 這個錯誤代表今日免費次數用完：只認每日證據，每分鐘限流不算。 */
export const isDailyExhausted = (outcome: CallOutcome) => outcome.kind === "error" && outcome.daily;
