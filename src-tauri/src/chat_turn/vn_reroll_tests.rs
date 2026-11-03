use super::*;
use crate::commands::character_card;

struct Fake {
    reply: String,
    sent: Vec<transport::ChatMessage>,
    /// 模型回來前玩家先往下走：在這桌追加一句
    meanwhile: Option<(std::path::PathBuf, String)>,
}

impl SummaryCaller for Fake {
    async fn call(&mut self, messages: Vec<transport::ChatMessage>) -> Result<String, String> {
        self.sent = messages;
        if let Some((root, world_id)) = &self.meanwhile {
            let player = event(data::TranscriptKind::Player, "我選甲", None);
            data::append_transcript(root, world_id, 0, &player).unwrap();
        }
        Ok(self.reply.clone())
    }
}

fn path() -> PathLimits {
    PathLimits {
        budget: None,
        model: String::new(),
        limits: Default::default(),
    }
}

fn event(kind: data::TranscriptKind, text: &str, raw: Option<&str>) -> data::TranscriptEvent {
    data::TranscriptEvent {
        ts: "2026-11-01T00:00:00Z".to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind,
        text: text.to_owned(),
        raw: raw.map(str::to_owned),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    }
}

/// world 目錄裡除了逐字稿以外的每個檔（state.json、世界書落地檔…）
fn other_files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "transcript") {
                files.extend(other_files(&path));
            }
        } else {
            files.push((path.clone(), std::fs::read(&path).unwrap()));
        }
    }
    files.sort();
    files
}

fn table(
    label: &str,
    kind: data::TableKind,
    last: data::TranscriptEvent,
) -> (std::path::PathBuf, String) {
    let root = crate::smart_free::test_root(label);
    let world_id = data::create_world_of_kind(&root, "港口", kind).unwrap();
    data::write_character(&root, &world_id, &character_card(&data::new_id(), "艾琳")).unwrap();
    let player = event(data::TranscriptKind::Player, "你好", None);
    data::append_transcript(&root, &world_id, 0, &player).unwrap();
    data::append_transcript(&root, &world_id, 0, &last).unwrap();
    (root, world_id)
}

/// 只改寫那則旁白的選項區塊（text 與 raw），事件 id 不變；其餘檔逐位元相同；不經 lane
#[tokio::test]
async fn reroll_rewrites_only_the_choices_block() {
    let text = "走吧。\n[choices]\n- 甲\n- 乙";
    let raw = format!("{text}\n```state\ntime: 夜\n```\n下一位：艾琳");
    let narration = event(data::TranscriptKind::Narration, text, Some(&raw));
    let (root, world_id) = table("vn-reroll", data::TableKind::Vn, narration);
    let dir = root.join("worlds").join(&world_id);
    let before = other_files(&dir);
    let old_id = data::read_transcript(&root, &world_id, 0).unwrap()[1]
        .id
        .clone();
    let mut fake = Fake {
        reply: "[choices]\n- 丙\n- 丁".to_owned(),
        sent: Vec::new(),
        meanwhile: None,
    };
    let options = reroll_choices(&root, &world_id, "zh-TW", &path(), &mut fake)
        .await
        .unwrap();
    assert_eq!(options, ["丙", "丁"]);
    let last = fake.sent.last().unwrap();
    assert!(last.content.contains("- 甲") && last.content.contains("[choices]"));
    assert!(fake.sent[0].content.contains("視覺小說劇本格式"));
    let events = data::read_transcript(&root, &world_id, 0).unwrap();
    assert_eq!(events[1].id, old_id);
    assert_eq!(events[1].text, "走吧。\n[choices]\n- 丙\n- 丁");
    assert_eq!(
        events[1].raw.as_deref(),
        Some("走吧。\n[choices]\n- 丙\n- 丁\n```state\ntime: 夜\n```\n下一位：艾琳")
    );
    assert_eq!(other_files(&dir), before);
    assert!(!data::lanes_path(&root, &world_id).unwrap().exists());
}

