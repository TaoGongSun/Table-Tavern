// 主頁這一側的 zod 引擎（mvu-zod-semantics 規格 4 節）：一桌一支。第一次派工時建不透明來源的 iframe（mvu-zod.html），
// 交引擎原始碼（含 QuickJS wasm 與 bundle）與兩個 MessagePort（Worker 用、控制用）；開機（取引擎、起 QuickJS）另給
// 10 秒。派工後單一 5 秒期限涵蓋載入卡片 schema、運算、這裡驗回傳與回成功前的最後檢查；逾時、出錯（含記憶體、堆疊）
// 就回收整支（terminate Worker，之後的回傳一律不認），下次派工（玩家下一次操作）才重建，不自動重試。同一桌依序派工、
// 不重疊。
import type { MvuDetailedResult, ZodInitOutcome } from "@desktop/features/mvu-engine/host-contract";
import { BOOT_TYPE, type ZodBootReply, type ZodReply, type Macros, type ZodRequest, type ZodWorkerFailure } from "./protocol";
import { checkDetailed, checkInitialized } from "./validate";

export const ZOD_BOOT_MS = 10_000;
export const ZOD_DEADLINE_MS = 5_000;
export const ZOD_FRAME_URL = `${import.meta.env.BASE_URL}mvu-zod.html`;

export type ZodFailureKind = "schema-load" | "deadline" | "memory" | "stack" | "script" | "boot" | "bad-output" | "closed";

/** 整次失敗：狀態不提交（呼叫端照常落正文） */
export class ZodRunError extends Error {
  constructor(
    readonly kind: ZodFailureKind,
    detail: string,
  ) {
    super(`${kind}: ${detail}`);
  }
}

export interface ZodInitialized {
  outcome: ZodInitOutcome;
  stat_data: Record<string, unknown>;
}

export interface ZodRunner {
  parse: (message: string, old: Record<string, unknown>, macros: Macros) => Promise<MvuDetailedResult>;
  /** 開局：cleanUpMetadata 再過 schema；驗證不過照原值（outcome.ok 為 false） */
  initialize: (stat: Record<string, unknown>) => Promise<ZodInitialized>;
  dispose: () => void;
}

export interface ZodRunnerOptions {
  schema: string;
  loadEngine?: () => Promise<string>;
  frameUrl?: string;
  bootMs?: number;
  deadlineMs?: number;
  now?: () => number;
}

interface Frame {
  frame: HTMLIFrameElement;
  port: MessagePort;
  control: MessagePort;
  /** 收到 Worker 的回覆或它掛了（null） */
  onReply: ((reply: ZodReply | null, detail?: string) => void) | null;
}

/** Worker 回報的失敗種類 → 主頁的種類 */
function failureKind(kind: ZodWorkerFailure): ZodFailureKind {
  return kind === "schema-load" || kind === "deadline" || kind === "memory" || kind === "stack" ? kind : "script";
}

const loadDefaultEngine = () => import("virtual:mvu-zod-engine").then((module) => module.default);

