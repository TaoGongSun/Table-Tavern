//! 作品包往返、隱私、失敗不留半桌、重播桌守門（plans/vn-mode-pkg8.md §3）。
use super::format::LIMITS;
use super::materialize::import_with;
use super::*;
use crate::chat_turn::read_scene;
use crate::data::{self, StateNode, TableKind, TableState, TranscriptEvent, TranscriptKind};
use crate::import::card_io::png_chunk;
use crate::import::png_image::test_png::real_png;
use crate::scene_budget::summarize::{advance_with, regenerate_locked, Capacity, SummaryCaller};
use crate::ui_msg::UiMsg;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) const SECRET: &str = "只有 GM 知道的私設";
const RAW: &str = "<UpdateVariable>內部狀態</UpdateVariable>";

pub(super) fn staged(chapter: u32, ended: Option<&str>) -> TableState {
    let mut state = TableState::default();
    state.table.insert("place".to_owned(), "港口".to_owned());
    let leaf = |text: &str| StateNode::Leaf(text.to_owned());
    state
        .tree
        .insert("chapter".to_owned(), leaf(&chapter.to_string()));
    state
        .tree
        .insert("ended".to_owned(), leaf(ended.unwrap_or("none")));
    state.tree.insert("finale".to_owned(), leaf("false"));
    state
}

pub(super) fn event(
    kind: TranscriptKind,
    speaker: &str,
    text: &str,
    state: TableState,
) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: Some(format!("{text}\n{RAW}")),
        ts: "2026-11-02 00:00".to_owned(),
        speaker_id: speaker.to_owned(),
        speaker_name: if speaker.is_empty() { "GM" } else { "艾琳" }.to_owned(),
        kind,
        text: text.to_owned(),
        state: Some(state),
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

/// 帶 chara／zTXt／eXIf 的圖：出包後只能剩白名單 chunk
fn dirty_png() -> Vec<u8> {
    let png = real_png(4, 3);
    let iend = png.len() - 12;
    let mut out = png[..iend].to_vec();
    out.extend_from_slice(&png_chunk(b"tEXt", b"chara\0c2VjcmV0"));
    out.extend_from_slice(&png_chunk(b"zTXt", b"note\0\0x"));
    out.extend_from_slice(&png_chunk(b"eXIf", b"MM\0*"));
    out.extend_from_slice(&png[iend..]);
    out
}

pub(super) struct Table {
    pub root: PathBuf,
    pub world: String,
    pub hero: String,
}

