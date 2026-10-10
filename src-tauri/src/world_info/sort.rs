//! ST 的排序都是 V8 `Array.prototype.sort`；order 混了非數字時比較子（`b.order - a.order`，NaN 當相等）
//! 不是全序，結果取決於排序演算法本身。所以這裡忠實移植 V8 的 TimSort（third_party/v8/builtins/array-sort.tq：
//! 偵測 run＋二分插入補到 minrun、run 堆疊不變式、gallop 合併），不用 Rust 標準庫的排序。

use super::entry::WiEntry;
use super::js_semantics::to_number;

const MIN_GALLOP_WINS: usize = 7;

/// ST `sortFn`：`b.order - a.order`，NaN 回 0。
pub fn compare_order(a: &WiEntry, b: &WiEntry) -> f64 {
    let diff = to_number(b.order.as_ref()) - to_number(a.order.as_ref());
    if diff.is_nan() {
        0.0
    } else {
        diff
    }
}

/// 條目照 ST `sortFn` 排好（掃描的輸入順序）。
pub fn sort_entries(entries: &mut [WiEntry]) {
    v8_sort(entries, &compare_order);
}

/// V8 `Array.prototype.sort(comparefn)`；比較子回傳值照 JS 解讀（< 0：a 在前；NaN 當 0）。
pub fn v8_sort<T: Clone>(items: &mut [T], compare: &dyn Fn(&T, &T) -> f64) {
    if items.len() < 2 {
        return;
    }
    let mut state = SortState {
        work: items.to_vec(),
        compare,
        runs: Vec::new(),
        min_gallop: MIN_GALLOP_WINS,
    };
    state.tim_sort();
    items.clone_from_slice(&state.work);
}

struct SortState<'a, T> {
    work: Vec<T>,
    compare: &'a dyn Fn(&T, &T) -> f64,
    /// (base, length)
    runs: Vec<(usize, usize)>,
    min_gallop: usize,
}

fn min_run_length(mut n: usize) -> usize {
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}

