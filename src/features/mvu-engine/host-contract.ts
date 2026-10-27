// MVU 共用核心與宿主之間的介面（engine-source.ts 的 `createMvuCore(env)`）。
import type { EvalOp } from "./value-parser";

/** 值解析結果；`error` 由核心轉成字串 */
export type EvalReply = { ok: true; value: unknown } | { ok: false; error: unknown };

export interface MvuHost {
  /** 值解析與數學式（value-parser.ts 的 runEval）：結果交給 `done`（可同步呼叫），每次都要回覆或拋錯；
   *  回傳取消函式（核心保險逾時時呼叫，之後不再收結果）或 null */
  evaluate: (op: EvalOp, text: string, done: (reply: EvalReply) => void) => (() => void) | null;
  /** 選配、成對提供：有計時器的宿主才給（iframe、網頁）。核心拿來設值解析的保險逾時；沒給就只靠整批期限 */
  timers?: { set: (fn: () => void, ms: number) => unknown; clear: (id: unknown) => void };
  /** UTF-8 位元組長度（上限檢查用） */
  utf8Length: (text: string) => number;
  /** 目前的巨集代換值；每次抽指令時讀 */
  macros: () => { user: string; char: string | null } | null | undefined;
}

export interface MvuCoreEnv {
  host: MvuHost;
  /** MVU 事件（名稱、參數、是否隔離監聽器錯誤）；依序 await */
  emit: (name: string, args: unknown[], isolate: boolean) => Promise<unknown>;
  /** MVU 事件名（MagVarUpdate variable_def.ts） */
  events: Record<string, string>;
  lodash: unknown;
}

export interface MvuCore {
  /** MVU updateVariables：深拷貝 old 後照訊息裡的指令更新、回新資料（超出上限丟錯） */
  parseMessage: (message: string, old: Record<string, unknown>) => Promise<Record<string, unknown>>;
  generateSchema: (data: unknown, oldSchema?: unknown) => Record<string, unknown>;
  cleanUpMetadata: (data: unknown) => void;
}