/// 第 0 幕：開場 → 選項 → 選甲 → 結局；從選項點分岔選乙 → 換章一幕（沒完成）。另有私設系統事件與立繪背景。
pub(super) fn played(label: &str) -> Table {
    let root = crate::smart_free::test_root(label);
    let world = data::create_world_of_kind(&root, "霧港", TableKind::Vn).unwrap();
    let hero = data::new_id();
    let mut card = crate::commands::character_card(&hero, "艾琳");
    card.private_md = SECRET.to_owned();
    data::write_character(&root, &world, &card).unwrap();
    // 卡定主角借玩家卡槽位：名單不列它，作品包也要帶（解析器靠它的名字對到主角）
    let me = data::new_id();
    data::write_character(
        &root,
        &world,
        &crate::commands::character_card(&me, "林誠吾"),
    )
    .unwrap();
    data::set_player_card(&root, &world, Some(me)).unwrap();
    let add = |scene, event: TranscriptEvent| {
        data::append_transcript(&root, &world, scene, &event).unwrap()
    };
    let mut opening = event(
        TranscriptKind::Narration,
        "",
        "[bg] 港口\n霧裡的港口。",
        staged(1, None),
    );
    opening.opening = true;
    add(0, opening);
    let mut private = event(TranscriptKind::System, "", SECRET, staged(1, None));
    private.gm_only = true;
    add(0, private);
    add(
        0,
        event(
            TranscriptKind::Narration,
            "",
            "門外有聲音。\n[choices]\n- 甲\n- 乙",
            staged(1, None),
        ),
    );
    add(0, event(TranscriptKind::Player, "", "甲", staged(1, None)));
    add(
        0,
        event(
            TranscriptKind::Dialogue,
            &hero,
            "艾琳：走吧。",
            staged(1, None),
        ),
    );
    add(
        0,
        event(
            TranscriptKind::Narration,
            "",
            "天亮了。\n[ending: default]",
            staged(1, Some("default")),
        ),
    );
    let choice = read_scene(&root, &world, 0)
        .unwrap()
        .0
        .into_iter()
        .find(|e| e.choice.is_some())
        .unwrap();
    crate::vn_branches::fork_choice(&root, &world, choice.event_id.as_deref().unwrap()).unwrap();
    add(1, event(TranscriptKind::Player, "", "乙", staged(1, None)));
    add(
        1,
        event(
            TranscriptKind::Narration,
            "",
            "你走進霧裡。",
            staged(1, None),
        ),
    );
    data::begin_next_scene_as(&root, &world, "摘要", Some("第二章"), true).unwrap();
    add(
        2,
        event(
            TranscriptKind::Narration,
            "",
            "第二章開始。",
            staged(2, None),
        ),
    );
    let sprites = data::vn_sprite_dir(&root, &world, &hero).unwrap();
    for (path, bytes) in [
        (sprites.join("body/base.png"), real_png(6, 8)),
        (sprites.join("face/happy.png"), real_png(2, 2)),
        (sprites.join("candidates/1.png"), real_png(2, 2)),
        (
            data::character_path(&root, &world, &hero)
                .unwrap()
                .with_extension("png"),
            dirty_png(),
        ),
        (
            data::vn_background_dir(&root, &world)
                .unwrap()
                .join("0123456789abcdef-day.png"),
            real_png(8, 6),
        ),
    ] {
        data::commit_world_write_atomic(&path, &bytes).unwrap();
    }
    let meta = r#"{"version":1,"face":{"x":0,"y":0,"w":2,"h":2},"variants":[],"default":"base"}"#;
    data::commit_world_write_atomic(&sprites.join("sprite.json"), meta.as_bytes()).unwrap();
    let audio = data::vn_audio_dir(&root, &world).unwrap();
    let mp3: &[u8] = b"ID3\x03\x00\x00";
    for (name, bytes) in [
        ("bgm-theme.mp3", mp3),
        ("sfx-abcdef0123456789.mp3", mp3),
        ("bgm-dark.mp3", b"NOPE".as_slice()),
        ("bgm-Nope.mp3", mp3),
    ] {
        data::commit_world_write_atomic(&audio.join(name), bytes).unwrap();
    }
    let index = r#"[{"kind":"bgm","key":"theme","title":"Theme","author":"Kevin MacLeod","source_url":"https://incompetech.com/a","license":"https://creativecommons.org/licenses/by/4.0/"},{"kind":"sfx","key":"abcdef0123456789","title":"door","author":"x","source_url":"https://freesound.org/s","license":"Creative Commons 0"}]"#;
    data::commit_world_write_atomic(&audio.join("audio.json"), index.as_bytes()).unwrap();
    Table { root, world, hero }
}

fn export(table: &Table) -> PathBuf {
    let path = table.root.join("work.ttvn");
    export_work(&table.root, &table.world, &path, None, "zh-TW", false).unwrap();
    path
}

pub(super) fn entries(path: &Path) -> Vec<(String, Vec<u8>)> {
    let file = super::opaque::open(std::fs::File::open(path).unwrap()).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (file.name().to_owned(), bytes)
        })
        .collect()
}

fn chunk_kinds(png: &[u8]) -> Vec<[u8; 4]> {
    let mut kinds = Vec::new();
    crate::import::png_image::for_each_chunk(png, |chunk| {
        kinds.push(chunk.kind);
        Ok(())
    })
    .unwrap();
    kinds
}

/// 一幕的格（去掉 id），比對往返用
fn cells(root: &Path, world: &str, scene: u64) -> String {
    let (cells, backdrops) = read_scene(root, world, scene).unwrap();
    let text: Vec<String> = cells
        .iter()
        .map(|event| {
            format!(
                "{:?}|{:?}|{:?}|{:?}",
                event.kind, event.cells, event.chapter, event.choice
            )
        })
        .collect();
    format!("{text:?}{backdrops:?}")
}

