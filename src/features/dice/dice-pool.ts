export type DiceIssue = "face" | "order" | "repeat" | "unknown";
export interface PoolDie {
  value: number;
  used: boolean;
  reported?: number[];
  issues: DiceIssue[];
}
export interface PoolRow {
  sides: number;
  dice: PoolDie[];
}

/** 固定池格式由 Rust 產生；前端只讀，兩個遊玩端共用。 */
export function inspectPool(pool: string, text: string) {
  const rows: PoolRow[] = [];
  let count = 0;
  if (pool.length > 512) return null;
  for (const line of pool.split("\n")) {
    const match = /^d(4|6|8|10|12|20|100):\[([0-9]+(?:,[0-9]+)*)\]$/.exec(line);
    if (!match) return null;
    const sides = Number(match[1]);
    const faces = match[2].split(",").map(Number);
    count += faces.length;
    if (
      rows.some((r) => r.sides === sides) ||
      count > 64 ||
      faces.some((n) => n < 1 || n > sides)
    )
      return null;
    rows.push({
      sides,
      dice: faces.map((value) => ({ value, used: false, issues: [] })),
    });
  }
  const next = new Map<number, number>();
  const unknown: string[] = [];
  let references = 0;
  // 括號可有可無：模型常寫 d20#2=9 或 (d4#1=1，d4#2=3)
  const pattern = /(?<![a-z0-9_])d\s*([0-9]+)\s*#\s*([0-9]+)\s*=\s*([0-9]+)/gi;
  for (const match of text.matchAll(pattern)) {
    references++;
    const [sides, index, value] = match.slice(1).map(Number);
    const die = rows.find((r) => r.sides === sides)?.dice[index - 1];
    if (!die) {
      unknown.push(match[0]);
      continue;
    }
    const issues = new Set(die.issues);
    if (die.used) issues.add("repeat");
    if (value !== die.value) {
      issues.add("face");
      die.reported = [...new Set([...(die.reported ?? []), value])];
    }
    const expected = next.get(sides) ?? 1;
    if (index !== expected) issues.add("order");
    else next.set(sides, expected + 1);
    die.used = true;
    die.issues = [...issues];
  }
  return { rows, references, unknown };
}
