//! 桌面 MVU 引擎宿主（mvu-zod-semantics 規格 2 節）：zod 版 MVU 卡自帶的變數 schema 與共用 MVU 核心
//! （src/features/mvu-engine，打包成 gen/mvu-engine.js）在 QuickJS 裡跑。隔離：沒有網路與檔案 API、模組只准卡片的
//! mvu_zod（對應到墊片）、記憶體／堆疊／整次期限上限，觸限或出錯就丟掉 runtime 重建。其餘卡片腳本照舊不執行。

mod contract;
mod host;
mod runtime;
#[cfg(test)]
mod tests;

pub use contract::{
    BatchOutput, Change, CommandResult, CommandStatus, DetailedResult, EngineFailure, FailureKind,
    InitOutput, Limits, Macros, SchemaScript,
};
pub use host::MvuEngine;
