//! ST 的排序是 `Array.prototype.sort`（穩定）。條目的 `order` 一律是有限數字（缺欄或不是數字補 100，
//! 見 `entry.rs`），比較子 `b.order - a.order` 是全序，所以直接用標準庫的穩定排序，不必移植 V8 的 TimSort。

use std::cmp::Ordering;

use super::entry::WiEntry;

/// ST `sortFn`：`b.order - a.order`。
pub fn compare_order(a: &WiEntry, b: &WiEntry) -> f64 {
    b.order - a.order
}

/// 條目照 ST `sortFn` 排好（掃描的輸入順序）。
pub fn sort_entries(entries: &mut [WiEntry]) {
    stable_sort(entries, &compare_order);
}

/// 穩定排序；比較子回傳值照 JS 解讀（< 0：a 在前；0 相等），**須為全序且不回 NaN**——標準庫的
/// `sort_by` 遇到不全序可能 panic。
pub fn stable_sort<T>(items: &mut [T], compare: &dyn Fn(&T, &T) -> f64) {
    items.sort_by(|a, b| {
        let diff = compare(a, b);
        debug_assert!(!diff.is_nan(), "比較子回了 NaN");
        diff.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ties_keep_their_order() {
        let mut items = vec![(0, 1.0), (1, 3.0), (2, 3.0), (3, 1.0)];
        stable_sort(&mut items, &|a: &(i32, f64), b: &(i32, f64)| b.1 - a.1);
        assert_eq!(
            items.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            [1, 2, 0, 3]
        );
    }
}
