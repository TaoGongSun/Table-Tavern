// 宿主端的值解析 Worker 管理（計畫 8.9）：一次只跑一筆，單筆 200 ms 逾時就 terminate 該 Worker、回錯、
// 下一筆再建新的（mathjs 重載不浪費在沒人用的時候）。沙盒送來的請求經 `handleEvalMessage` 轉進來。
import { type EvalOp } from "./card-mvu-parse-engine";

export type EvalOutcome = { ok: true; value: unknown } | { ok: false; error: string };

/** Worker 的最小介面（測試用假的替身） */
export interface EvalWorkerLike {
  postMessage: (message: unknown) => void;
  terminate: () => void;
  addEventListener: (type: "message" | "error", listener: (event: { data?: unknown }) => void) => void;
}

export interface EvalHost {
  run: (op: EvalOp, text: string) => Promise<EvalOutcome>;
  /** 關掉：終止 Worker、未完成的請求回 `closed` */
  dispose: () => void;
}

export const EVAL_TIMEOUT_MS = 200;
const BOOT_TIMEOUT_MS = 10_000;

/** 預設的 Worker：Vite 把 worker 檔與 mathjs 打成獨立 chunk，宿主第一次要求值才載入 */
export function createEvalWorker(): EvalWorkerLike {
  return new Worker(new URL("./card-mvu-eval.worker.ts", import.meta.url), { type: "module" });
}

export function createEvalHost(options: {
  createWorker?: () => EvalWorkerLike;
  timeoutMs?: number;
  bootMs?: number;
} = {}): EvalHost {
  const createWorker = options.createWorker ?? createEvalWorker;
  const timeoutMs = options.timeoutMs ?? EVAL_TIMEOUT_MS;
  const bootMs = options.bootMs ?? BOOT_TIMEOUT_MS;
  interface Job {
    id: number;
    op: EvalOp;
    text: string;
    resolve: (outcome: EvalOutcome) => void;
  }
  const queue: Job[] = [];
  let current: Job | null = null;
  let worker: EvalWorkerLike | null = null;
  let ready = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let sequence = 0;
  let closed = false;

  const clearTimer = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  // 丟掉目前這支 Worker（逾時、出錯、開機失敗）：舊 Worker 的任何晚到訊息之後都不認
  const recycle = () => {
    clearTimer();
    const old = worker;
    worker = null;
    ready = false;
    old?.terminate();
  };
  const finish = (job: Job, outcome: EvalOutcome) => {
    if (current === job) current = null;
    clearTimer();
    job.resolve(outcome);
  };
  const failAll = (error: string) => {
    const jobs = [...(current === null ? [] : [current]), ...queue.splice(0)];
    current = null;
    for (const job of jobs) job.resolve({ ok: false, error });
  };
  const pump = () => {
    if (closed || current !== null || queue.length === 0) return;
    if (worker === null) {
      let fresh: EvalWorkerLike;
      try {
        fresh = createWorker();
      } catch {
        // 建不出 Worker（例如 SecurityError）：排隊中的請求一併回錯，不懸掛；之後的請求再試
        worker = null;
        failAll("worker-create-failed");
        return;
      }
      worker = fresh;
      fresh.addEventListener("message", (event) => {
        if (worker !== fresh) return;
        const data = event.data as { type?: unknown; id?: unknown; ok?: unknown; value?: unknown; error?: unknown } | null;
        if (data === null || typeof data !== "object") return;
        if (data.type === "ready") {
          ready = true;
          clearTimer();
          pump();
          return;
        }
        if (current === null || data.id !== current.id) return;
        finish(current, data.ok === true ? { ok: true, value: data.value } : { ok: false, error: String(data.error ?? "error") });
        pump();
      });
      fresh.addEventListener("error", () => {
        if (worker !== fresh) return;
        recycle();
        failAll("worker-error");
      });
      // 開機太久（載入失敗）：整批回錯，下一筆再試
      timer = setTimeout(() => {
        recycle();
        failAll("worker-boot-timeout");
      }, bootMs);
      return;
    }
    if (!ready) return;
    const job = queue.shift()!;
    current = job;
    timer = setTimeout(() => {
      // 逾時：這支 Worker 可能卡在無窮迴圈，終止它；排在後面的下一筆會建新的
      recycle();
      finish(job, { ok: false, error: "timeout" });
      pump();
    }, timeoutMs);
    try {
      worker.postMessage({ id: job.id, op: job.op, text: job.text });
    } catch {
      recycle();
      finish(job, { ok: false, error: "worker-post-failed" });
      pump();
    }
  };

  return {
    run(op, text) {
      if (closed) return Promise.resolve({ ok: false, error: "closed" });
      return new Promise<EvalOutcome>((resolve) => {
        sequence += 1;
        queue.push({ id: sequence, op, text, resolve });
        pump();
      });
    },
    dispose() {
      if (closed) return;
      closed = true;
      recycle();
      failAll("closed");
    },
  };
}

/** 沙盒送來的 `mvu-eval` 請求：形狀不對回 null（不理）；否則交給 Worker，回覆內容由呼叫端送回沙盒 */
export function parseEvalRequest(data: Record<string, unknown>): { requestId: string; op: EvalOp; text: string } | null {
  if (typeof data.requestId !== "string" || typeof data.text !== "string") return null;
  if (data.op !== "value" && data.op !== "patch") return null;
  return { requestId: data.requestId, op: data.op, text: data.text };
}
