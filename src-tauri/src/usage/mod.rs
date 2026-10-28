//! 快取／花費紀錄：log 逐次寫 JSONL，report 彙總給設定頁，floor 認出各來源自帶的快取底線。

pub(crate) mod floor;
pub(crate) mod log;
pub(crate) mod report;
