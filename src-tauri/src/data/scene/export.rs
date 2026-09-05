use std::fs;
use std::path::Path;

use super::super::{DataResult, invalid_data, local_timestamp};
use super::super::paths::world_dir;
use super::super::state::read_state;
use super::transcript::{TranscriptEvent, TranscriptKind, read_transcript, transcript_path};



/// 把單一事件渲染成一行（或多行）Markdown，整桌／單場匯出共用同一份格式。
fn render_transcript_entry(event: &TranscriptEvent, english: bool) -> String {
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            if english {
                format!("**{}**: {}", event.speaker_name, event.text)
            } else {
                format!("**{}**：{}", event.speaker_name, event.text)
            }
        }
        TranscriptKind::Narration => {
            if event.text.is_empty() {
                "> ".to_owned()
            } else {
                event
                    .text
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        TranscriptKind::System => {
            if english {
                format!("*({})*", event.text)
            } else {
                format!("*（{}）*", event.text)
            }
        }
    }
}

/// 場景標題＋事件列表組成一段章節，整桌匯出把多段章節接起來。
fn render_scene_section(events: &[TranscriptEvent], heading: &str, english: bool) -> String {
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        heading.to_owned()
    } else {
        format!("{heading}\n\n{}", entries.join("\n\n"))
    }
}

pub fn export_transcript_markdown(root: &Path, world_id: &str, lang: &str) -> DataResult<String> {
    let world_name = read_state(root, world_id)?.name;
    let transcript_dir = world_dir(root, world_id)?.join("transcript");
    if !transcript_dir.is_dir() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let mut scenes = Vec::new();
    for entry in fs::read_dir(transcript_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if let Ok(scene) = stem.parse::<u64>() {
            scenes.push(scene);
        }
    }
    scenes.sort_unstable();
    scenes.dedup();
    if scenes.is_empty() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world_name} — Session Transcript\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 跑團紀錄\n\n匯出時間：{timestamp}")
    };
    let mut sections = Vec::new();
    for scene in scenes {
        let heading = if english {
            format!("## Scene {scene}")
        } else {
            format!("## 場景 {scene}")
        };
        let events = read_transcript(root, world_id, scene)?;
        sections.push(render_scene_section(&events, &heading, english));
    }

    Ok(format!("{title}\n\n{}\n", sections.join("\n\n")))
}

/// 匯出單一場景的紀錄，格式與整桌匯出一致，供「過去的場」單場匯出使用。
/// 場景不存在（無該檔）視為錯誤，避免誤匯出空白文件。
pub fn export_scene_markdown(
    root: &Path,
    world_id: &str,
    scene: u64,
    lang: &str,
) -> DataResult<String> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Err(invalid_data(format!("場景 {scene} 不存在")));
    }

    let world_name = read_state(root, world_id)?.name;
    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world_name} — Scene {scene}\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 場景 {scene}\n\n匯出時間：{timestamp}")
    };
    let events = read_transcript(root, world_id, scene)?;
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    Ok(format!("{title}\n\n{}\n", entries.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_transcript_markdown(root.path(), &world_id, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 跑團紀錄\n\n匯出時間："));
        assert!(zh.find("## 場景 0").unwrap() < zh.find("## 場景 1").unwrap());
        assert!(zh.contains("> 霧氣升起。\n> 港口安靜。"));
        assert!(zh.contains("*（第一幕開始）*"));
        assert!(zh.contains("**玩家**：我登上甲板。"));
        assert!(zh.contains("**船長**：我們啟航。"));

        let en = export_transcript_markdown(root.path(), &world_id, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Session Transcript\n\nExported: "));
        assert!(en.contains("## Scene 0"));
        assert!(en.contains("## Scene 1"));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(en.contains("*(第一幕開始)*"));
    }

    #[test]
    fn transcript_export_rejects_a_world_without_scenes() {
        let root = TestRoot::new("empty-transcript-export");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_transcript_markdown(root.path(), &world_id, "zh-TW").is_err());
    }

    #[test]
    fn scene_export_contains_only_that_scenes_events() {
        let root = TestRoot::new("scene-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_scene_markdown(root.path(), &world_id, 0, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 場景 0\n\n匯出時間："));
        assert!(zh.contains("> 霧氣升起。"));
        assert!(!zh.contains("船長"));

        let en = export_scene_markdown(root.path(), &world_id, 1, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Scene 1\n\nExported: "));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(!en.contains("霧氣升起"));
    }

    #[test]
    fn scene_export_rejects_a_missing_scene() {
        let root = TestRoot::new("scene-export-missing");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_scene_markdown(root.path(), &world_id, 0, "zh-TW").is_err());
    }

}
