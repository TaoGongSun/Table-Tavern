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
  /** AI 順便產的 HTML 渲染殼；沒產出就沒有這個欄位（後端 Option<String>）。 */
  shell?: string;
  /** 這張卡自己的欄位規則（點分路徑→規則），介面接管才有。 */
  rules?: Record<string, unknown>;
  /** 這張卡自己的回報指引，套用後跟著進 GM 的系統提示詞。 */
  guide?: string;
}

export interface RefactorMechanism {
  source_uid: string;
  rules: Record<string, unknown>;
  triggers: unknown[];
}

/** carry 型條目（原文照搬）才有：原條目 keys/constant/order/disabled/visibility/is_person 原樣
 * 保留，套用時取代新條目預設值。AI 重寫的條目一律沒有這欄。 */
export interface RefactorEntryMeta {
  keys: string[];
  constant: boolean;
  order: number;
  disabled: boolean;
  visibility: unknown;
  is_person: boolean;
}

export interface RefactorNewEntry {
  title: string;
  kind: "setting" | "mechanism";
  content: string;
  source_uids: string[];
  rules?: Record<string, unknown>;
  triggers?: unknown[];
  meta?: RefactorEntryMeta;
}

export interface RefactorRewriteOutcome {
  entry: RefactorNewEntry | null;
  raw: string;
}

/** 整條淘汰（ENTRIES action=drop）或半條淘汰（SPLITS route=drop）的內容快照，供玩家展開查看、
 * 一鍵放回（轉 carry 進 entries）；span=""＝整條淘汰。 */
export interface RefactorDroppedEntry {
  uid: string;
  span: string;
  title: string;
  content: string;
  rule: number;
}

/** app 尚無執行機構的機制段：原文已經照搬進對應的 GM 規則條目（資料不會遺失），這裡只是給
 * 玩家看「有哪些機制還沒被系統接管」的清單。 */
export interface RefactorUnabsorbedItem {
  uid: string;
  span: string;
  title: string;
  note: string;
}

/** 機械稽核紅字：kind 是 coverage／mechanism／split／drop_rule 之一；span 空字串代表整條層級
 * 的稽核項（沒有特定段落）。 */
export interface RefactorAuditItem {
  kind: string;
  uid: string;
  span: string;
  detail: string;
}

export interface RefactorOutcome {
  characters: RefactorCharacter[];
  interface: RefactorInterface | null;
  entries: RefactorNewEntry[];
  mechanisms: RefactorMechanism[];
  /** 收尾階段判定「刪了只剩殘渣」的共用合集條目 uid；套用時還要所有共用這條的人都被勾選
   * 才會真的刪（要點 7：基準是優先保留而非刪除）。 */
  deletable_shared_uids: string[];
  /** 本地零呼叫組裝淘汰的整條／半條內容：預設不套用，純粹隨產物保留供玩家展開查看、一鍵放回。 */
  dropped: RefactorDroppedEntry[];
  /** app 尚無執行機構、原文已照搬進 GM 規則條目的機制清單（資訊性，內容不會遺失）。 */
  unabsorbed: RefactorUnabsorbedItem[];
  /** 機械稽核紅字：涵蓋漏網／機制守恆／拆組守恆／淘汰稽核，四類之一。 */
  audit: RefactorAuditItem[];
}

export interface RefactorSelection {
  character_indices: number[];
  apply_interface: boolean;
  mechanism_indices: number[];
  entry_indices: number[];
  /** characters 裡要設成玩家卡的那一位；null＝不指定。 */
  player_index: number | null;
}

// 盤點階段的型別，對照後端 src-tauri/src/refactor_ai.rs 的 RefactorSurveyOutcome（小抄合約 v1）。
// 人物已經是「認人」後的結果——一人一筆，來源 uid 可能多條。
export interface RefactorSurveyPerson {
  name: string;
  uids: string[];
  is_player: boolean;
  /** ""｜"clean"｜"tangled"：clean＝spans 原文可零呼叫組裝成卡，tangled＝照現行 person_expand。 */
  mode: string;
  /** mode="clean" 時這個人全部段落引用（`uid#sN`）；mode 非 clean 不使用。 */
  spans: string[];
  /** spans 之中屬於私密段的子集；mode 非 clean 不使用。 */
  private_spans: string[];
}

/** ENTRIES 一行：uid 這條原始條目該怎麼處置。rule 只有 action="drop" 才有意義。 */
export interface RefactorEntryVerdict {
  uid: string;
  action: "carry" | "absorb" | "drop" | "split";
  rule: number | null;
  reason: string;
}

