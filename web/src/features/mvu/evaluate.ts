// MVU 指令值的解析與數學式：桌面版同一支宿主 Worker（mathjs、json5、yaml、jsonrepair，單筆 200 ms 逾時就終止）。
// 宿主處理模型回覆與卡片 iframe 的 parseMessage 共用一支；第一次有請求才建 Worker。
import { createEvalHost, type EvalOutcome } from "@desktop/features/card-interface/mvu/card-mvu-eval-host";
import type { EvalOp } from "@desktop/features/card-interface/mvu/card-mvu-parse-engine";

export interface Evaluator {
  run: (op: EvalOp, text: string) => Promise<EvalOutcome>;
  dispose: () => void;
}

export function createEvaluator(): Evaluator {
  return createEvalHost();
}
