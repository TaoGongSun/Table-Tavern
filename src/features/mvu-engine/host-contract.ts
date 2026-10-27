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
  /** 選配：zod 命名空間（zod 4）。給了，卡片 schema 經 registerMvuSchema 註冊後走 mvu_zod 語意；沒給（iframe）就不走 */
  z?: unknown;
}

export interface MvuCore {
  /** MVU updateVariables：深拷貝 old 後照訊息裡的指令更新、回新資料（超出上限丟錯） */
  parseMessage: (message: string, old: Record<string, unknown>) => Promise<Record<string, unknown>>;
  /** 宿主提交用：完整新表＋zod 逐條結果＋stat_data 逐葉變動（整次失敗丟錯） */
  parseMessageDetailed: (message: string, old: Record<string, unknown>) => Promise<MvuDetailedResult>;
  generateSchema: (data: unknown, oldSchema?: unknown) => Record<string, unknown>;
  cleanUpMetadata: (data: unknown) => void;
  /** 卡片 schema 腳本 import 的 registerMvuSchema（mvu_zod）：只認第一份，之後的只計數 */
  registerMvuSchema: (input: unknown) => void;
  zodStatus: () => { registered: boolean; duplicates: number };
  /** 開局（VARIABLE_INITIALIZED）：整份 stat_data 過 schema，成功就淺合併回 variables.stat_data */
  zodInitialize: (variables: Record<string, unknown>) => ZodInitOutcome;
}

export type ZodInitOutcome =
  | { ok: true; zod: boolean }
  | { ok: false; zod: true; issues: string[]; issueCount: number };

/** zod 逐條結果。applied：已套用；no-change：過了 schema 但資料沒變（例如巢狀未列鍵被剝掉）；
 *  rejected：上游會通知 GM 的拒收（schema、add-not-number、move-source-missing、move-source-<刪來源那步的原因>）；
 *  ignored：上游靜默丟掉（唯讀路徑、add 沒有路徑、目標不是容器、不認得的指令） */
export interface ZodCommandResult {
  index: number;
  op: string;
  /** 指令作用的路徑（分段，鍵名完整保留）；delete 照上游把所有參數接成一條 */
  path: string[];
  /** move 的目的地 */
  to?: string[];
  status: "applied" | "no-change" | "rejected" | "ignored";
  reason?: string;
  issues?: string[];
  issueCount?: number;
}

export interface MvuChange {
  path: string[];
  before?: unknown;
  after?: unknown;
  /** 值太大只記路徑 */
  truncated?: true;
}

export interface MvuDetailedResult {
  table: Record<string, unknown>;
  zod: boolean;
  results: ZodCommandResult[];
  changes: MvuChange[];
  changesTruncated: boolean;
}
