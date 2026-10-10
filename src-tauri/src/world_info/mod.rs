//! 世界書觸發（worldbook-st-trigger-parity）：照網頁版 `web/src/features/sillytavern/world-info-*.ts`
//! （SillyTavern 06bde939 `checkWorldInfo`）逐行移植的純函式，兩邊跑 `src/shared/contracts/world-info/` 同一份案例對拍。
//! 組裝點接線在包 5a；在那之前只有測試用到，所以先放寬 dead_code。
#![cfg_attr(not(test), allow(dead_code))]

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
