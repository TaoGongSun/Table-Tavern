//! VN 作品包 `.ttvn`（plans/vn-mode-pkg8.md）：故事完成後匯出成免模型的演出成品、匯入成重播桌。
mod archive;
mod export;
mod format;
mod materialize;
mod opaque;
mod settings;
mod validate;

pub use export::{export_work, WorkExported};
pub use format::ImportMode;
pub use materialize::{import_work, probe_work, WorkImported, WorkProbe};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
