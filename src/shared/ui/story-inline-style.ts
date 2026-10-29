// 卡片行內 style 照 ST 放行，但只收字色、底色、字型類：沒有會脫離文件流的屬性，字級限在 8–32px，蓋不住介面；
// 輸入只准數字參數的顏色函式，不准其他函式與跳脫，外連不出去；輸出另有這裡產生的 clamp()。
// 只看原始字串、由驗過的值重組，不走 CSSOM（瀏覽器會把顏色正規化成 rgb()，jsdom 與 WebKit 也不一致）。

// 跳脫（\75rl( 拼出 url(）、引號、註解、標記類字元出現在任何一處，整個 style 都不要
const FORBIDDEN = /[\\"'/<>&@{}]/;
// 顏色函式只收數字類參數；拿掉合格的之後還剩括號，代表有別的函式（url、image-set、expression…）
const COLOR_FN = /\b(?:rgba?|hsla?)\([\d.,%\sdeg]*\)/gi;
const HEX = /^#(?:[\da-f]{3,4}|[\da-f]{6}|[\da-f]{8})$/i;
// 具名色與關鍵字；不認得的字瀏覽器自己當無效
const WORD = /^[a-z]+$/i;

const COLOR_FN_WHOLE = new RegExp(`^${COLOR_FN.source}$`, "i");

function color(value: string): string | null {
  return HEX.test(value) || WORD.test(value) || COLOR_FN_WHOLE.test(value) ? value : null;
}

const pick = (pattern: RegExp) => (value: string) => (pattern.test(value) ? value : null);

// 字級限制在最後計算出的大小：逐層夾數值擋不住巢狀倍增（五層 2em 就到 512px），
// 所以輸出由這裡產生的 clamp()，外層已經被夾過，裡層再怎麼乘都出不了範圍
const FONT_SIZE = /^(?:\d+(?:\.\d+)?|\.\d+)(?:px|em|rem|%)$/i;
const ABSOLUTE_SIZE = /^(?:xx-small|x-small|small|medium|large|x-large|xx-large)$/i;
const RELATIVE_SIZE = new Map([
  ["larger", "1.2em"],
  ["smaller", "0.833em"],
]);
const clampSize = (size: string) => `clamp(8px, ${size}, 32px)`;

function fontSize(value: string): string | null {
  if (ABSOLUTE_SIZE.test(value)) return value;
  const relative = RELATIVE_SIZE.get(value.toLowerCase());
  if (relative) return clampSize(relative);
  return FONT_SIZE.test(value) ? clampSize(value) : null;
}

function textDecoration(value: string): string | null {
  const tokens = value.split(/\s+/);
  return tokens.length <= 4 && tokens.every((token) => HEX.test(token) || WORD.test(token)) ? value : null;
}

const RULES = new Map<string, (value: string) => string | null>([
  ["color", color],
  ["background-color", color],
  ["font-weight", pick(/^(?:normal|bold|bolder|lighter|[1-9]00)$/i)],
  ["font-style", pick(/^(?:normal|italic|oblique)$/i)],
  ["text-decoration", textDecoration],
  ["font-size", fontSize],
]);

/** 回傳可放行的 style 值；空字串＝整個屬性拿掉 */
export function sanitizeInlineStyle(raw: string): string {
  if (FORBIDDEN.test(raw) || /[()]/.test(raw.replace(COLOR_FN, ""))) return "";
  const kept: string[] = [];
  for (const part of raw.split(";")) {
    const colon = part.indexOf(":");
    if (colon < 0) continue;
    const property = part.slice(0, colon).trim().toLowerCase();
    // ST 卡常寫 !important；去掉後照常驗
    const value = part.slice(colon + 1).replace(/!\s*important\s*$/i, "").trim();
    const safe = value ? RULES.get(property)?.(value) : null;
    if (safe) kept.push(`${property}: ${safe}`);
  }
  return kept.join("; ");
}
