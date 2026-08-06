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

// 盤點／展開階段的型別，對照後端 src-tauri/src/refactor_ai.rs 的 RefactorSurveyOutcome／RefactorExpandOutcome。
export interface RefactorSurveyPerson {
  uid: string;
  names: string[];
}

export interface RefactorSurveyOutcome {
  persons: RefactorSurveyPerson[];
  interface_uids: string[];
  mechanism_uids: string[];
  raw: string;
}

export interface RefactorExpandOutcome {
  characters: RefactorCharacter[];
  rewrite: { uid: string; remainder_md: string } | null;
  interface: RefactorInterface | null;
  mechanism: RefactorMechanism | null;
  raw: string;
}

export type RefactorEntryKind = "person" | "interface" | "mechanism";

export interface RefactorQueueItem {
  uid: string;
  kind: RefactorEntryKind;
}

export interface RefactorApplySummary {
  new_characters: number;
  new_entries: number;
  rewritten_entries: number;
  interface_applied: boolean;
  mechanisms_applied: number;
}

/** 盤點結果組展開佇列：人物合集每條、介面每條、機制每條，依序（人物→介面→機制）序列展開——
 * 後端 system 提示詞快取要先由第一條呼叫建立，逐條 await 不可並行。 */
export function buildRefactorExpandQueue(survey: RefactorSurveyOutcome): RefactorQueueItem[] {
  return [
    ...survey.persons.map((person): RefactorQueueItem => ({ uid: person.uid, kind: "person" })),
    ...survey.interface_uids.map((uid): RefactorQueueItem => ({ uid, kind: "interface" })),
    ...survey.mechanism_uids.map((uid): RefactorQueueItem => ({ uid, kind: "mechanism" })),
  ];
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 多條介面候選合併成一條：state_fields 兩邊都是物件就淺合併（後蓋前），否則後者整個蓋掉；
 * source_uids 依序串聯，raw 以空行接起來方便人審逐條核對來源。零條回傳 null。 */
export function mergeRefactorInterfaces(interfaces: RefactorInterface[]): RefactorInterface | null {
  if (interfaces.length === 0) return null;
  let stateFields: unknown;
  for (const candidate of interfaces) {
    stateFields =
      isPlainObject(stateFields) && isPlainObject(candidate.state_fields)
        ? { ...stateFields, ...candidate.state_fields }
        : candidate.state_fields;
  }
  return {
    state_fields: stateFields,
    source_uids: interfaces.flatMap((candidate) => candidate.source_uids),
    raw: interfaces.map((candidate) => candidate.raw).join("\n\n"),
  };
}

/** 逐條展開結果（序列 await 累積出來的陣列）合併成一份 RefactorOutcome：角色與機制全累積，
 * rewrite 過濾掉 null 的（沒有來源條目要改寫的那些 kind），介面走多條合併規則。 */
export function mergeRefactorExpandResults(results: RefactorExpandOutcome[]): RefactorOutcome {
  return {
    characters: results.flatMap((result) => result.characters),
    interface: mergeRefactorInterfaces(
      results.map((result) => result.interface).filter((candidate): candidate is RefactorInterface => candidate !== null),
    ),
    mechanisms: results
      .map((result) => result.mechanism)
      .filter((candidate): candidate is RefactorMechanism => candidate !== null),
    rewrites: results
      .map((result) => result.rewrite)
      .filter((candidate): candidate is { uid: string; remainder_md: string } => candidate !== null),
  };
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
