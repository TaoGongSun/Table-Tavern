// 後端訊息代碼（ui_msg.rs 的 `TTMSG:{json}`）轉成目前語系的文字。
// state 一律存後端原文，render 時才呼叫 backendText；程式要判斷哪種錯誤用 backendCode。
// 舊版後端或舊檔的中文、未知 code、壞標記都整段原文保留，不猜。
import { t, type MsgKey } from "../../i18n";
import { BACKEND_MSG_PARAMS } from "../../i18n/features/backend-msg";

const MARK = "TTMSG:";
/** parse 回這個＝JSON 語法本身壞掉，跟「合法但不認得」分開處理。 */
const SYNTAX = Symbol("syntax");
/** 巢狀翻譯最多幾層（跟 ui_msg.rs 一致）；更深的 error 原文代入。 */
const MAX_DEPTH = 3;

interface Parsed {
  code: string;
  params: Record<string, string | number | boolean>;
}

/** 只做顯示：字串中每個合法標記換成譯文，其餘文字原樣。 */
export function backendText(raw: unknown): string {
  return render(String(raw), 0);
}

/**
 * 給程式判斷用：只認字串起首（容許 invoke 包裝的 `Error: `）那一個合法標記，回傳 code。
 * 夾在其他文字中間的標記不算，避免供應商 body 之類的原文被誤判。
 */
export function backendCode(raw: unknown): string | null {
  let text = String(raw);
  if (text.startsWith("Error: ")) text = text.slice("Error: ".length);
  if (!text.startsWith(MARK)) return null;
  const end = jsonObjectEnd(text, MARK.length);
  if (end < 0) return null;
  const parsed = parse(text.slice(MARK.length, end));
  return parsed === SYNTAX ? null : (parsed?.code ?? null);
}

function render(text: string, depth: number): string {
  let out = "";
  let from = 0;
  for (;;) {
    const at = text.indexOf(MARK, from);
    if (at < 0) return out + text.slice(from);
    out += text.slice(from, at);
    const start = at + MARK.length;
    const end = jsonObjectEnd(text, start);
    const parsed = end < 0 ? SYNTAX : parse(text.slice(start, end));
    if (parsed === SYNTAX) {
      // 括號不成對或 JSON 語法壞掉：標記本身原樣輸出，接著往後找（裡面可能夾著合法標記）。
      out += MARK;
      from = start;
      continue;
    }
    // 合法 JSON 但不是認得的訊息：整段原文保留。
    out += parsed ? translate(parsed, depth) : text.slice(at, end);
    from = end;
  }
}

function translate({ code, params }: Parsed, depth: number): string {
  const values: Record<string, string> = {};
  for (const [name, value] of Object.entries(params)) {
    values[name] =
      name === "error" && typeof value === "string" && depth + 1 < MAX_DEPTH
        ? render(value, depth + 1)
        : String(value);
  }
  return t(`be_${code}` as MsgKey, values);
}

/** 驗證物件、已知 code、必要參數存在且型別對；任何一項不合就回 null（呼叫端保留原文）。 */
function parse(json: string): Parsed | null | typeof SYNTAX {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    return SYNTAX;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  const code = record.code;
  if (typeof code !== "string" || !Object.prototype.hasOwnProperty.call(BACKEND_MSG_PARAMS, code)) {
    return null;
  }
  const params: Parsed["params"] = {};
  for (const [name, type] of Object.entries(BACKEND_MSG_PARAMS[code])) {
    const param = record[name];
    if (typeof param !== type) return null;
    params[name] = param as string | number | boolean;
  }
  return { code, params };
}

/** text[start] 是 `{` 時回傳對應 `}` 的下一個位置；跨過字串內的括號與跳脫。不成對回 -1。 */
function jsonObjectEnd(text: string, start: number): number {
  if (text[start] !== "{") return -1;
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let i = start; i < text.length; i += 1) {
    const ch = text[i];
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') inString = true;
    else if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) return i + 1;
    }
  }
  return -1;
}
