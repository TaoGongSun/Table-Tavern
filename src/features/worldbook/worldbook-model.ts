import type { Visibility } from "../../shared/contracts/backend-contracts";

export interface WorldbookDraft {
  uid: number | null;
  title: string;
  keys: string;
  content: string;
  constant: boolean;
  enabled: boolean;
  order: number;
  visibility: Visibility["type"];
  characters: string[];
}

// 機制帳本：世界書分頁「哪些條目被本地機制接管／跳過」面板，對應 mechanism.rs 的 Ledger。
export type RecordKind = "rejected" | "clamped" | "error" | "absorbed" | "skipped" | "jump";

export interface LedgerEntry {
  uid: number;
  title: string;
  kind: RecordKind;
  detail: string;
  sent: boolean;
}

export interface Ledger {
  entries: LedgerEntry[];
  rejected: number;
  clamped: number;
  errors: number;
  jumps: number;
}

export const EMPTY_LEDGER: Ledger = {
  entries: [],
  rejected: 0,
  clamped: 0,
  errors: 0,
  jumps: 0,
};