#[test]
fn export_then_import_rebuilds_the_same_scenes_and_branches() {
    let table = played("vn-work-roundtrip");
    let pack = export(&table);
    let imported = import_work(&table.root, &pack, ImportMode::Replay, false).unwrap();
    let (root, copy) = (&table.root, &imported.world_id);
    let state = data::read_state(root, copy).unwrap();
    assert!(state.replay && state.current_scene == 0);
    let player = data::read_player_card(root, copy)
        .unwrap()
        .map(|card| card.name);
    assert_eq!(player.as_deref(), Some("林誠吾"));
    assert_eq!(data::scene_high(&state), 2);
    for scene in 0..=2 {
        assert_eq!(
            cells(root, &table.world, scene),
            cells(root, copy, scene),
            "scene {scene}"
        );
    }
    // 分岔幕只存後段、`cut` 換成新 id 後源頭對應仍成立；換章幕是第 1 幕的故事子幕
    let label = data::scene_label(&state, 1);
    assert_eq!(
        (label.forked, label.source, label.keep),
        (true, Some(0), Some(3))
    );
    let first = data::read_transcript(root, copy, 0).unwrap();
    assert_eq!(label.cut, first[2].id);
    let mut book = data::Transcripts::default();
    assert_eq!(
        data::origin(root, copy, &state, &mut book, 1, 2).unwrap(),
        0
    );
    assert_eq!(data::tip_of(&state, 1), 2);
    // 選項點兩條分支都列得出來，且帶選了那句的幕與玩家句
    let choice = read_scene(root, copy, 0)
        .unwrap()
        .0
        .into_iter()
        .find(|e| e.choice.is_some())
        .unwrap();
    let branches =
        crate::vn_branches::choice_branches(root, copy, choice.event_id.as_deref().unwrap())
            .unwrap();
    let found: Vec<(&str, u64, u64)> = branches
        .iter()
        .map(|b| (b.option.as_str(), b.scene, b.tip))
        .collect();
    assert_eq!(found, [("乙", 1, 2), ("甲", 0, 0)]);
    let ending = crate::vn_bible::endings_overview(root, copy).unwrap();
    assert!(ending
        .endings
        .iter()
        .any(|e| e.id == "default" && e.reached));
    // 立繪、背景、角色圖落檔；候選不帶
    let sprites = data::vn_sprite_dir(root, copy, &table.hero).unwrap();
    assert!(sprites.join("body/base.png").is_file() && sprites.join("face/happy.png").is_file());
    assert!(!sprites.join("candidates").exists());
    let image = std::fs::read(
        data::character_path(root, copy, &table.hero)
            .unwrap()
            .with_extension("png"),
    )
    .unwrap();
    assert_eq!(chunk_kinds(&image), [*b"IHDR", *b"IDAT", *b"IEND"]);
    let packed = entries(&pack);
    let names: Vec<&str> = packed.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"assets/audio/bgm-theme.mp3"));
    assert!(names.contains(&"assets/audio/sfx-abcdef0123456789.mp3"));
    assert!(names.contains(&"assets/audio/audio.json"));
    assert!(!names
        .iter()
        .any(|name| name.contains("Nope") || name.contains("bgm-dark")));
    let index = packed
        .iter()
        .find(|(name, _)| name == "assets/audio/audio.json")
        .unwrap();
    let text = String::from_utf8(index.1.clone()).unwrap();
    assert!(text.contains("sfx"));
    assert!(text.contains("theme"));
    let copied = data::vn_audio_dir(root, copy).unwrap();
    assert_eq!(
        std::fs::read(copied.join("bgm-theme.mp3")).unwrap(),
        b"ID3\x03\x00\x00"
    );
    assert_eq!(
        std::fs::read(copied.join("sfx-abcdef0123456789.mp3")).unwrap(),
        b"ID3\x03\x00\x00"
    );
    assert!(std::fs::read_to_string(copied.join("audio.json"))
        .unwrap()
        .contains("sfx"));
}

#[test]
fn export_import_keeps_bgm_and_sfx() {
    let table = played("vn-audio-calming");
    let audio = data::vn_audio_dir(&table.root, &table.world).unwrap();
    for name in [
        "bgm-theme.mp3",
        "sfx-abcdef0123456789.mp3",
        "bgm-dark.mp3",
        "bgm-Nope.mp3",
        "audio.json",
    ] {
        std::fs::remove_file(audio.join(name)).unwrap();
    }
    let mp3: &[u8] = b"ID3\x03\x00\x00";
    data::commit_world_write_atomic(&audio.join("bgm-calming.mp3"), mp3).unwrap();
    data::commit_world_write_atomic(&audio.join("sfx-abcdef0123456789.mp3"), mp3).unwrap();
    let index = r#"[{"kind":"bgm","key":"calming","title":"Calm","author":"Kevin MacLeod","source_url":"https://incompetech.com/a","license":"https://creativecommons.org/licenses/by/4.0/"},{"kind":"sfx","key":"abcdef0123456789","title":"door","author":"x","source_url":"https://freesound.org/s","license":"Creative Commons 0"}]"#;
    data::commit_world_write_atomic(&audio.join("audio.json"), index.as_bytes()).unwrap();

    let pack = export(&table);
    let raw = std::fs::read(&pack).unwrap();
    assert_eq!(&raw[..8], b"TTVNPK\x01\x00");
    assert!(zip::ZipArchive::new(std::fs::File::open(&pack).unwrap()).is_err());
    let packed = entries(&pack);
    let manifest = packed
        .iter()
        .find(|(name, _)| name == "manifest.json")
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest.1).unwrap();
    let audio_assets: Vec<&str> = manifest["assets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.as_str())
        .filter(|name| name.starts_with("assets/audio/"))
        .collect();
    assert_eq!(
        audio_assets,
        [
            "assets/audio/bgm-calming.mp3",
            "assets/audio/sfx-abcdef0123456789.mp3",
            "assets/audio/audio.json"
        ]
    );

    let imported = import_work(&table.root, &pack, ImportMode::Replay, false).unwrap();
    let copied = data::vn_audio_dir(&table.root, &imported.world_id).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&copied)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["audio.json", "bgm-calming.mp3", "sfx-abcdef0123456789.mp3"]
    );
    let text = std::fs::read_to_string(copied.join("audio.json")).unwrap();
    assert!(text.contains("\"key\":\"calming\"") || text.contains("\"key\": \"calming\""));
    assert!(text.contains("sfx"));
}

