// AI 卡重構結果的人審面板邏輯：產物解析、預設全勾、摘要計數、出處標題查找、checkbox 切換。
// 純函式、零 UI／invoke 依賴——App.tsx 只管接線與畫面，判斷邏輯在這裡單獨測。
// 型別對照後端 src-tauri/src/refactor.rs 前段的 RefactorOutcome／RefactorSelection 契約。

export interface RefactorCharacter {
  name: string;
  emoji: string;
  public_md: string;
  private_md: string;
  /** 這位角色的資料來源條目 uid 清單；只有單一專屬來源時長度為 1（人物合併，person-promote）。 */
  source_uids: string[];
  solo_entry_md: string;
  /** 盤點階段 AI 標記的疑似玩家本人；整份 characters 至多一筆為 true。 */
  suspected_player: boolean;
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
  /** 收尾階段判定「刪了只剩殘渣」的共用合集條目 uid；套用時還要所有共用這條的人都被勾選
   * 才會真的刪（要點 7：基準是優先保留而非刪除）。 */
  deletable_shared_uids: string[];
}

export interface RefactorSelection {
  character_indices: number[];
  apply_interface: boolean;
  mechanism_indices: number[];
  /** characters 裡要設成玩家卡的那一位；null＝不指定。 */
  player_index: number | null;
}

// 盤點階段的型別，對照後端 src-tauri/src/refactor_ai.rs 的 RefactorSurveyOutcome。
// 人物已經是「認人」後的結果——一人一筆，來源 uid 可能多條。
export interface RefactorSurveyPerson {
  name: string;
  uids: string[];
  is_player: boolean;
}

export interface RefactorSurveyOutcome {
  persons: RefactorSurveyPerson[];
  interface_uids: string[];
  mechanism_uids: string[];
  raw: string;
}

// 展開階段（介面／機制）：對照後端 RefactorExpandOutcome，一 uid 一次呼叫的形狀。
export interface RefactorExpandOutcome {
  interface: RefactorInterface | null;
  mechanism: RefactorMechanism | null;
  raw: string;
}

// 展開階段（人物）：對照後端 RefactorPersonExpandOutcome，一人一次呼叫，結果只有一個角色。
export interface RefactorPersonExpandOutcome {
  character: RefactorCharacter | null;
  raw: string;
}

