// 主頁（frame-host.ts）與隔離 Worker（worker-entry.ts）之間的訊息：走開機時轉交的專用 MessagePort。
// 表一律以 JSON 文字往返（同桌面：鍵順序、非有限數存成 null 的語意與存檔一致）。

/** 載入 schema 用：schema 腳本在白名單 import 處切開的片段（schema-script.ts），Worker 以墊片網址接回 */
export type ZodRequest =
  | { id: number; op: "parse"; parts: string[]; message: string; old: string; macros: string }
  | { id: number; op: "initialize"; parts: string[]; stat: string };

/** 開機：ready＝剝除與失敗即停檢查都過了；boot-failed＝還看得到危險能力 */
export type ZodBootReply = { type: "ready" } | { type: "boot-failed"; detail: string };

export type ZodReply =
  | { id: number; ok: true; json: string }
  | { id: number; ok: false; kind: "schema-load" | "script" | "bad-request"; detail: string };

/** iframe 開機訊息（只收一次，來源要是父頁、來源 origin 要是本站） */
export const BOOT_TYPE = "tt-mvu-zod-boot";
