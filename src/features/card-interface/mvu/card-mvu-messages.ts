// 卡片沙盒的 MVU 請求（寫入、值解析、zod 桌 parseMessage）在宿主這側的形狀檢查與服務：桌面 controller 與網頁
// frontend 宿主共用。純函式，不存狀態；iframe 身分、token、生命週期由呼叫端判定，依賴（Worker、後端呼叫、回覆）用參數給。
import { type EvalHost, parseEvalRequest } from "./card-mvu-eval-host";
import { type MvuWriteRequest } from "./card-mvu-write";

type Reply = (message: Record<string, unknown>) => void;

/** 沙盒送來的一筆寫入；形狀不對回 null（不理） */
export function parseWriteRequest(data: Record<string, unknown>): MvuWriteRequest | null {
  if (
    typeof data.requestId !== "string" ||
    typeof data.target !== "string" ||
    typeof data.payload !== "string" ||
    !(typeof data.base === "string" || data.base === null) ||
    !Number.isInteger(data.generation) ||
    !Number.isInteger(data.scene)
  ) {
    return null;
  }
  return {
    requestId: data.requestId,
    target: data.target,
    payload: data.payload,
    base: data.base,
    generation: data.generation as number,
    scene: data.scene as number,
  };
}

/** parseMessage 的值解析與數學式：交給宿主專用 Worker，任何例外（Worker 建不出來、結果送不回）都回錯，不讓請求懸著 */
export function serveEval(evals: Pick<EvalHost, "run">, data: Record<string, unknown>, reply: Reply): void {
  const request = parseEvalRequest(data);
  if (request === null) return;
  void evals
    .run(request.op, request.text)
    .catch((reason: unknown) => ({ ok: false, error: `host-error: ${String(reason)}` }))
    .then((outcome) => reply({ kind: "mvu-eval-result", requestId: request.requestId, ...(outcome as object) }));
}

export interface ParseRequest {
  requestId: string;
  message: string;
  /** 舊表的 JSON 文字（保留鍵序） */
  old: string;
}

export function parseParseRequest(data: Record<string, unknown>): ParseRequest | null {
  if (typeof data.requestId !== "string" || typeof data.message !== "string" || typeof data.old !== "string") return null;
  return { requestId: data.requestId, message: data.message, old: data.old };
}

/** 宿主代跑的結果：`zod` false＝確認不是 zod 桌（沙盒自己跑）；true 時 `table` 是結果表的 JSON 文字 */
export type HostParse = { zod: false } | { zod: true; table: string };

/** zod 桌的 parseMessage（card-shim-no-zod）：交宿主的 zod 引擎只算不寫。`run` 拋錯（判定失敗、還沒載好、整次失敗）就回錯，
 *  沙盒據以 reject，不退回 MagVarUpdate 語意 */
export function serveParse(run: (message: string, old: string) => Promise<HostParse>, data: Record<string, unknown>, reply: Reply): void {
  const request = parseParseRequest(data);
  if (request === null) return;
  void Promise.resolve()
    .then(() => run(request.message, request.old))
    .then(
      (result) => (result.zod ? { ok: true, zod: true, table: result.table } : { ok: true, zod: false }),
      (reason: unknown) => ({ ok: false, error: String(reason instanceof Error ? reason.message : reason).slice(0, 500) }),
    )
    .then((outcome) => reply({ kind: "mvu-parse-result", requestId: request.requestId, ...outcome }));
}