export function createZodRunner(options: ZodRunnerOptions): ZodRunner {
  const loadEngine = options.loadEngine ?? loadDefaultEngine;
  const frameUrl = options.frameUrl ?? ZOD_FRAME_URL;
  const bootMs = options.bootMs ?? ZOD_BOOT_MS;
  const deadlineMs = options.deadlineMs ?? ZOD_DEADLINE_MS;
  const now = options.now ?? (() => performance.now());
  let live: Frame | null = null;
  let closed = false;
  let sequence = 0;
  let tail: Promise<unknown> = Promise.resolve();

  /** 回收一支：叫 iframe 終止 Worker、關掉兩個 port、移除 iframe；之後它送來的任何東西都不認 */
  const retire = (target: Frame) => {
    if (live === target) live = null;
    target.onReply = null;
    target.port.onmessage = null;
    target.control.onmessage = null;
    try {
      target.control.postMessage("kill");
    } catch {
      // iframe 已不在
    }
    target.port.close();
    target.control.close();
    target.frame.remove();
  };

  /** 建 iframe、開機到 ready（剝除與失敗即停檢查都過）；卡片腳本這時還沒執行 */
  const boot = (engine: string, budget: number) =>
    new Promise<Frame>((resolve, reject) => {
      const frame = document.createElement("iframe");
      frame.setAttribute("sandbox", "allow-scripts");
      frame.setAttribute("aria-hidden", "true");
      frame.tabIndex = -1;
      frame.style.display = "none";
      frame.src = frameUrl;
      const worker = new MessageChannel();
      const control = new MessageChannel();
      const entry: Frame = { frame, port: worker.port1, control: control.port1, onReply: null };
      let settled = false;
      const timer = setTimeout(() => fail("引擎開機逾時"), budget);
      function fail(detail: string) {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        retire(entry);
        reject(new ZodRunError("boot", detail));
      }
      worker.port1.onmessage = (event: MessageEvent) => {
        const data = event.data as ZodBootReply | ZodReply;
        if (!settled) {
          if ((data as ZodBootReply).type === "ready") {
            settled = true;
            clearTimeout(timer);
            resolve(entry);
          } else if ((data as ZodBootReply).type === "boot-failed") {
            fail((data as { detail?: string }).detail ?? "boot-failed");
          }
          return;
        }
        entry.onReply?.(data as ZodReply);
      };
      control.port1.onmessage = (event: MessageEvent) => {
        const detail = String((event.data as { detail?: unknown } | null)?.detail ?? "worker-error");
        if (!settled) fail(detail);
        else entry.onReply?.(null, detail);
      };
      frame.addEventListener("load", () => {
        frame.contentWindow?.postMessage({ type: BOOT_TYPE, engine }, "*", [worker.port2, control.port2]);
      }, { once: true });
      document.body.append(frame);
    });

  const ensure = async (): Promise<Frame> => {
    if (live) return live;
    const started = now();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const engine = await new Promise<string>((resolve, reject) => {
      timer = setTimeout(() => reject(new ZodRunError("boot", "引擎下載逾時")), bootMs);
      loadEngine().then(resolve, (error: unknown) => reject(new ZodRunError("boot", String(error))));
    }).finally(() => clearTimeout(timer));
    if (closed) throw new ZodRunError("closed", "closed");
    const frame = await boot(engine, Math.max(0, bootMs - (now() - started)));
    if (closed) {
      retire(frame);
      throw new ZodRunError("closed", "closed");
    }
    live = frame;
    return frame;
  };

  /** 派工：從送出起算單一期限；逾時或出錯回收整支 */
  const dispatch = <T>(build: (id: number) => ZodRequest, accept: (json: string) => T): Promise<T> => {
    const run = async (): Promise<T> => {
      if (closed) throw new ZodRunError("closed", "closed");
      const frame = await ensure();
      const started = now();
      sequence += 1;
      const id = sequence;
      return new Promise<T>((resolve, reject) => {
        const finish = (error: ZodRunError | null, value?: T) => {
          clearTimeout(timer);
          frame.onReply = null;
          if (error) {
            retire(frame);
            reject(error);
          } else resolve(value as T);
        };
        const timer = setTimeout(() => finish(new ZodRunError("deadline", "超過 5 秒期限")), deadlineMs);
        frame.onReply = (reply, detail) => {
          if (reply === null) return finish(new ZodRunError("script", detail ?? "worker-error"));
          if (reply.id !== id) return;
          if (!reply.ok) return finish(new ZodRunError(failureKind(reply.kind), reply.detail));
          let value: T;
          try {
            value = accept(reply.json);
          } catch (error) {
            return finish(new ZodRunError("bad-output", String(error instanceof Error ? error.message : error)));
          }
          // 回成功前最後對一次期限
          if (now() - started > deadlineMs) return finish(new ZodRunError("deadline", "超過 5 秒期限"));
          finish(null, value);
        };
        try {
          frame.port.postMessage(build(id));
        } catch (error) {
          finish(new ZodRunError("script", String(error)));
        }
      });
    };
    const result = tail.then(run, run);
    tail = result.catch(() => {});
    return result;
  };

  return {
    parse(message, old, macros) {
      const oldJson = JSON.stringify(old);
      return dispatch(
        (id) => ({ id, op: "parse", schema: options.schema, message, old: oldJson, macros: JSON.stringify(macros) }),
        checkDetailed,
      );
    },
    initialize(stat) {
      const statJson = JSON.stringify(stat);
      return dispatch((id) => ({ id, op: "initialize", schema: options.schema, stat: statJson }), checkInitialized);
    },
    dispose() {
      closed = true;
      if (live) retire(live);
    },
  };
}
