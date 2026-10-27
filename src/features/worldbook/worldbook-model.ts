import type { Visibility, WorldbookEntry } from "../../shared/contracts/backend-contracts";

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

/** 撤銷匯入（後端 UndoReport.entry_changes）或匯入合併（widened_entries，只有 visibility 組）實際改了哪些既有條目：
 * changed 的 groups 是草稿欄組 */
export interface EntryChanges {
  changed: { uid: number; groups: DraftGroup[] }[];
  deleted: number[];
}

/** 世界設定的通知：刪角色只帶 ids；撤銷匯入另帶這次的條目改動（null＝後端讀不到世界書，無從判斷）；
 * 匯入兩者都可能空（只重讀），擴大了既有條目可見度時帶一筆。
 * 世代單調遞增；ids、entryChanges 是同一桌還沒被世界設定取走的累積 */
export interface WorldChange {
  worldId: string;
  generation: number;
  ids: string[];
  entryChanges: (EntryChanges | null)[];
}

/** 草稿欄組，與後端 undo_changes 的欄組名一致；visibility 含名單 */
export type DraftGroup =
  | "title"
  | "keys"
  | "content"
  | "constant"
  | "enabled"
  | "order"
  | "visibility";

/** 條目轉成表單草稿；鍵順序固定（draftDirty 以 JSON.stringify 比對） */
export function draftFromEntry(entry: WorldbookEntry, keySeparator: string): WorldbookDraft {
  return {
    uid: entry.uid,
    title: entry.title,
    keys: entry.keys.join(keySeparator),
    content: entry.content,
    constant: entry.constant,
    enabled: !entry.disabled,
    order: entry.order,
    visibility: entry.visibility.type,
    characters: entry.visibility.type === "characters" ? entry.visibility.characters : [],
  };
}

/** 表單關鍵字欄轉回陣列：半形逗號與全形頓號都收，各段去空白、丟空段 */
export function parseKeys(text: string): string[] {
  return text
    .split(/[,、]/)
    .map((key) => key.trim())
    .filter(Boolean);
}

/** 開著的條目草稿與它開啟當下的原樣快照（JSON）；seq 是開啟序號，換一條就遞增 */
export interface DraftState {
  draft: WorldbookDraft | null;
  origin: string;
  seq: number;
}

/** 待定基：還沒套到草稿上的撤銷／匯入改動，同 uid 欄組取聯集 */
export interface PendingRebase {
  changed: Map<number, Set<DraftGroup>>;
  deleted: Set<number>;
}

export function mergePendingRebase(
  pending: PendingRebase | null,
  changes: EntryChanges,
): PendingRebase {
  const next: PendingRebase = {
    changed: new Map(pending?.changed ?? []),
    deleted: new Set(pending?.deleted ?? []),
  };
  for (const change of changes.changed) {
    next.changed.set(change.uid, new Set([...(next.changed.get(change.uid) ?? []), ...change.groups]));
  }
  for (const uid of changes.deleted) next.deleted.add(uid);
  return next;
}

/** 把草稿的某欄組換成來源值 */
function withGroup(target: WorldbookDraft, source: WorldbookDraft, group: DraftGroup): WorldbookDraft {
  if (group === "visibility") {
    return { ...target, visibility: source.visibility, characters: [...source.characters] };
  }
  return { ...target, [group]: source[group] };
}

/** 依撤銷／匯入報上來的改動把開著的既有條目草稿定基：條目被刪（或重讀找不到）→ 關；
 * 被改的欄組草稿與快照都換成重讀值，玩家在這些欄組的輸入丟掉（撤銷〔作者裁決 2026-10-11〕；匯入比照〔模型判斷·未裁決〕）；其餘欄組不動。新條目草稿與沒命中的不動。
 * 回傳的 origin 鍵順序與 draftFromEntry 一致。 */
export function rebaseDraft(
  state: DraftState,
  pending: PendingRebase,
  fresh: WorldbookEntry[],
  keySeparator: string,
): DraftState {
  const { draft } = state;
  if (!draft || draft.uid === null) return state;
  if (pending.deleted.has(draft.uid)) return { ...state, draft: null, origin: "" };
  const groups = pending.changed.get(draft.uid);
  if (!groups) return state;
  const entry = fresh.find((candidate) => candidate.uid === draft.uid);
  if (!entry) return { ...state, draft: null, origin: "" };
  const reread = draftFromEntry(entry, keySeparator);
  let nextDraft = draft;
  let nextOrigin = JSON.parse(state.origin) as WorldbookDraft;
  for (const group of groups) {
    nextDraft = withGroup(nextDraft, reread, group);
    nextOrigin = withGroup(nextOrigin, reread, group);
  }
  return { ...state, draft: nextDraft, origin: JSON.stringify(nextOrigin) };
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
export type RecordKind =
  | "rejected"
  | "clamped"
  | "error"
  | "absorbed"
  | "skipped"
  | "jump"
  | "ignored";

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