export interface RefactorApplySummary {
  new_characters: number;
  new_entries: number;
  /** 合併升格後整條刪除的來源世界書條目數（專屬條目＋收尾判定可刪的共用合集條目）。 */
  deleted_entries: number;
  rewritten_entries: number;
  interface_applied: boolean;
  mechanisms_applied: number;
  player_assigned: boolean;
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

/** uid → 列出這個 uid 當來源的人名清單；判斷一條來源條目是「專屬」還是「共用」的依據
 * （長度 ≤1＝專屬，≥2＝共用），本地轉換分流與收尾清單共用這份分組。 */
function groupPersonsByUid(persons: RefactorSurveyPerson[]): Map<string, string[]> {
  const byUid = new Map<string, string[]>();
  for (const person of persons) {
    for (const uid of person.uids) {
      const names = byUid.get(uid) ?? [];
      names.push(person.name);
      byUid.set(uid, names);
    }
  }
  return byUid;
}

/** 單一專屬來源的人不用展開呼叫：entries 裡找到內容直接當公開設定，免費升格候選——
 * 「認人近乎免費，合併才要花展開呼叫」。找不到對應條目就回 null，呼叫端退回展開佇列。 */
export function localConvertPerson(
  person: RefactorSurveyPerson,
  entries: { uid: number; content: string }[],
): RefactorCharacter | null {
  if (person.uids.length !== 1) return null;
  const entry = entries.find((candidate) => String(candidate.uid) === person.uids[0]);
  if (!entry) return null;
  const publicMd = entry.content.trim();
  return {
    name: person.name,
    emoji: "🎭",
    public_md: publicMd,
    private_md: "",
    source_uids: [...person.uids],
    solo_entry_md: publicMd,
    suspected_player: person.is_player,
  };
}

export interface RefactorPersonQueueItem {
  name: string;
  uids: string[];
  is_player: boolean;
}

/** 盤點結果分流：專屬單一來源的人走本地轉換（0 呼叫）；其餘（多來源，或唯一來源被別人
 * 共用）進展開佇列，一人一次呼叫（要點 8）。 */
export function buildRefactorPersonPlan(
  survey: RefactorSurveyOutcome,
  entries: { uid: number; content: string }[],
): { local: RefactorCharacter[]; queue: RefactorPersonQueueItem[] } {
  const byUid = groupPersonsByUid(survey.persons);
  const local: RefactorCharacter[] = [];
  const queue: RefactorPersonQueueItem[] = [];
  for (const person of survey.persons) {
    const exclusive = person.uids.length === 1 && (byUid.get(person.uids[0])?.length ?? 0) <= 1;
    const converted = exclusive ? localConvertPerson(person, entries) : null;
    if (converted) {
      local.push(converted);
    } else {
      queue.push({ name: person.name, uids: person.uids, is_player: person.is_player });
    }
  }
  return { local, queue };
}

export interface RefactorSharedEntryDraw {
  uid: string;
  drawn_by: string[];
}

/** 共用來源條目（uid 被兩人以上列為來源）：整理成收尾呼叫要送的「已被誰抽走」清單；
 * 沒有共用條目時回空陣列，呼叫端據此跳過收尾呼叫。 */
export function buildSharedEntryDraws(survey: RefactorSurveyOutcome): RefactorSharedEntryDraw[] {
  const byUid = groupPersonsByUid(survey.persons);
  return [...byUid.entries()]
    .filter(([, names]) => names.length >= 2)
    .map(([uid, drawn_by]) => ({ uid, drawn_by }));
}

/** 三段呼叫（人物展開＋介面展開＋機制展開）累積出的候選，加上收尾判定，組成最終產物。 */
export function assembleRefactorOutcome(parts: {
  characters: RefactorCharacter[];
  interfaces: RefactorInterface[];
  mechanisms: RefactorMechanism[];
  deletableSharedUids: string[];
}): RefactorOutcome {
  return {
    characters: parts.characters,
    interface: mergeRefactorInterfaces(parts.interfaces),
    mechanisms: parts.mechanisms,
    deletable_shared_uids: parts.deletableSharedUids,
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
    deletable_shared_uids: raw.deletable_shared_uids ?? [],
  };
}

/** 產物剛讀進來的預設勾選：全勾——玩家看到的第一印象是「照單全收」，要拿掉自己取消；
 * 盤點階段被標記疑似玩家的那位，預設就指定為玩家卡（玩家勾選時順手確認，見要點 4）。 */
export function defaultRefactorSelection(outcome: RefactorOutcome): RefactorSelection {
  const playerIndex = outcome.characters.findIndex((character) => character.suspected_player);
  return {
    character_indices: outcome.characters.map((_, index) => index),
    apply_interface: outcome.interface !== null,
    mechanism_indices: outcome.mechanisms.map((_, index) => index),
    player_index: playerIndex === -1 ? null : playerIndex,
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

/** 一位角色可能併自多條來源：逐條查標題後用「、」接起來，人審畫面看得出這張卡併了哪幾條。 */
export function sourceEntryTitles(entries: { uid: number; title: string }[], sourceUids: string[]): string {
  return sourceUids.map((uid) => sourceEntryTitle(entries, uid)).join("、");
}

/** 展開細看的 checkbox 切換：角色／機制都是 indices 陣列，勾選加入、取消移除。 */
export function toggleIndex(indices: number[], index: number, checked: boolean): number[] {
  return checked ? [...indices, index] : indices.filter((value) => value !== index);
}

/** 指定玩家：一定要同時勾選成卡，沒勾就順手勾上；index=null 表示不指定（單選、可不選）。 */
export function setPlayerIndex(selection: RefactorSelection, index: number | null): RefactorSelection {
  if (index === null) return { ...selection, player_index: null };
  const character_indices = selection.character_indices.includes(index)
    ? selection.character_indices
    : [...selection.character_indices, index];
  return { ...selection, character_indices, player_index: index };
}

/** 取消勾選某個角色：若他正是目前指定的玩家，一併清掉玩家指定（沒有卡就不能是玩家卡）。 */
export function unselectCharacter(selection: RefactorSelection, index: number): RefactorSelection {
  return {
    ...selection,
    character_indices: selection.character_indices.filter((value) => value !== index),
    player_index: selection.player_index === index ? null : selection.player_index,
  };
}
