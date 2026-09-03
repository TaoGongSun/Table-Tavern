//! MVU 機制格式：解析模型吐出的 `<UpdateVariable>` JSON Patch，依欄位規則本地決定收不收。
//! 模型只說「這一幕變動多少」，加減／夾邊界／擲骰全在這裡做——模型算數字會幻覺，
//! 本地帳才是真相。容錯是紅線：格式壞的那一筆丟掉沿用舊值，絕不 panic、絕不中斷整批更新。

mod apply;
mod derive;
mod ledger;
mod parse;
mod rules;
mod tree;
mod triggers;
mod types;

#[cfg(test)]
mod test_support;

pub use ledger::{apply_block, append_log, read_ledger, Ledger};
pub use rules::rule_for_path;
pub use types::{Outcome, Record, RecordKind};
