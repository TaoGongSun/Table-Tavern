//! ST 巨集引擎（worldbook-st-trigger-parity 包 3）：照網頁版 `web/src/features/sillytavern/` 的
//! `macro-*.ts`、`substitute.ts`、`variables.ts`、`seedrandom.ts`（SillyTavern 06bde939）逐支移植，
//! 兩邊跑 `src/shared/contracts/st-macros/` 同一份案例對拍。求值分完整與中性兩種模式，代換同時回報
//! 「讀到私密來源」。組裝點的接線在 `crate::world_scan`（巨集環境、掃完的代換、落地）。

pub mod engine;
pub mod js_value;
pub mod library;
pub mod moment;
pub mod moment_parse;
pub mod parser;
pub mod seedrandom;
pub mod substitute;
pub mod variables;

#[cfg(test)]
mod mode_tests;
#[cfg(test)]
mod parity_tests;
