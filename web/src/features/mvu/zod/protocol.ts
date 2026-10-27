// 主頁（frame-host.ts）與隔離 Worker（worker-entry.ts）之間的訊息：走開機時轉交的專用 MessagePort。
// 表一律以 JSON 文字往返（同桌面：鍵順序、非有限數存成 null 的語意與存檔一致）。

/** 巨集（同桌面 quickjs-entry.ts） */
export type Macros = { user: string; char: string | null } | null;

/** 卡片 schema 腳本原文只當字串送進 QuickJS（Worker 第一次派工時載入） */
export type ZodRequest =
  | { id: number; op: "parse"; schema: string; message: string; old: string; macros: string }
  | { id: number; op: "initialize"; schema: string; stat: string };

/** 開機：ready＝QuickJS 與引擎 bundle 都起來了；boot-failed＝wasm 或引擎起不來 */
export type ZodBootReply = { type: "ready" } | { type: "boot-failed"; detail: string };

/** Worker 端的失敗種類（同桌面 FailureKind）：memory／stack／deadline／crashed 後 Worker 不再接工 */
export type ZodWorkerFailure = "schema-load" | "script" | "memory" | "stack" | "deadline" | "too-large" | "crashed" | "bad-request";

export type ZodReply = { id: number; ok: true; json: string } | { id: number; ok: false; kind: ZodWorkerFailure; detail: string };

/** iframe 開機訊息（只收一次，來源要是父頁、來源 origin 要是本站） */
export const BOOT_TYPE = "tt-mvu-zod-boot";

/** 從 QuickJS 取回的回傳字串上限（字元）：超過就整次失敗，不先整份搬出來再截（變數表 2 MB、變動值過大只記路徑） */
export const REPLY_CHARS = 8 * 1024 * 1024;
