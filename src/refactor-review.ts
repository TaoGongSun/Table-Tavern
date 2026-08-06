// AI 卡重構結果的人審面板邏輯：產物解析、預設全勾、摘要計數、出處標題查找、checkbox 切換。
// 純函式、零 UI／invoke 依賴——App.tsx 只管接線與畫面，判斷邏輯在這裡單獨測。
// 型別對照後端 src-tauri/src/refactor.rs 前 95 行的 RefactorOutcome／RefactorSelection 契約。

export interface RefactorCharacter {
  name: string;
  emoji: string;
  public_md: string;
  private_md: string;
  /** 這位角色是從哪條世界書條目切出來的；同一條目切出多人時 source_uid 重複。 */
  source_uid: string;
  solo_entry_md: string;
}

export interface RefactorInterface {
  state_fields: unknown;
  source_uids: string[];
  raw: string;
}

export interface RefactorMechanism {
  source_uid: string;
  rules: Record<string, unknown>;
  triggers: unknown[];
}

export interface RefactorOutcome {
  characters: RefactorCharacter[];
  interface: RefactorInterface | null;
  mechanisms: RefactorMechanism[];
  rewrites: { uid: string; remainder_md: string }[];
}

export interface RefactorSelection {
  character_indices: number[];
  apply_interface: boolean;
  mechanism_indices: number[];
}

export interface RefactorApplySummary {
  new_characters: number;
  new_entries: number;
  rewritten_entries: number;
  interface_applied: boolean;
  mechanisms_applied: number;
}

/** JSON 檔文字解析成產物；缺頂層鍵比照後端 #[serde(default)] 補空，格式不對就丟例外給呼叫端接。 */
export function parseRefactorOutcome(text: string): RefactorOutcome {
  const raw = JSON.parse(text) as Partial<RefactorOutcome> | null;
  if (!raw || typeof raw !== "object") throw new Error("not an object");
  return {
    characters: raw.characters ?? [],
    interface: raw.interface ?? null,
    mechanisms: raw.mechanisms ?? [],
    rewrites: raw.rewrites ?? [],
  };
}

/** 產物剛讀進來的預設勾選：全勾——玩家看到的第一印象是「照單全收」，要拿掉自己取消。 */
export function defaultRefactorSelection(outcome: RefactorOutcome): RefactorSelection {
  return {
    character_indices: outcome.characters.map((_, index) => index),
    apply_interface: outcome.interface !== null,
    mechanism_indices: outcome.mechanisms.map((_, index) => index),
  };
}

export interface RefactorSummaryCounts {
  characters: number;
  hasInterface: boolean;
  mechanisms: number;
}

/** 結果卡摘要行只列有產物的區：三個欄位對應「拆出 N 個角色」「介面」「收編 N 條規則」。 */
export function refactorSummaryCounts(outcome: RefactorOutcome): RefactorSummaryCounts {
  return {
    characters: outcome.characters.length,
    hasInterface: outcome.interface !== null,
    mechanisms: outcome.mechanisms.length,
  };
}

/** source_uid 對世界書條目查標題；查不到（條目已刪或 uid 對不上）就顯示 uid 本身兜底。 */
export function sourceEntryTitle(entries: { uid: number; title: string }[], sourceUid: string): string {
  const entry = entries.find((candidate) => String(candidate.uid) === sourceUid);
  return entry ? entry.title || sourceUid : sourceUid;
}

/** 展開細看的 checkbox 切換：角色／機制都是 indices 陣列，勾選加入、取消移除。 */
export function toggleIndex(indices: number[], index: number, checked: boolean): number[] {
  return checked ? [...indices, index] : indices.filter((value) => value !== index);
}