/// 前置不符或模型沒給出選項：回錯、逐字稿不動
#[tokio::test]
async fn reroll_rejects_without_writing() {
    let with_choices = || {
        event(
            data::TranscriptKind::Narration,
            "走吧。\n[choices]\n- 甲\n- 乙",
            None,
        )
    };
    let cases = [
        (
            "vn-reroll-chat",
            data::TableKind::Chat,
            with_choices(),
            "[choices]\n- 丙\n- 丁",
        ),
        (
            "vn-reroll-plain",
            data::TableKind::Vn,
            event(data::TranscriptKind::Narration, "走吧。", None),
            "[choices]\n- 丙\n- 丁",
        ),
        (
            "vn-reroll-empty",
            data::TableKind::Vn,
            with_choices(),
            "沒有選項",
        ),
    ];
    for (label, kind, last, reply) in cases {
        let (root, world_id) = table(label, kind, last);
        let before = data::read_transcript(&root, &world_id, 0).unwrap();
        let mut fake = Fake {
            reply: reply.to_owned(),
            sent: Vec::new(),
            meanwhile: None,
        };
        let result = reroll_choices(&root, &world_id, "zh-TW", &path(), &mut fake).await;
        assert!(result.is_err(), "{label}");
        assert_eq!(
            data::read_transcript(&root, &world_id, 0).unwrap(),
            before,
            "{label}"
        );
    }
}

/// 在途時拒收（與回合互斥）；模型回來前已往下走＝提交閘門重驗不過，歷史不改
#[tokio::test]
async fn reroll_refuses_when_busy_or_stale() {
    let narration = || {
        event(
            data::TranscriptKind::Narration,
            "走吧。\n[choices]\n- 甲\n- 乙",
            None,
        )
    };
    let reply = "[choices]\n- 丙\n- 丁".to_owned();
    let (root, world_id) = table("vn-reroll-busy", data::TableKind::Vn, narration());
    let before = data::read_transcript(&root, &world_id, 0).unwrap();
    let mut fake = Fake {
        reply: reply.clone(),
        sent: Vec::new(),
        meanwhile: None,
    };
    let busy = {
        let (_guard, _cancel) = crate::inflight::register_turn(&world_id, "turn-1");
        reroll_choices(&root, &world_id, "zh-TW", &path(), &mut fake).await
    };
    assert!(busy.unwrap_err().starts_with("VN_REROLL_BUSY"));
    assert!(fake.sent.is_empty());
    assert_eq!(data::read_transcript(&root, &world_id, 0).unwrap(), before);

    let (root, world_id) = table("vn-reroll-stale", data::TableKind::Vn, narration());
    let mut fake = Fake {
        reply,
        sent: Vec::new(),
        meanwhile: Some((root.clone(), world_id.clone())),
    };
    let stale = reroll_choices(&root, &world_id, "zh-TW", &path(), &mut fake).await;
    assert!(stale.unwrap_err().starts_with("VN_REROLL_STALE"));
    let events = data::read_transcript(&root, &world_id, 0).unwrap();
    assert_eq!(events[1].text, "走吧。\n[choices]\n- 甲\n- 乙");
    assert_eq!(events.len(), 3);

    // 本文已變（同一則仍是最後一則）：閘門內比對原文不符也不寫
    let (root, world_id) = table("vn-reroll-edited", data::TableKind::Vn, narration());
    let id = data::read_transcript(&root, &world_id, 0).unwrap()[1]
        .id
        .clone()
        .unwrap();
    let written = data::replace_transcript_text(&root, &world_id, 0, &id, "舊的原文", "新", None);
    assert!(!written.unwrap());
    assert_eq!(
        data::read_transcript(&root, &world_id, 0).unwrap()[1].text,
        "走吧。\n[choices]\n- 甲\n- 乙"
    );
}
