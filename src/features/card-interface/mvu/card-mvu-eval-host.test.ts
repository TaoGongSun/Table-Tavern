// 宿主端值解析 Worker 管理（包 2c，計畫 8.9）：序列派工、200 ms 逾時終止並重建、開機等待、關閉。
import { afterEach, describe, expect, it, vi } from "vitest";
import { createEvalHost, parseEvalRequest, type EvalWorkerLike } from "./card-mvu-eval-host";
import { runEval } from "./card-mvu-parse-engine";

type Handler = (event: { data?: unknown }) => void;

/** 假 Worker：mode 決定怎麼回；ready＝要不要發開機訊號 */
function fakeWorkers(mode: (index: number) => "answer" | "hang" | "no-ready" | "error") {
  const created: { terminated: boolean; requests: unknown[]; fire: (type: "message" | "error", data?: unknown) => void }[] = [];
  const createWorker = (): EvalWorkerLike => {
    const index = created.length;
    const handlers: Record<string, Handler[]> = { message: [], error: [] };
    const record = { terminated: false, requests: [] as unknown[], fire: (type: "message" | "error", data?: unknown) => handlers[type].forEach((h) => h({ data })) };
    created.push(record);
    const behavior = mode(index);
    queueMicrotask(() => {
      if (behavior !== "no-ready") record.fire("message", { type: "ready" });
      if (behavior === "error") record.fire("error");
    });
    return {
      postMessage(message) {
        record.requests.push(message);
        if (behavior !== "answer") return;
        const { id, op, text } = message as { id: number; op: "value" | "patch"; text: string };
        queueMicrotask(() => record.fire("message", { id, ...runEval(op, text) }));
      },
      terminate() {
        record.terminated = true;
      },
      addEventListener(type, listener) {
        handlers[type].push(listener);
      },
    };
  };
  return { created, createWorker };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("值解析 Worker 宿主", () => {
  it("開機後依序派工、回結果；同一支 Worker 重複使用", async () => {
    const { created, createWorker } = fakeWorkers(() => "answer");
    const host = createEvalHost({ createWorker });
    const results = await Promise.all([host.run("value", "1+1"), host.run("value", "'a'"), host.run("patch", "[]")]);
    expect(results).toEqual([
      { ok: true, value: 2 },
      { ok: true, value: "a" },
      { ok: true, value: [] },
    ]);
    expect(created).toHaveLength(1);
    expect(created[0].requests.map((r) => (r as { id: number }).id)).toEqual([1, 2, 3]);
    host.dispose();
  });

  it("單筆超過 200 ms：回 timeout、terminate 該 Worker；下一筆建新的 Worker 照常", async () => {
    vi.useFakeTimers();
    const { created, createWorker } = fakeWorkers((index) => (index === 0 ? "hang" : "answer"));
    const host = createEvalHost({ createWorker });
    const slow = host.run("value", "inv(ones(500,500))");
    const next = host.run("value", "2+3");
    await vi.advanceTimersByTimeAsync(199);
    expect(created).toHaveLength(1);
    expect(created[0].terminated).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(await slow).toEqual({ ok: false, error: "timeout" });
    expect(created[0].terminated).toBe(true);
    // 排在後面的那筆由新建的 Worker 處理
    await vi.advanceTimersByTimeAsync(0);
    expect(await next).toEqual({ ok: true, value: 5 });
    expect(created).toHaveLength(2);
    expect(created[1].terminated).toBe(false);
    // 舊 Worker 晚到的訊息不認
    created[0].fire("message", { id: 1, ok: true, value: "晚到" });
    expect(await host.run("value", "1")).toEqual({ ok: true, value: 1 });
    host.dispose();
  });

  it("逾時從派工那刻算起：開機慢不吃掉 200 ms", async () => {
    vi.useFakeTimers();
    const handlers: Handler[] = [];
    const worker: EvalWorkerLike = {
      postMessage: (message) => {
        const { id, op, text } = message as { id: number; op: "value"; text: string };
        queueMicrotask(() => handlers.forEach((h) => h({ data: { id, ...runEval(op, text) } })));
      },
      terminate: () => {},
      addEventListener: (type, listener) => {
        if (type === "message") handlers.push(listener);
      },
    };
    const host = createEvalHost({ createWorker: () => worker });
    const pending = host.run("value", "3*3");
    await vi.advanceTimersByTimeAsync(5000);
    handlers.forEach((h) => h({ data: { type: "ready" } }));
    expect(await pending).toEqual({ ok: true, value: 9 });
    host.dispose();
  });

  it("開機逾時、Worker 出錯：該批回錯並丟掉那支 Worker，之後的請求重建", async () => {
    vi.useFakeTimers();
    const { created, createWorker } = fakeWorkers((index) => (index === 0 ? "no-ready" : index === 1 ? "error" : "answer"));
    const host = createEvalHost({ createWorker, bootMs: 1000 });
    const booting = host.run("value", "1");
    await vi.advanceTimersByTimeAsync(1001);
    expect(await booting).toEqual({ ok: false, error: "worker-boot-timeout" });
    expect(created[0].terminated).toBe(true);
    const broken = host.run("value", "1");
    await vi.advanceTimersByTimeAsync(0);
    expect(await broken).toEqual({ ok: false, error: "worker-error" });
    expect(created[1].terminated).toBe(true);
    const healthy = host.run("value", "4");
    await vi.advanceTimersByTimeAsync(0);
    expect(await healthy).toEqual({ ok: true, value: 4 });
    host.dispose();
  });

  it("重建 Worker 丟例外、postMessage 丟例外：該請求回錯、不懸掛，之後的請求照常", async () => {
    vi.useFakeTimers();
    let built = 0;
    const { createWorker } = fakeWorkers((index) => (index === 0 ? "hang" : "answer"));
    const host = createEvalHost({
      createWorker: () => {
        built += 1;
        if (built === 2) throw new Error("SecurityError");
        return createWorker();
      },
    });
    const first = host.run("value", "1");
    const second = host.run("value", "2");
    await vi.advanceTimersByTimeAsync(201);
    expect(await first).toEqual({ ok: false, error: "timeout" });
    expect(await second).toEqual({ ok: false, error: "worker-create-failed" });
    const third = host.run("value", "3");
    await vi.advanceTimersByTimeAsync(0);
    expect(await third).toEqual({ ok: true, value: 3 });
    // postMessage 丟例外
    const broken = createEvalHost({
      createWorker: () => ({
        postMessage: () => {
          throw new Error("DataCloneError");
        },
        terminate: () => {},
        addEventListener: (type, listener) => {
          if (type === "message") queueMicrotask(() => listener({ data: { type: "ready" } }));
        },
      }),
    });
    const outcome = broken.run("value", "1");
    await vi.advanceTimersByTimeAsync(0);
    expect(await outcome).toEqual({ ok: false, error: "worker-post-failed" });
    host.dispose();
    broken.dispose();
  });

  it("dispose：終止 Worker、未完成的請求回 closed、之後的請求直接回 closed", async () => {
    const { created, createWorker } = fakeWorkers(() => "hang");
    const host = createEvalHost({ createWorker });
    const pending = host.run("value", "1");
    await Promise.resolve();
    host.dispose();
    expect(await pending).toEqual({ ok: false, error: "closed" });
    expect(created[0].terminated).toBe(true);
    expect(await host.run("value", "1")).toEqual({ ok: false, error: "closed" });
  });

  it("parseEvalRequest：形狀不對回 null", () => {
    expect(parseEvalRequest({ requestId: "t:e1", op: "value", text: "1" })).toEqual({ requestId: "t:e1", op: "value", text: "1" });
    expect(parseEvalRequest({ requestId: 1, op: "value", text: "1" })).toBeNull();
    expect(parseEvalRequest({ requestId: "x", op: "run", text: "1" })).toBeNull();
    expect(parseEvalRequest({ requestId: "x", op: "patch", text: 1 })).toBeNull();
  });
});
