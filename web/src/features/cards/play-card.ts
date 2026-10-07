// 一張可以開玩的卡：原卡 JSON 外殼原樣留著（桌檔契約要原封不動帶走），另外整理出提示組裝、
// regex 腳本、開場白要用的欄位。匯入檔的錯誤在這裡翻成玩家看得懂的分類。
import { regexScriptsFrom, type RegexScript } from "../sillytavern/regex-scripts";
import type { CardText } from "../sillytavern/substitute";
import { cardData, decodeCardFile, isObject, type CardErrorCode, type CardSource } from "./card-file";
import { viewFromValue, type CardView } from "./card-view";

export type CardRoute = "character" | "worldbook";

export interface PlayCard {
  /** 原卡 JSON 外殼（含 spec、spec_version、未知欄位），一個字都不改 */
  shell: unknown;
  source: CardSource | "builtin";
  text: CardText;
  regexScripts: RegexScript[];
  /**
   * 玩家是否允許卡內 regex 腳本（D21〔作者裁決 2026-10-07〕照 ST 匯入時問一次）。不允許就整組不套，
   * 送模前與顯示都一樣。預設照 ST：沒同意前不套（regex/index.js isScopedScriptsAllowed）。存檔要帶。
   */
  regexAllowed: boolean;
  depthPrompt: { depth: number; role: "system" | "user" | "assistant" };
  /** first_mes＋alternate_greetings（各自去頭尾空白、略過空的） */
  openings: string[];
  view: CardView;
  /** 匯入身分（角色卡路／世界書路）；桌面版匯入網頁存檔時照這個走，不重新推測 */
  route: CardRoute;
}

/** 網頁版只玩單張角色卡：角色卡檔太大就不收（信任邊界）。 */
export const MAX_CARD_BYTES = 30 * 1024 * 1024;

export type ImportErrorCode = CardErrorCode | "too_large" | "standalone_book" | "missing_name" | "name_not_single_line";

const text = (data: unknown, field: string) => (isObject(data) && typeof data[field] === "string" ? (data[field] as string) : "");

function depthPrompt(data: unknown): PlayCard["depthPrompt"] {
  const raw = isObject(data) && isObject(data.extensions) ? data.extensions.depth_prompt : undefined;
  const depth = isObject(raw) && typeof raw.depth === "number" && Number.isInteger(raw.depth) && raw.depth >= 0 ? raw.depth : 4;
  const role = isObject(raw) && (raw.role === "user" || raw.role === "assistant") ? raw.role : "system";
  return { depth, role };
}

export function playCardFromValue(source: PlayCard["source"], value: unknown): PlayCard {
  const data = cardData(value);
  const view = viewFromValue(source === "builtin" ? "json" : source, value);
  const greetings = isObject(data) && Array.isArray(data.alternate_greetings) ? data.alternate_greetings : [];
  const extensions = isObject(data) ? data.extensions : undefined;
  return {
    shell: value,
    source,
    text: {
      name: text(data, "name").trim(),
      description: text(data, "description"),
      personality: text(data, "personality"),
      scenario: text(data, "scenario"),
      first_mes: text(data, "first_mes"),
      mes_example: text(data, "mes_example"),
      creator_notes: text(data, "creator_notes"),
      system_prompt: text(data, "system_prompt"),
      post_history_instructions: text(data, "post_history_instructions"),
      alternate_greetings: greetings.filter((item): item is string => typeof item === "string"),
      character_version: text(data, "character_version"),
      depth_prompt: isObject(extensions) && isObject(extensions.depth_prompt) ? text(extensions.depth_prompt, "prompt") : "",
    },
    regexScripts: regexScriptsFrom(extensions),
    regexAllowed: false,
    depthPrompt: depthPrompt(data),
    openings: view.openings,
    view,
    // 預設走桌面版身分框的主按鈕那條；那條桌面版會拒收就改走另一條
    route: view.route.suggested === "worldbook" && view.validity.worldbook === null ? "worldbook" : "character",
  };
}

/** 這張卡實際生效的 regex 腳本：玩家沒允許就一個都不套。 */
export const activeRegexScripts = (card: PlayCard): RegexScript[] => (card.regexAllowed ? card.regexScripts : []);

export type ImportResult = { ok: true; card: PlayCard } | { ok: false; error: ImportErrorCode };

/**
 * 匯入一個卡檔：解碼照卡片契約；沒有角色可玩的（獨立世界書檔、沒有名字）不收——那種檔案請玩家
 * 用桌面版匯入。分路照桌面版：身分兩可的卡預設用桌面版身分框的主按鈕那條。
 */
export function importCardBytes(bytes: Uint8Array): ImportResult {
  if (bytes.length > MAX_CARD_BYTES) return { ok: false, error: "too_large" };
  const decoded = decodeCardFile(bytes);
  if (!decoded.ok) return { ok: false, error: decoded.error };
  const card = playCardFromValue(decoded.source, decoded.value);
  if (card.view.route.book_shaped) return { ok: false, error: "standalone_book" };
  if (card.view.route.name === null) return { ok: false, error: card.view.books.worldbook.source === "top_level" ? "standalone_book" : "missing_name" };
  // 有效性照卡片契約：桌面版角色卡路會拒收的卡，網頁版也不收（存檔才帶得回桌面版）
  const invalid = card.view.validity.character;
  if (invalid) return { ok: false, error: invalid === "card_missing_name" ? "missing_name" : invalid };
  return { ok: true, card };
}

/** 先看檔案大小再讀內容：超過上限的檔不讀進記憶體。 */
export async function importCardFile(file: Pick<File, "size" | "arrayBuffer">): Promise<ImportResult> {
  if (file.size > MAX_CARD_BYTES) return { ok: false, error: "too_large" };
  return importCardBytes(new Uint8Array(await file.arrayBuffer()));
}
