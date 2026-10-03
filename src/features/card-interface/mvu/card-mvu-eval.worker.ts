// 宿主專用的值解析 Worker（計畫 8.9）：收 `{ id, op, text }`、回 `{ id, ok, value | error }`。
// 開機先備好 mathjs 實例再發 `ready`，宿主的 200 ms 逾時從派工那刻才開始算。
import { prepareMath, runEval, type EvalOp } from "./card-mvu-parse-engine";

const scope = self as unknown as { postMessage: (message: unknown) => void };

prepareMath();
self.addEventListener("message", (event: MessageEvent) => {
  const data = event.data as { id?: unknown; op?: unknown; text?: unknown } | null;
  if (data === null || typeof data !== "object" || typeof data.id !== "number") return;
  if ((data.op !== "value" && data.op !== "patch") || typeof data.text !== "string") {
    scope.postMessage({ id: data.id, ok: false, error: "bad-request" });
    return;
  }
  scope.postMessage({ id: data.id, ...runEval(data.op as EvalOp, data.text) });
});
scope.postMessage({ type: "ready" });
