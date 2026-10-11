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

/** 表單存回的可見度。名單照表單原樣：表單只列沒封存的角色，名單裡不在清單上的 id（已封存的角色等）
 * 玩家看不到也取消不了，照留；玩家這次取消勾選的已經不在 draft.characters 裡。 */
export function draftVisibility(draft: WorldbookDraft): Visibility {
  return draft.visibility === "characters"
    ? { type: "characters", characters: [...draft.characters] }
    : { type: draft.visibility };
}

/** 確定已刪的角色通知：世代單調遞增；ids 是同一桌還沒被世界設定取走的累積 */
export interface CastRemoval {
  worldId: string;
  generation: number;
  ids: string[];
}

/** 草稿名單拿掉確定已刪的角色（不論目前選哪種可見度，名單都濾），規則同後端刪角色後清理：
 * 有命中才動；只有「指定角色」濾空才改 GM；沒命中回原物件 */
export function pruneDraftCharacters(draft: WorldbookDraft, removed: string[]): WorldbookDraft {
  if (!draft.characters.some((id) => removed.includes(id))) return draft;
  const kept = draft.characters.filter((id) => !removed.includes(id));
  const emptied = draft.visibility === "characters" && kept.length === 0;
  return { ...draft, characters: kept, visibility: emptied ? "gm" : draft.visibility };
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
