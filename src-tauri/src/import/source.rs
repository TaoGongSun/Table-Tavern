//! 開場白初始值（refactor-statusbar-skeleton 待問 3〔作者裁決 2026-10-03〕）：expand 讀玩家貼出的那則
//! 開場白，取自匯入原檔（receipts/sources.rs），不讀逐字稿。
use crate::receipts;
use regex::Regex;
use std::path::Path;

/// 玩家貼出的那則開場白原文（取自匯入原檔＋收據上記的序號，不讀逐字稿）；來源不完整、沒記序號或
/// 讀不到回 None。
pub fn chosen_opening(root: &Path, world_id: &str) -> Option<String> {
    let replays = receipts::import_replays(root, world_id).ok()??;
    let (replay, index) = replays
        .iter()
        .rev()
        .find_map(|replay| Some((replay, replay.opening.as_ref()?.2?)))?;
    let (_, openings) = super::card_openings(&replay.bytes)?;
    openings.get(index).cloned()
}

fn open_tag() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"<([A-Za-z_][\w.-]*)>").expect("open tag regex"))
}

/// 開場白裡、標籤也出現在條目文字的容器區塊原文（依開場白順序，每個標籤取第一個完整區塊）。
/// 展開介面條目時交給模型當初始值依據；一個都沒有回 None。
pub fn opening_blocks_for(opening: &str, entry_text: &str) -> Option<String> {
    let mut blocks: Vec<&str> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for found in open_tag().captures_iter(opening) {
        let tag = &found[1];
        if seen.iter().any(|known| known == tag) || !entry_text.contains(&format!("<{tag}>")) {
            continue;
        }
        let start = found.get(0).expect("match").start();
        let close = format!("</{tag}>");
        let Some(end) = opening[start..].find(&close) else {
            continue;
        };
        seen.push(tag.to_owned());
        blocks.push(&opening[start..start + end + close.len()]);
    }
    (!blocks.is_empty()).then(|| blocks.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;

    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn root(label: &str) -> Root {
        Root(std::env::temp_dir().join(format!(
            "table-tavern-import-source-{label}-{}",
            data::new_id()
        )))
    }

    const CARD: &str = r#"{"data":{"name":"驛站","first_mes":"<maintext>開場</maintext>","alternate_greetings":["<Status_block>\n地点: 後院\n</Status_block>"],"character_book":{"entries":[{"keys":[],"content":"設定","comment":"驛站","enabled":true}]}}}"#;

    #[test]
    fn chosen_opening_follows_the_receipt_and_undo() {
        let root = root("chosen");
        let world_id = data::create_world(&root.0, "驛站").unwrap();
        assert_eq!(chosen_opening(&root.0, &world_id), None);
        let source = super::super::import_worldbook_file(
            &root.0,
            &world_id,
            CARD.as_bytes(),
            "驛站",
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap()
        .source;
        assert!(source.is_some());
        super::super::post_opening_text(
            &root.0,
            &world_id,
            0,
            "t1",
            "開場",
            "zh-TW",
            Some(1),
            source.as_deref(),
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap();
        assert_eq!(
            chosen_opening(&root.0, &world_id).as_deref(),
            Some("<Status_block>\n地点: 後院\n</Status_block>")
        );
        crate::receipts::undo_last_import(
            &root.0,
            &world_id,
            &crate::data::test_exclusive(&world_id),
        )
        .unwrap();
        assert_eq!(chosen_opening(&root.0, &world_id), None);
    }

    #[test]
    fn opening_blocks_only_take_containers_the_entry_mentions() {
        let opening = "<maintext>開場</maintext>\n<Status_block>\n地点: 前院\n</Status_block>\n<Status_block>二</Status_block>";
        assert_eq!(
            opening_blocks_for(opening, "格式：<Status_block>…</Status_block>").as_deref(),
            Some("<Status_block>\n地点: 前院\n</Status_block>")
        );
        assert_eq!(opening_blocks_for(opening, "沒有容器"), None);
    }

    #[test]
    fn invalid_source_file_names_are_rejected() {
        let root = root("names");
        let world_id = data::create_world(&root.0, "驛站").unwrap();
        for name in [
            "../x.png",
            "import-source-.png",
            "import-source-1.exe",
            "source-card.png",
        ] {
            assert!(data::import_source_file_path(&root.0, &world_id, name).is_err());
        }
    }
}
