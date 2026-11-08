// 玩家擲骰（room-player-dice）：算式嚴格解析與擲面。規則同單人（src-tauri/src/dice.rs），結果文字格式也一樣。
export interface Formula {
  count: number;
  sides: number;
  modifier: number;
}

const WHOLE = /^([0-9]*)d([0-9]+)(?:\s*([+-])\s*([0-9]+))?$/i;

export function parseFormula(raw: string): Formula | null {
  if (raw.length > 128) return null;
  const c = WHOLE.exec(raw.trim());
  if (!c) return null;
  const count = c[1] ? Number(c[1]) : 1;
  const sides = Number(c[2]);
  const size = c[4] ? Number(c[4]) : 0;
  const modifier = c[3] === "-" ? -size : size;
  return count >= 1 && count <= 100 && sides >= 2 && sides <= 1000 && Math.abs(modifier) <= 9999
    ? { count, sides, modifier }
    : null;
}

/** 拒絕取樣：丟掉 2^32 尾端除不盡的值，面值才均勻 */
function face(rng: (bytes: number) => Uint8Array, sides: number): number {
  const range = 2 ** 32;
  const limit = range - (range % sides);
  for (;;) {
    const [a, b, c, d] = rng(4);
    const value = a * 2 ** 24 + (b << 16) + (c << 8) + d;
    if (value < limit) return (value % sides) + 1;
  }
}

/** 擲一次並組結果文字，例：`沃恩 2d6+3: [4, 2] +3 = 9` */
export function rollText(name: string, f: Formula, rng: (bytes: number) => Uint8Array): string {
  const faces = Array.from({ length: f.count }, () => face(rng, f.sides));
  const signed = f.modifier > 0 ? `+${f.modifier}` : f.modifier < 0 ? `${f.modifier}` : "";
  const total = faces.reduce((sum, n) => sum + n, f.modifier);
  return `${name} ${f.count}d${f.sides}${signed}: [${faces.join(", ")}]${signed && ` ${signed}`} = ${total}`;
}
