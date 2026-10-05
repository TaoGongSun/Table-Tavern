// 重構卡封套 `{ format, version, outcome, applied? }` 的解析：.json 重構卡（新封套與舊裸產物）
// 與 PNG 重構卡的 manifest 共用同一形狀。玩家選的檔＝信任邊界，後端 refactor/card_file.rs
// 照同一套規則再驗一次（PNG 路徑以後端為準）。
import {
  REFACTOR_IMPORT_INVALID,
  parseRefactorOutcomeValue,
  type RefactorOutcome,
} from "./refactor-review";

export const REFACTOR_CARD_FORMAT = "table-tavern-refactor-card";
export const REFACTOR_CARD_VERSION = 1;
/** 封套版本比這版 App 新：呼叫端翻成「請先更新 App」。 */
export const REFACTOR_IMPORT_NEWER = "refactor-import-newer";

export interface RefactorAppliedCharacter {
  outcome_index: number;
  /** null＝來源桌這一位沒建卡（走 person 條目） */
  character_id: string | null;
}

/** 來源桌的套用映射：跨桌只用來重現「哪幾位建卡、誰是玩家」，character_id 不拿來用。 */
export interface RefactorApplied {
  characters: RefactorAppliedCharacter[];
  player_index: number | null;
}

export interface RefactorCard {
  outcome: RefactorOutcome;
  applied: RefactorApplied | null;
}

function invalid(): Error {
  return new Error(REFACTOR_IMPORT_INVALID);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isIndex(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

/** applied 完整性（後端 validate_applied 同規則）：恰好覆蓋 0..角色數 每個 index 各一次；
 * character_id 為 null 或非空字串，非 null 的互不重複；player_index 為 null 或指向有卡的 index。 */
export function parseRefactorApplied(raw: unknown, characterCount: number): RefactorApplied {
  if (!isRecord(raw) || !Array.isArray(raw.characters)) throw invalid();
  if (raw.characters.length !== characterCount) throw invalid();
  const indices = new Set<number>();
  const ids = new Set<string>();
  const characters = raw.characters.map((item): RefactorAppliedCharacter => {
    if (!isRecord(item)) throw invalid();
    const index = item.outcome_index;
    if (!isIndex(index) || index >= characterCount || indices.has(index)) throw invalid();
    indices.add(index);
    const id = item.character_id ?? null;
    if (id !== null) {
      if (typeof id !== "string" || id.trim() === "" || ids.has(id)) throw invalid();
      ids.add(id);
    }
    return { outcome_index: index, character_id: id };
  });
  const player = raw.player_index ?? null;
  if (player !== null) {
    if (!isIndex(player)) throw invalid();
    if (!characters.some((item) => item.outcome_index === player && item.character_id !== null)) {
      throw invalid();
    }
  }
  return { characters, player_index: player };
}

/** 重構卡 JSON 文字：有 format 欄＝封套（format 不符拒收、version 只收 1、更新版回
 * REFACTOR_IMPORT_NEWER），沒有＝舊版裸產物（applied 為 null）。 */
export function parseRefactorCard(text: string): RefactorCard {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw invalid();
  }
  return parseRefactorCardValue(raw);
}

export function parseRefactorCardValue(raw: unknown): RefactorCard {
  if (!isRecord(raw)) throw invalid();
  if (!("format" in raw)) return { outcome: parseRefactorOutcomeValue(raw), applied: null };
  if (raw.format !== REFACTOR_CARD_FORMAT) throw invalid();
  if (!isIndex(raw.version) || raw.version === 0) throw invalid();
  if (raw.version > REFACTOR_CARD_VERSION) throw new Error(REFACTOR_IMPORT_NEWER);
  const outcome = parseRefactorOutcomeValue(raw.outcome);
  const applied =
    raw.applied === undefined || raw.applied === null
      ? null
      : parseRefactorApplied(raw.applied, outcome.characters.length);
  return { outcome, applied };
}

const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

export function isPng(bytes: Uint8Array): boolean {
  return bytes.length >= 8 && PNG_MAGIC.every((value, index) => bytes[index] === value);
}

/** 單一匯入入口的分流：PNG 的 chunk 表裡有自家重構卡 manifest（ttRd）。只走結構、不驗 CRC，
 * 真驗證在後端；在轉成數字陣列送 IPC 之前判斷，重構卡（可能數十 MB）不走那條慢路。 */
export function hasRefactorCardChunk(bytes: Uint8Array): boolean {
  if (!isPng(bytes)) return false;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = 8;
  while (bytes.length - offset >= 12) {
    const length = view.getUint32(offset);
    const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    if (type === "ttRd") return true;
    const end = offset + 12 + length;
    if (end > bytes.length) return false;
    offset = end;
  }
  return false;
}

/** 陣容欄匯入認出重構卡 PNG 後交給世界設定的待處理檔：綁來源桌與請求世代，換桌即作廢，
 * WorldEditor 只接手同一桌、且只接一次（世代比對）。 */
export interface PendingRefactorCard {
  worldId: string;
  file: File;
  generation: number;
}

/** refactor_card_open 的回傳：card 交 parseRefactorCardValue 再驗；assets 只有摘要，圖在後端暫存。 */
export interface RefactorCardOpenResult {
  card: unknown;
  assets: { outcome_index: number; kind: "portrait" | "avatar" }[];
  token: string | null;
}
