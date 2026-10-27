//! 桌面 MVU 引擎宿主（mvu-zod-semantics 規格 2 節）：zod 版 MVU 卡自帶的變數 schema 與共用 MVU 核心
//! （src/features/mvu-engine，打包成 gen/mvu-engine.js）在 QuickJS 裡跑。隔離：沒有網路與檔案 API、模組只准卡片的
//! mvu_zod（對應到墊片）、記憶體／堆疊／整次期限上限，觸限或出錯就丟掉 runtime 重建。其餘卡片腳本照舊不執行。

mod contract;
mod host;
#[cfg(test)]
mod parity_tests;
pub mod plain_opening;
mod runtime;
#[cfg(test)]
mod tests;
pub mod wiring;
#[cfg(test)]
mod wiring_tests;

#[cfg(test)]
use contract::{BatchOutput, CommandStatus, DetailedResult};
pub use contract::{
    Change, CommandResult, EngineFailure, FailureKind, Limits, Macros, SchemaScript, SeedBase,
};
pub use host::MvuEngine;

/// 測試用：引擎全 app 一份、全機名額 2，用到共享引擎的測試互相排隊，免得並行時互搶名額
#[cfg(test)]
pub static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
