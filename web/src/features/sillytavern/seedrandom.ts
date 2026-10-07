// seedrandom（ARC4，davidbau/seedrandom 3.x 預設演算法）的字串種子版，ST 的 {{pick}} 用它：
// 同一個種子跟 ST 產生位元相同的序列。只移植 {{pick}} 用到的部分（字串種子、不混入熵池）。
const WIDTH = 256;
const CHUNKS = 6;
const START_DENOM = WIDTH ** CHUNKS;
const SIGNIFICANCE = 2 ** 52;
const OVERFLOW = SIGNIFICANCE * 2;
const MASK = WIDTH - 1;

class Arc4 {
  private i = 0;
  private j = 0;
  private readonly s: number[] = [];

  constructor(key: number[]) {
    const keys = key.length ? key : [0];
    const s = this.s;
    for (let i = 0; i < WIDTH; i++) s[i] = i;
    let j = 0;
    for (let i = 0; i < WIDTH; i++) {
      const t = s[i];
      j = MASK & (j + keys[i % keys.length] + t);
      s[i] = s[j];
      s[j] = t;
    }
    this.next(WIDTH);
  }

  next(count: number): number {
    const s = this.s;
    let { i, j } = this;
    let r = 0;
    for (let left = count; left > 0; left--) {
      i = MASK & (i + 1);
      const t = s[i];
      j = MASK & (j + t);
      s[i] = s[j];
      s[j] = t;
      r = r * WIDTH + s[MASK & (s[i] + s[j])];
    }
    this.i = i;
    this.j = j;
    return r;
  }
}

function mixKey(seed: string): number[] {
  const key: number[] = [];
  let smear = 0;
  for (let j = 0; j < seed.length; j++) {
    smear ^= ((key[MASK & j] ?? 0) * 19) | 0;
    key[MASK & j] = MASK & (smear + seed.charCodeAt(j));
  }
  return key;
}

/** `seedrandom(seed)`：回傳 [0,1) 的產生器。 */
export function seedrandom(seed: string): () => number {
  const arc4 = new Arc4(mixKey(seed));
  return () => {
    let n = arc4.next(CHUNKS);
    let d = START_DENOM;
    let x = 0;
    while (n < SIGNIFICANCE) {
      n = (n + x) * WIDTH;
      d *= WIDTH;
      x = arc4.next(1);
    }
    while (n >= OVERFLOW) {
      n /= 2;
      d /= 2;
      x >>>= 1;
    }
    return (n + x) / d;
  };
}