/** SPLITS 一行：某個 span 的去處；rule／name／title／group／note 依 route 種類擇一使用。 */
export interface RefactorSpanRoute {
  span: string;
  route: string;
  rule: number | null;
  name: string;
  title: string;
  group: string;
  note: string;
}

/** GROUPS 一行：SPLITS 標 group 的 span 們合組成的一條新條目。 */
export interface RefactorSplitGroup {
  id: string;
  title: string;
  /** "setting"|"mechanism"。 */
  kind: string;
  spans: string[];
}

export interface RefactorSurveyOutcome {
  persons: RefactorSurveyPerson[];
  /** 全部介面條目 uid（含 playable 與否）。 */
  interface_uids: string[];
  /** 其中盤點判 playable 的介面條目 uid：展開時走 interface_shell、產殼；其餘走 interface。 */
  playable_interface_uids: string[];
  /** 非純人物、非純介面條目的分類判定：一條原始條目一筆。 */
  verdicts: RefactorEntryVerdict[];
  /** action=split 條目的逐 span 路由。 */
  splits: RefactorSpanRoute[];
  /** SPLITS 用到的 group id 對應的合組宣告。 */
  groups: RefactorSplitGroup[];
  /** 狀態欄位命名唯一權威：後續每次展開呼叫的 knownFields 都從這裡固定取用（不再沿鏈累積）。 */
  fields: string[];
  raw: string;
}

/** 本地零呼叫組裝的完整產物，對照後端 src-tauri/src/refactor_assemble.rs 的 RefactorLocalAssembly。 */
export interface RefactorLocalAssembly {
  entries: RefactorNewEntry[];
  characters: RefactorCharacter[];
  /** 已由 mode="clean" 零呼叫組裝產出的人名；buildRefactorPersonPlan 用來跳過，避免重複處理。 */
  clean_person_names: string[];
  dropped: RefactorDroppedEntry[];
  unabsorbed: RefactorUnabsorbedItem[];
  audit: RefactorAuditItem[];
}