#[test]
fn audio_asset_names_are_whitelist_only() {
    assert!(super::format::Asset::parse("assets/audio/bgm-theme.mp3").is_some());
    assert!(super::format::Asset::parse("assets/audio/bgm-mysterious+dark.mp3").is_some());
    assert!(super::format::Asset::parse("assets/audio/bgm-Nope.mp3").is_none());
    assert!(super::format::Asset::parse("assets/audio/sfx-abcdef0123456789.mp3").is_some());
    assert!(super::format::Asset::parse("assets/audio/sfx-Nope.mp3").is_none());
    assert!(super::format::Asset::parse("assets/audio/sfx-abc.mp3").is_none());
    assert!(super::format::Asset::parse("assets/audio/../bgm-theme.mp3").is_none());
}

#[test]
fn a_pack_with_a_bad_mp3_header_is_refused() {
    let table = played("vn-audio-bad-mp3");
    let pack = export(&table);
    let broken = table.root.join("bad.ttvn");
    let mut archive =
        zip::ZipArchive::new(super::opaque::open(std::fs::File::open(&pack).unwrap()).unwrap())
            .unwrap();
    let mut out = zip::ZipWriter::new(
        super::opaque::create(std::fs::File::create(&broken).unwrap()).unwrap(),
    );
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().to_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        if name.ends_with(".mp3") {
            bytes = b"NOPE".to_vec();
        }
        out.start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut out, &bytes).unwrap();
    }
    out.finish().unwrap();
    assert!(import_work(&table.root, &broken, ImportMode::Replay, false).is_err());
}

#[test]
fn the_pack_carries_no_private_text_or_image_metadata() {
    let table = played("vn-work-privacy");
    let pack = export(&table);
    for (name, bytes) in entries(&pack) {
        if name.ends_with(".png") {
            for kind in chunk_kinds(&bytes) {
                assert!(
                    [*b"IHDR", *b"IDAT", *b"IEND"].contains(&kind),
                    "{name}: {kind:?}"
                );
            }
        } else {
            let text = String::from_utf8(bytes).unwrap();
            for leak in [SECRET, "UpdateVariable", "2026-11-02 00:00", "candidates"] {
                assert!(!text.contains(leak), "{name} 帶了 {leak}");
            }
        }
    }
}

#[test]
fn export_needs_an_ending_and_a_playable_table() {
    let root = crate::smart_free::test_root("vn-work-no-ending");
    let world = data::create_world_of_kind(&root, "未完", TableKind::Vn).unwrap();
    data::append_transcript(
        &root,
        &world,
        0,
        &event(TranscriptKind::Narration, "", "開場", staged(1, None)),
    )
    .unwrap();
    let path = root.join("x.ttvn");
    let error = export_work(&root, &world, &path, None, "zh-TW", false).unwrap_err();
    assert_eq!(error, UiMsg::VnWorkNoEnding.to_string());
    assert!(!path.exists() && !root.join("x.ttvn.part").exists());
    // 重播桌不能再匯出
    let table = played("vn-work-reexport");
    let copy = import_work(&table.root, &export(&table), ImportMode::Replay, false)
        .unwrap()
        .world_id;
    let again = export_work(
        &table.root,
        &copy,
        &table.root.join("y.ttvn"),
        None,
        "zh-TW",
        false,
    )
    .unwrap_err();
    assert_eq!(again, UiMsg::VnReplayReadOnly.to_string());
}

