//! 初始 VN 卡（plans/vn-mode-pkg9.md）：ST V2 卡容器＋`data.extensions.table_tavern.vn`。偵測、驗證夾斷、
//! id 重對應、整桌原子建桌與反向匯出。
mod art;
mod export;
mod materialize;
mod remap;
mod schema;

#[cfg(test)]
mod tests;

pub use export::export_vn_card;
pub(crate) use export::{book_entries, origin_json};
pub use materialize::{import_vn_card, VnCardImported};
pub(crate) use schema::NoticeKind as CardNoticeKind;
pub use schema::VnCardProbe;
pub(crate) use schema::{bible_from, origin_from, probe};
