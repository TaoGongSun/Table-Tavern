// 骰式建議：從 GM 文字挑玩家能擲的算式。桌面與房間共用，規則同算式解析（顆數 1–100、面數 2–1000、加值 ±9999）。
const FORMULA = /([0-9]*)d([0-9]+)(?:\s*([+-])\s*([0-9]+))?/gi;
const WORD = /[A-Za-z0-9_]/;
const OPERATOR = /[+\-*/!]/;
const CLAUSE_END = /[。！？!?；;，,\n]/;
const SENTENCE_END = /[。！？!?\n]/;
const MAX_CHARS = 32768;
const MAX_OPTIONS = 8;

const isWord = (ch: string | undefined) => ch !== undefined && WORD.test(ch);

function canonical(c: RegExpExecArray): string | null {
  const count = c[1] ? Number(c[1]) : 1;
  const sides = Number(c[2]);
  const size = c[4] ? Number(c[4]) : 0;
  const modifier = c[3] === "-" ? -size : size;
  if (count < 1 || count > 100 || sides < 2 || sides > 1000 || Math.abs(modifier) > 9999) return null;
  return `${count}d${sides}${modifier > 0 ? `+${modifier}` : modifier < 0 ? modifier : ""}`;
}

/** 這個位置所在的那一段（到 `stop` 為界） */
function around(text: string, from: number, to: number, stop: RegExp): string {
  let start = from;
  while (start > 0 && !stop.test(text[start - 1])) start--;
  let end = to;
  while (end < text.length && !stop.test(text[end])) end++;
  return text.slice(start, end);
}

export interface Suggestion {
  value: string;
  /** 那句點名別的角色、沒點自己：是別人該擲的 */
  other: boolean;
}

/** 算式的歸屬：小句點名就看小句；沒有再看整句，只點別人才算別人的，其餘算沒指定 */
function rankOf(clause: string, sentence: string, name: string, others: string[]): 0 | 1 | 2 {
  const of = (scope: string) => ({
    mine: !!name && scope.includes(name),
    theirs: others.some((other) => scope.includes(other)),
  });
  const near = of(clause);
  if (near.mine) return 0;
  if (near.theirs) return 2;
  const wide = of(sentence);
  return wide.theirs && !wide.mine ? 2 : 1;
}

/** 擲骰紀錄（`名字 2d6: [5, 1] = 6`、`1d20+5: [7] +5 = 12`）裡的算式 */
export const ROLLED_FORMULA = /(?:^| )(\d+d\d+(?:[+-]\d+)?): \[[^\]]*\](?: [+-]\d+)? = -?\d+$/;
export const rolledFormula = (text: string) => ROLLED_FORMULA.exec(text)?.[1];

/**
 * 自己的排最前、沒指定的其次、別人的最後，多人同句點名時才不會點到別人的加值。`others`：別的角色卡名。
 * `answered`：這則 GM 回應的那幾次擲骰；GM 常回頭提到剛擲的算式（「2d6 不是命中骰」），同級裡排到後面，免得主鈕停在舊算式。
 */
export function rankSuggestions(
  raw: string,
  name = "",
  others: string[] = [],
  answered: string[] = [],
): Suggestion[] {
  // 模型常寫全形加減號
  const text = Array.from(raw)
    .slice(0, MAX_CHARS)
    .join("")
    .replace(/＋/g, "+")
    .replace(/[－−]/g, "-");
  const found: Array<{ value: string; rank: 0 | 1 | 2 }> = [];
  const who = name.trim();
  const rest = others.map((other) => other.trim()).filter((other) => other && other !== who);
  for (const c of text.matchAll(FORMULA)) {
    const from = c.index;
    const to = from + c[0].length;
    // Markdown 粗體不算運算子；`**` 另一側緊接算式字元（1d6**2）才是乘冪
    let left = text.slice(0, from);
    if (left.endsWith("**") && !isWord(left[left.length - 3])) left = left.slice(0, -2);
    let right = text.slice(to);
    if (right.startsWith("**") && !isWord(right[2])) right = right.slice(2);
    const before = left.trimEnd();
    const after = right.trimStart();
    if (
      isWord(left[left.length - 1]) ||
      isWord(right[0]) ||
      (before && OPERATOR.test(before[before.length - 1])) ||
      (after && OPERATOR.test(after[0])) ||
      after.startsWith("#")
    )
      continue;
    const value = canonical(c);
    if (!value) continue;
    const rank = rankOf(
      around(text, from, to, CLAUSE_END),
      around(text, from, to, SENTENCE_END),
      who,
      rest,
    );
    const seen = found.find((x) => x.value === value);
    if (seen) {
      seen.rank = Math.min(seen.rank, rank) as 0 | 1 | 2;
      continue;
    }
    found.push({ value, rank });
    if (found.length === MAX_OPTIONS) break;
  }
  const old = (x: { value: string }) => answered.includes(x.value);
  return [0, 1, 2].flatMap((rank) => {
    const same = found.filter((x) => x.rank === rank);
    return [...same.filter((x) => !old(x)), ...same.filter(old)].map((x) => ({
      value: x.value,
      other: rank === 2,
    }));
  });
}