#[test]
fn a_failure_at_any_step_leaves_no_table_behind() {
    let table = played("vn-work-atomic");
    let pack = export(&table);
    for step in ["characters", "assets", "scene 1", "state", "vn"] {
        let before = data::list_worlds(&table.root).unwrap().len();
        let mut hook = |at: &str| {
            if at == step {
                Err("故障注入".to_owned())
            } else {
                Ok(())
            }
        };
        assert!(
            import_with(
                &table.root,
                &pack,
                ImportMode::Replay,
                false,
                LIMITS,
                &mut hook
            )
            .is_err(),
            "{step}"
        );
        assert_eq!(
            data::list_worlds(&table.root).unwrap().len(),
            before,
            "{step} 留下半桌"
        );
    }
}

struct Counting(usize);

impl SummaryCaller for Counting {
    async fn call(&mut self, _: Vec<crate::transport::ChatMessage>) -> Result<String, String> {
        self.0 += 1;
        Ok("標題：甲\n\n摘要".to_owned())
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn replay_tables_refuse_generation_and_writes_but_allow_navigation() {
    let table = played("vn-work-replay-guard");
    let copy = import_work(&table.root, &export(&table), ImportMode::Replay, false)
        .unwrap()
        .world_id;
    let root = table.root.as_path();
    let bytes = |scene| {
        std::fs::read(
            root.join("worlds")
                .join(&copy)
                .join(format!("transcript/{scene}.jsonl")),
        )
        .ok()
    };
    let before: Vec<_> = (0..=3).map(bytes).collect();
    let refused = UiMsg::VnReplayReadOnly.to_string();
    let line = event(TranscriptKind::Player, "", "乙", staged(1, None));
    assert_eq!(
        data::append_transcript(root, &copy, 0, &line)
            .unwrap_err()
            .to_string(),
        refused
    );
    assert_eq!(
        data::fork_scene(root, &copy, 0).unwrap_err().to_string(),
        refused
    );
    assert_eq!(
        data::begin_next_scene_as(root, &copy, "x", None, true)
            .unwrap_err()
            .to_string(),
        refused
    );
    assert_eq!(
        data::pop_transcript_floor(root, &copy, 0)
            .unwrap_err()
            .to_string(),
        refused
    );
    assert_eq!(data::vn::refuse_if_ended(root, &copy).unwrap_err(), refused);
    // 收回沒回成的玩家句：拿最後一句玩家句的真收據也拒，不截檔
    let scene0 = before[0].clone().unwrap();
    let start = scene0[..scene0.len() - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let last: crate::data::TranscriptEvent =
        serde_json::from_slice(&scene0[start..scene0.len() - 1]).unwrap();
    assert_eq!(
        data::discard_unanswered_player(root, &copy, 0, start as u64, &last.ts, &last.text)
            .unwrap_err()
            .to_string(),
        refused
    );
    assert_eq!(
        data::replace_scene_summary(root, &copy, "摘要", None)
            .unwrap_err()
            .to_string(),
        refused
    );
    // 重構的模型入口都先組脈絡：這裡就拒，派不到模型
    assert_eq!(
        crate::refactor_ai::assemble_card_context(root, &copy)
            .unwrap_err()
            .to_string(),
        refused
    );
    assert_eq!(
        data::vn_playthrough::new_playthrough(root, &copy, "{name}")
            .unwrap_err()
            .to_string(),
        refused
    );
    // 換幕與重寫提要在呼叫模型之前就擋：零呼叫
    let cap = Capacity {
        unit: crate::scene_budget::Unit::Bytes,
        total: None,
        reserve: 0,
        ratio: 1.0,
        transport: "agy".to_owned(),
        lang: "zh-TW".to_owned(),
    };
    let mut caller = Counting(0);
    assert_eq!(
        block_on(advance_with(root, &copy, &cap, &mut caller, true)).unwrap_err(),
        refused
    );
    assert_eq!(
        block_on(regenerate_locked(root, &copy, &cap, &mut caller)).unwrap_err(),
        refused
    );
    assert_eq!(caller.0, 0);
    // 換幕：非尾幕也能去；從某一章重玩只換到章頭幕，不建新幕
    assert_eq!(data::replay_goto(root, &copy, 1).unwrap(), 1);
    assert_eq!(data::replay_chapter(root, &copy, 1).unwrap(), 0);
    let state = data::read_state(root, &copy).unwrap();
    assert_eq!((state.current_scene, data::scene_high(&state)), (0, 2));
    assert_eq!((0..=3).map(bytes).collect::<Vec<_>>(), before);
    // 一般桌不能 goto
    assert!(data::replay_goto(root, &table.world, 0).is_err());
}
