//! seedrandom（ARC4，davidbau/seedrandom 3.x 預設演算法）的字串種子版，照網頁版 `seedrandom.ts`：
//! `{{pick}}` 用它，同一個種子與 ST 產生位元相同的序列。運算照 JS 的倍精度浮點做。

const WIDTH: f64 = 256.0;
const CHUNKS: usize = 6;
const MASK: usize = 255;

struct Arc4 {
    i: usize,
    j: usize,
    s: [usize; 256],
}

impl Arc4 {
    fn new(key: &[usize]) -> Self {
        let keys: &[usize] = if key.is_empty() { &[0] } else { key };
        let mut s = [0usize; 256];
        for (index, slot) in s.iter_mut().enumerate() {
            *slot = index;
        }
        let mut j = 0usize;
        for i in 0..256 {
            let t = s[i];
            j = MASK & (j + keys[i % keys.len()] + t);
            s[i] = s[j];
            s[j] = t;
        }
        let mut arc4 = Self { i: 0, j: 0, s };
        arc4.next(256);
        arc4
    }

    fn next(&mut self, count: usize) -> f64 {
        let (mut i, mut j) = (self.i, self.j);
        let s = &mut self.s;
        let mut r = 0.0;
        for _ in 0..count {
            i = MASK & (i + 1);
            let t = s[i];
            j = MASK & (j + t);
            s[i] = s[j];
            s[j] = t;
            r = r * WIDTH + s[MASK & (s[i] + s[j])] as f64;
        }
        self.i = i;
        self.j = j;
        r
    }
}

fn mix_key(seed: &str) -> Vec<usize> {
    let mut key: Vec<usize> = Vec::new();
    let mut smear: i64 = 0;
    for (j, unit) in seed.encode_utf16().enumerate() {
        let slot = MASK & j;
        let current = key.get(slot).copied().unwrap_or(0) as i64;
        smear ^= current * 19;
        let value = MASK & ((smear + i64::from(unit)) as usize);
        if slot < key.len() {
            key[slot] = value;
        } else {
            key.push(value);
        }
    }
    key
}

/// `seedrandom(seed)`：回傳 [0,1) 的產生器。
pub fn seedrandom(seed: &str) -> impl FnMut() -> f64 {
    let mut arc4 = Arc4::new(&mix_key(seed));
    let start_denom = WIDTH.powi(CHUNKS as i32);
    let significance = 2f64.powi(52);
    let overflow = significance * 2.0;
    move || {
        let mut n = arc4.next(CHUNKS);
        let mut d = start_denom;
        let mut x: u32 = 0;
        while n < significance {
            n = (n + f64::from(x)) * WIDTH;
            d *= WIDTH;
            x = arc4.next(1) as u32;
        }
        while n >= overflow {
            n /= 2.0;
            d /= 2.0;
            x >>= 1;
        }
        (n + f64::from(x)) / d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_published_arc4_vector() {
        assert_eq!(seedrandom("hello.")(), 0.9282578795792454);
    }
}