impl<T: Clone> SortState<'_, T> {
    fn cmp(&self, a: &T, b: &T) -> f64 {
        let order = (self.compare)(a, b);
        if order.is_nan() {
            0.0
        } else {
            order
        }
    }

    fn tim_sort(&mut self) {
        let length = self.work.len();
        // V8：長度 < 8 直接從第 1 個元素起二分插入，不偵測 run
        if length < 8 {
            self.binary_insertion_sort(0, 1, length);
            return;
        }
        let mut remaining = length;
        let mut low = 0;
        let min_run = min_run_length(remaining);
        while remaining != 0 {
            let mut run = self.count_and_make_run(low, low + remaining);
            if run < min_run {
                let forced = min_run.min(remaining);
                self.binary_insertion_sort(low, low + run, low + forced);
                run = forced;
            }
            self.runs.push((low, run));
            self.merge_collapse();
            low += run;
            remaining -= run;
        }
        self.merge_force_collapse();
    }

    fn count_and_make_run(&mut self, low_arg: usize, high: usize) -> usize {
        let low = low_arg + 1;
        if low == high {
            return 1;
        }
        let mut run = 2;
        let mut order = self.cmp(&self.work[low], &self.work[low - 1]);
        let descending = order < 0.0;
        let mut previous = low;
        for index in low + 1..high {
            order = self.cmp(&self.work[index], &self.work[previous]);
            if descending {
                if order >= 0.0 {
                    break;
                }
            } else if order < 0.0 {
                break;
            }
            previous = index;
            run += 1;
        }
        if descending {
            self.work[low_arg..low_arg + run].reverse();
        }
        run
    }

    fn binary_insertion_sort(&mut self, low: usize, start_arg: usize, high: usize) {
        let mut start = if low == start_arg {
            start_arg + 1
        } else {
            start_arg
        };
        while start < high {
            let (mut left, mut right) = (low, start);
            let pivot = self.work[start].clone();
            while left < right {
                let mid = left + ((right - left) >> 1);
                if self.cmp(&pivot, &self.work[mid]) < 0.0 {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            let mut p = start;
            while p > left {
                self.work[p] = self.work[p - 1].clone();
                p -= 1;
            }
            self.work[left] = pivot;
            start += 1;
        }
    }

    fn run_len(&self, index: usize) -> usize {
        self.runs[index].1
    }

    fn invariant(&self, n: usize) -> bool {
        if n < 2 {
            return true;
        }
        self.run_len(n - 2) > self.run_len(n - 1) + self.run_len(n)
    }

    fn merge_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if !self.invariant(n + 1) || !self.invariant(n) {
                if n > 0 && self.run_len(n - 1) < self.run_len(n + 1) {
                    n -= 1;
                }
                self.merge_at(n);
            } else if self.run_len(n) <= self.run_len(n + 1) {
                self.merge_at(n);
            } else {
                break;
            }
        }
    }

    fn merge_force_collapse(&mut self) {
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if n > 0 && self.run_len(n - 1) < self.run_len(n + 1) {
                n -= 1;
            }
            self.merge_at(n);
        }
    }

    fn merge_at(&mut self, i: usize) {
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];
        self.runs[i].1 = length_a + length_b;
        self.runs.remove(i + 1);

        let key_right = self.work[base_b].clone();
        let k = gallop_right(self, Side::Work, &key_right, base_a, length_a, 0);
        base_a += k;
        length_a -= k;
        if length_a == 0 {
            return;
        }
        let key_left = self.work[base_a + length_a - 1].clone();
        length_b = gallop_left(self, Side::Work, &key_left, base_b, length_b, length_b - 1);
        if length_b == 0 {
            return;
        }
        if length_a <= length_b {
            self.merge_low(base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(base_a, length_a, base_b, length_b);
        }
    }

    fn merge_low(&mut self, base_a: usize, length_a: usize, base_b: usize, length_b: usize) {
        let temp: Vec<T> = self.work[base_a..base_a + length_a].to_vec();
        let (mut length_a, mut length_b) = (length_a, length_b);
        let mut dest = base_a;
        let mut cursor_temp = 0;
        let mut cursor_b = base_b;
        self.work[dest] = self.work[cursor_b].clone();
        dest += 1;
        cursor_b += 1;

        enum Exit {
            Succeed,
            CopyB,
        }
        let exit = 'merge: {
            length_b -= 1;
            if length_b == 0 {
                break 'merge Exit::Succeed;
            }
            if length_a == 1 {
                break 'merge Exit::CopyB;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let mut wins_a = 0;
                let mut wins_b = 0;
                loop {
                    if self.cmp(&self.work[cursor_b], &temp[cursor_temp]) < 0.0 {
                        self.work[dest] = self.work[cursor_b].clone();
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[dest] = temp[cursor_temp].clone();
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 1 {
                            break 'merge Exit::CopyB;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;

                    let key = self.work[cursor_b].clone();
                    wins_a = gallop_right(self, Side::Temp(&temp), &key, cursor_temp, length_a, 0);
                    if wins_a > 0 {
                        for offset in 0..wins_a {
                            self.work[dest + offset] = temp[cursor_temp + offset].clone();
                        }
                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;
                        if length_a == 1 {
                            break 'merge Exit::CopyB;
                        }
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest] = self.work[cursor_b].clone();
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;
                    if length_b == 0 {
                        break 'merge Exit::Succeed;
                    }

                    let key = temp[cursor_temp].clone();
                    wins_b = gallop_left(self, Side::Work, &key, cursor_b, length_b, 0);
                    if wins_b > 0 {
                        for offset in 0..wins_b {
                            self.work[dest + offset] = self.work[cursor_b + offset].clone();
                        }
                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[dest] = temp[cursor_temp].clone();
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;
                    if length_a == 1 {
                        break 'merge Exit::CopyB;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match exit {
            Exit::Succeed => {
                for offset in 0..length_a {
                    self.work[dest + offset] = temp[cursor_temp + offset].clone();
                }
            }
            Exit::CopyB => {
                for offset in 0..length_b {
                    self.work[dest + offset] = self.work[cursor_b + offset].clone();
                }
                self.work[dest + length_b] = temp[cursor_temp].clone();
            }
        }
    }

    fn merge_high(&mut self, base_a: usize, length_a: usize, base_b: usize, length_b: usize) {
        let temp: Vec<T> = self.work[base_b..base_b + length_b].to_vec();
        let (mut length_a, mut length_b) = (length_a, length_b);
        // 游標可能退到 base 之前一格，用有號整數
        let mut dest = (base_b + length_b - 1) as isize;
        let mut cursor_temp = length_b as isize - 1;
        let mut cursor_a = (base_a + length_a - 1) as isize;
        let at = |index: isize| index as usize;
        self.work[at(dest)] = self.work[at(cursor_a)].clone();
        dest -= 1;
        cursor_a -= 1;

        enum Exit {
            Succeed,
            CopyA,
        }
        let exit = 'merge: {
            length_a -= 1;
            if length_a == 0 {
                break 'merge Exit::Succeed;
            }
            if length_b == 1 {
                break 'merge Exit::CopyA;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let mut wins_a = 0;
                let mut wins_b = 0;
                loop {
                    if self.cmp(&temp[at(cursor_temp)], &self.work[at(cursor_a)]) < 0.0 {
                        self.work[at(dest)] = self.work[at(cursor_a)].clone();
                        dest -= 1;
                        cursor_a -= 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    } else {
                        self.work[at(dest)] = temp[at(cursor_temp)].clone();
                        dest -= 1;
                        cursor_temp -= 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 1 {
                            break 'merge Exit::CopyA;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP_WINS || wins_b >= MIN_GALLOP_WINS || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;

                    let key = temp[at(cursor_temp)].clone();
                    let k = gallop_right(self, Side::Work, &key, base_a, length_a, length_a - 1);
                    wins_a = length_a - k;
                    if wins_a > 0 {
                        dest -= wins_a as isize;
                        cursor_a -= wins_a as isize;
                        for offset in (0..wins_a).rev() {
                            self.work[at(dest + 1) + offset] =
                                self.work[at(cursor_a + 1) + offset].clone();
                        }
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[at(dest)] = temp[at(cursor_temp)].clone();
                    dest -= 1;
                    cursor_temp -= 1;
                    length_b -= 1;
                    if length_b == 1 {
                        break 'merge Exit::CopyA;
                    }

                    let key = self.work[at(cursor_a)].clone();
                    let k = gallop_left(self, Side::Temp(&temp), &key, 0, length_b, length_b - 1);
                    wins_b = length_b - k;
                    if wins_b > 0 {
                        dest -= wins_b as isize;
                        cursor_temp -= wins_b as isize;
                        for offset in 0..wins_b {
                            self.work[at(dest + 1) + offset] =
                                temp[at(cursor_temp + 1) + offset].clone();
                        }
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge Exit::CopyA;
                        }
                        if length_b == 0 {
                            break 'merge Exit::Succeed;
                        }
                    }
                    self.work[at(dest)] = self.work[at(cursor_a)].clone();
                    dest -= 1;
                    cursor_a -= 1;
                    length_a -= 1;
                    if length_a == 0 {
                        break 'merge Exit::Succeed;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        match exit {
            Exit::Succeed => {
                if length_b > 0 {
                    let start = at(dest) + 1 - length_b;
                    for offset in 0..length_b {
                        self.work[start + offset] = temp[offset].clone();
                    }
                }
            }
            Exit::CopyA => {
                dest -= length_a as isize;
                cursor_a -= length_a as isize;
                for offset in (0..length_a).rev() {
                    self.work[at(dest + 1) + offset] = self.work[at(cursor_a + 1) + offset].clone();
                }
                self.work[at(dest)] = temp[at(cursor_temp)].clone();
            }
        }
    }
}

/// gallop 在哪個陣列上找：工作陣列或合併用的暫存。
enum Side<'t, T> {
    Work,
    Temp(&'t [T]),
}

fn element<'s, T: Clone>(
    state: &'s SortState<'_, T>,
    side: &'s Side<'_, T>,
    index: usize,
) -> &'s T {
    match side {
        Side::Work => &state.work[index],
        Side::Temp(temp) => &temp[index],
    }
}

/// 找 key 在 run 裡的最左插入點（相等的排在 key 後面）。
fn gallop_left<T: Clone>(
    state: &SortState<'_, T>,
    side: Side<'_, T>,
    key: &T,
    base: usize,
    length: usize,
    hint: usize,
) -> usize {
    let mut last_ofs: isize = 0;
    let mut offset: isize = 1;
    let hint_i = hint as isize;
    let order = state.cmp(element(state, &side, base + hint), key);
    if order < 0.0 {
        let max_ofs = (length - hint) as isize;
        while offset < max_ofs {
            if state.cmp(element(state, &side, base + hint + offset as usize), key) >= 0.0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        last_ofs += hint_i;
        offset += hint_i;
    } else {
        let max_ofs = hint_i + 1;
        while offset < max_ofs {
            if state.cmp(
                element(state, &side, base + (hint_i - offset) as usize),
                key,
            ) < 0.0
            {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint_i - offset;
        offset = hint_i - tmp;
    }
    last_ofs += 1;
    while last_ofs < offset {
        let m = last_ofs + ((offset - last_ofs) >> 1);
        if state.cmp(element(state, &side, base + m as usize), key) < 0.0 {
            last_ofs = m + 1;
        } else {
            offset = m;
        }
    }
    offset as usize
}

/// 找 key 在 run 裡的最右插入點（相等的排在 key 前面）。
fn gallop_right<T: Clone>(
    state: &SortState<'_, T>,
    side: Side<'_, T>,
    key: &T,
    base: usize,
    length: usize,
    hint: usize,
) -> usize {
    let mut last_ofs: isize = 0;
    let mut offset: isize = 1;
    let hint_i = hint as isize;
    let order = state.cmp(key, element(state, &side, base + hint));
    if order < 0.0 {
        let max_ofs = hint_i + 1;
        while offset < max_ofs {
            if state.cmp(
                key,
                element(state, &side, base + (hint_i - offset) as usize),
            ) >= 0.0
            {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint_i - offset;
        offset = hint_i - tmp;
    } else {
        let max_ofs = (length - hint) as isize;
        while offset < max_ofs {
            if state.cmp(key, element(state, &side, base + hint + offset as usize)) < 0.0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        last_ofs += hint_i;
        offset += hint_i;
    }
    last_ofs += 1;
    while last_ofs < offset {
        let m = last_ofs + ((offset - last_ofs) >> 1);
        if state.cmp(key, element(state, &side, base + m as usize)) < 0.0 {
            offset = m;
        } else {
            last_ofs = m + 1;
        }
    }
    offset as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_sort_like_v8() {
        let mut values: Vec<i64> = (0..500).map(|index| (index * 7919) % 101).collect();
        v8_sort(&mut values, &|a: &i64, b: &i64| (*a - *b) as f64);
        assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
