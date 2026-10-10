//! 世界書觸發（worldbook-st-trigger-parity）：照網頁版 `web/src/features/sillytavern/world-info-*.ts`
//! （SillyTavern 06bde939 `checkWorldInfo`）逐行移植的純函式，兩邊跑 `src/shared/contracts/world-info/` 同一份案例對拍。
//! 桌面版的接線（視角、放置、落地）在 `crate::world_scan`。

pub mod book_order;
pub mod entry;
pub mod js_semantics;
pub mod regex_key;
pub mod scan;
pub mod settings;
pub mod sort;
pub mod timed;

#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod source_tests;