// 展開階段（介面／機制）：對照後端 RefactorExpandOutcome，一 uid 一次呼叫的形狀。
export interface RefactorExpandOutcome {
  interface: RefactorInterface | null;
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
 * source_uids 依序串聯，raw 以空行接起來方便人審逐條核對來源。欄位規則同樣淺合併（後蓋前）。
 * 渲染殼是整份 HTML、回報指引是一段完整文字，合併沒有意義——各取最後一個非空的（與 state_fields
 * 後蓋前同向）。零條回傳 null。 */
export function mergeRefactorInterfaces(interfaces: RefactorInterface[]): RefactorInterface | null {
  if (interfaces.length === 0) return null;
  let stateFields: unknown;
  let shell = "";
  let guide = "";
  const rules: Record<string, unknown> = {};
  for (const candidate of interfaces) {
    stateFields =
      isPlainObject(stateFields) && isPlainObject(candidate.state_fields)
        ? { ...stateFields, ...candidate.state_fields }
        : candidate.state_fields;
    if (candidate.shell) shell = candidate.shell;
    if (candidate.guide) guide = candidate.guide;
    Object.assign(rules, candidate.rules ?? {});
  }
  return {
    state_fields: stateFields,
    source_uids: interfaces.flatMap((candidate) => candidate.source_uids),
    raw: interfaces.map((candidate) => candidate.raw).join("\n\n"),
    ...(shell ? { shell } : {}),
    ...(Object.keys(rules).length > 0 ? { rules } : {}),
    ...(guide ? { guide } : {}),
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

/** 盤點結果分流：cleanNames 裡的人已由本地零呼叫組裝（mode="clean"）產出卡片，直接跳過；
 * 其餘專屬單一來源的人走本地轉換（0 呼叫），其餘（多來源，或唯一來源被別人共用）進展開
 * 佇列，一人一次呼叫（要點 8）。 */
export function buildRefactorPersonPlan(
  survey: RefactorSurveyOutcome,
  entries: { uid: number; content: string }[],
  cleanNames: string[],
): { local: RefactorCharacter[]; queue: RefactorPersonQueueItem[] } {
  const byUid = groupPersonsByUid(survey.persons);
  const local: RefactorCharacter[] = [];
  const queue: RefactorPersonQueueItem[] = [];
  for (const person of survey.persons) {
    if (cleanNames.includes(person.name)) continue;
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

/** 人物、世界書條目與介面展開結果組成最終產物。舊機制欄位保留為空，讓舊版匯入卡仍可套用；
 * dropped/unabsorbed/audit 是本地零呼叫組裝的透傳資訊，省略時預設空陣列。 */
export function assembleRefactorOutcome(parts: {
  characters: RefactorCharacter[];
  interfaces: RefactorInterface[];
  entries: RefactorNewEntry[];
  dropped?: RefactorDroppedEntry[];
  unabsorbed?: RefactorUnabsorbedItem[];
  audit?: RefactorAuditItem[];
}): RefactorOutcome {
  return {
    characters: parts.characters,
    interface: mergeRefactorInterfaces(parts.interfaces),
    entries: parts.entries,
    mechanisms: [],
    deletable_shared_uids: [],
    dropped: parts.dropped ?? [],
    unabsorbed: parts.unabsorbed ?? [],
    audit: parts.audit ?? [],
  };
}

/** 匯入產物的例外訊息；呼叫端認這個字串換成玩家看得懂的一句話。 */
export const REFACTOR_IMPORT_INVALID = "refactor-import-invalid";

function invalid(): Error {
  return new Error(REFACTOR_IMPORT_INVALID);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 缺鍵補預設（比照後端 #[serde(default)]），有值但型別不對就整份拒收。 */
function readArray(source: Record<string, unknown>, key: string): unknown[] {
  const value = source[key];
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw invalid();
  return value;
}

function readString(source: Record<string, unknown>, key: string, required = false): string {
  const value = source[key];
  if (value === undefined || value === null) {
    if (required) throw invalid();
    return "";
  }
  if (typeof value !== "string") throw invalid();
  if (required && value === "") throw invalid();
  return value;
}

function readStringArray(source: Record<string, unknown>, key: string): string[] {
  return readArray(source, key).map((item) => {
    if (typeof item !== "string") throw invalid();
    return item;
  });
}

function readNumber(source: Record<string, unknown>, key: string, required = false): number {
  const value = source[key];
  if (value === undefined || value === null) {
    if (required) throw invalid();
    return 0;
  }
  if (typeof value !== "number") throw invalid();
  return value;
}

function parseCharacter(raw: unknown): RefactorCharacter {
  if (!isRecord(raw)) throw invalid();
  const source_uids = readStringArray(raw, "source_uids");
  // 每位角色都得指得出來源條目：空的多半是舊版單數 source_uid 格式，放行只會在畫面上炸開。
  if (source_uids.length === 0) throw invalid();
  return {
    name: readString(raw, "name", true),
    emoji: readString(raw, "emoji"),
    public_md: readString(raw, "public_md"),
    private_md: readString(raw, "private_md"),
    source_uids,
    solo_entry_md: readString(raw, "solo_entry_md"),
    suspected_player: raw.suspected_player === true,
  };
}

function parseInterface(raw: unknown): RefactorInterface {
  if (!isRecord(raw)) throw invalid();
  if (raw.state_fields === undefined || raw.state_fields === null) throw invalid();
  const shell = readString(raw, "shell");
  const guide = readString(raw, "guide");
  return {
    state_fields: raw.state_fields,
    source_uids: readStringArray(raw, "source_uids"),
    raw: readString(raw, "raw"),
    ...(shell ? { shell } : {}),
    ...(isRecord(raw.rules) ? { rules: raw.rules } : {}),
    ...(guide ? { guide } : {}),
  };
}

function parseMechanism(raw: unknown): RefactorMechanism {
  if (!isRecord(raw)) throw invalid();
  if (raw.rules !== undefined && raw.rules !== null && !isRecord(raw.rules)) throw invalid();
  return {
    source_uid: readString(raw, "source_uid", true),
    rules: (raw.rules as Record<string, unknown>) ?? {},
    triggers: readArray(raw, "triggers"),
  };
}

function parseNewEntry(raw: unknown): RefactorNewEntry {
  if (!isRecord(raw)) throw invalid();
  const kind = readString(raw, "kind", true);
  if (kind !== "setting" && kind !== "mechanism") throw invalid();
  if (raw.rules !== undefined && raw.rules !== null && !isRecord(raw.rules)) throw invalid();
  // meta 是物件才收（carry 型條目才有），否則整份拒收；缺席（AI 重寫的條目、舊產物 JSON）
  // 就直接不帶這欄，原樣通過不逐欄驗證——套用端只認這個結構是不是物件。
  if (raw.meta !== undefined && raw.meta !== null && !isRecord(raw.meta)) throw invalid();
  return {
    title: readString(raw, "title", true),
    kind,
    content: readString(raw, "content", true),
    source_uids: (() => {
      const sourceUids = readStringArray(raw, "source_uids");
      if (sourceUids.length === 0) throw invalid();
      return sourceUids;
    })(),
    rules: (raw.rules as Record<string, unknown>) ?? {},
    triggers: readArray(raw, "triggers"),
    ...(isRecord(raw.meta) ? { meta: raw.meta as unknown as RefactorEntryMeta } : {}),
  };
}

function parseDropped(raw: unknown): RefactorDroppedEntry {
  if (!isRecord(raw)) throw invalid();
  return {
    uid: readString(raw, "uid", true),
    span: readString(raw, "span"),
    title: readString(raw, "title"),
    content: readString(raw, "content"),
    rule: readNumber(raw, "rule", true),
  };
}

function parseUnabsorbed(raw: unknown): RefactorUnabsorbedItem {
  if (!isRecord(raw)) throw invalid();
  return {
    uid: readString(raw, "uid", true),
    span: readString(raw, "span"),
    title: readString(raw, "title"),
    note: readString(raw, "note"),
  };
}

function parseAuditItem(raw: unknown): RefactorAuditItem {
  if (!isRecord(raw)) throw invalid();
  return {
    kind: readString(raw, "kind", true),
    uid: readString(raw, "uid"),
    span: readString(raw, "span"),
    detail: readString(raw, "detail"),
  };
}

/** JSON 檔文字解析成產物。玩家自己選的檔＝信任邊界，逐欄檢查型別後才進面板——
 * 半信半疑地放行，缺欄位會在展開細看時炸成白畫面。格式不對整份拒收，訊息交呼叫端翻譯。 */
export function parseRefactorOutcome(text: string): RefactorOutcome {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw invalid();
  }
  if (!isRecord(raw)) throw invalid();
  const outcome: RefactorOutcome = {
    characters: readArray(raw, "characters").map(parseCharacter),
    interface: raw.interface === undefined || raw.interface === null ? null : parseInterface(raw.interface),
    entries: readArray(raw, "entries").map(parseNewEntry),
    mechanisms: readArray(raw, "mechanisms").map(parseMechanism),
    deletable_shared_uids: readStringArray(raw, "deletable_shared_uids"),
    // 三欄缺席（舊產物 JSON，包 4 之前存的重構卡）＝[]，照舊可解；有值但不是陣列才拒收。
    dropped: readArray(raw, "dropped").map(parseDropped),
    unabsorbed: readArray(raw, "unabsorbed").map(parseUnabsorbed),
    audit: readArray(raw, "audit").map(parseAuditItem),
  };
  // 三區全空＝這檔案沒有任何可套用的東西，多半根本不是重構產物。
  if (outcome.characters.length === 0 && !outcome.interface && outcome.entries.length === 0 && outcome.mechanisms.length === 0) {
    throw invalid();
  }
  return outcome;
}

/** 產物剛讀進來的預設勾選：全勾——玩家看到的第一印象是「照單全收」，要拿掉自己取消；
 * 盤點階段被標記疑似玩家的那位，預設就指定為玩家卡（玩家勾選時順手確認，見要點 4）。 */
export function defaultRefactorSelection(outcome: RefactorOutcome): RefactorSelection {
  const playerIndex = outcome.characters.findIndex((character) => character.suspected_player);
  return {
    character_indices: outcome.characters.map((_, index) => index),
    apply_interface: outcome.interface !== null,
    mechanism_indices: outcome.mechanisms.map((_, index) => index),
    entry_indices: outcome.entries.map((_, index) => index),
    player_index: playerIndex === -1 ? null : playerIndex,
  };
}

export interface RefactorSummaryCounts {
  characters: number;
  hasInterface: boolean;
  mechanisms: number;
  entries: number;
}

/** 結果卡摘要行只列有產物的區：三個欄位對應「拆出 N 個角色」「介面」「收編 N 條規則」。 */
export function refactorSummaryCounts(outcome: RefactorOutcome): RefactorSummaryCounts {
  return {
    characters: outcome.characters.length,
    hasInterface: outcome.interface !== null,
    mechanisms: outcome.mechanisms.length,
    entries: outcome.entries.length,
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

/** 放回被判官淘汰的整條或半條：從 dropped 移除，轉成新的 setting 條目 push 進 entries 尾端並預設
 * 勾選——玩家主動救回來的東西不該還要再手動勾一次。title 一律用原標題（2026-08-12 拍板：
 * 內部段標不露出，同條多段救回同名可接受）。不可變更新，回傳新物件。 */
export function restoreDropped(
  outcome: RefactorOutcome,
  selection: RefactorSelection,
  index: number,
): { outcome: RefactorOutcome; selection: RefactorSelection } {
  const item = outcome.dropped[index];
  const entry: RefactorNewEntry = {
    title: item.title,
    kind: "setting",
    content: item.content,
    source_uids: [item.uid],
    rules: {},
    triggers: [],
  };
  const entries = [...outcome.entries, entry];
  return {
    outcome: { ...outcome, entries, dropped: outcome.dropped.filter((_, i) => i !== index) },
    selection: { ...selection, entry_indices: [...selection.entry_indices, entries.length - 1] },
  };
}
