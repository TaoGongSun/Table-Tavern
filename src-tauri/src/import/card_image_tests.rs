//! 角色卡與世界書 PNG 匯入的存圖：原卡存 `<id>.card.png`（救圖版截到 IEND／重編，搬不到卡文字退成
//! .import.json），顯示圖 `<id>.png` 另存剝掉卡文字的一份；GM 圖救回與略過，以及卡片介面（顯示腳本、
//! MVU、開場白）不受影響。
use super::card_io::{png_chunk, PNG_MAGIC};
use super::png_clean::strip_card_text;
use super::png_clean::tests::{break_crc, palette_png, with_trailing};
use super::png_image::test_png::real_png;
use super::png_image::{validate_png_image, STORED_IMAGE_LIMITS};
use super::test_support::{card_png, minimal_png, TestRoot};
use super::*;
use crate::data;
use crate::ui_msg::UiMsg;
use serde_json::json;
use std::fs;
use std::path::Path;

fn card_json() -> String {
    json!({
        "data": {
            "name": "莉亞",
            "first_mes": "門開了。",
            "extensions": {
                "regex_scripts": [{
                    "scriptName": "顯示介面",
                    "findRegex": "/.+/s",
                    "replaceString": "<div>ok</div>",
                    "placement": [2]
                }],
                "tavern_helper": {"scripts": [{
                    "name": "MVU", "enabled": true, "type": "script",
                    "content": "import 'https://example.invalid/MagVarUpdate/bundle.js';"
                }]}
            },
            "character_book": {"entries": [
                {"keys": ["城門"], "content": "城門在北邊。", "enabled": true}
            ]}
        }
    })
    .to_string()
}

/// 在 IEND 前插一個 tEXt chara
fn with_chara(png: &[u8], chara_json: &str) -> Vec<u8> {
    let text = format!(
        "chara\0{}",
        super::card_io::base64_encode(chara_json.as_bytes())
    );
    let iend = png.len() - 12;
    let mut out = png[..iend].to_vec();
    out.extend_from_slice(&png_chunk(b"tEXt", text.as_bytes()));
    out.extend_from_slice(&png[iend..]);
    out
}

fn insert_before_idat(png: &[u8], chunk: &[u8]) -> Vec<u8> {
    let mut offset = PNG_MAGIC.len();
    loop {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        if &png[offset + 4..offset + 8] == b"IDAT" {
            let mut out = png[..offset].to_vec();
            out.extend_from_slice(chunk);
            out.extend_from_slice(&png[offset..]);
            return out;
        }
        offset += 12 + length;
    }
}

struct Outcome {
    root: TestRoot,
    world_id: String,
    id: String,
    image_dropped: bool,
}

impl Outcome {
    fn file(&self, extension: &str) -> std::path::PathBuf {
        data::character_path(self.root.path(), &self.world_id, &self.id)
            .unwrap()
            .with_extension(extension)
    }

    fn interface(&self) -> (Vec<InterfaceScript>, Option<String>, Option<String>, bool) {
        let found = read_card_interfaces(self.root.path(), &self.world_id)
            .unwrap()
            .into_iter()
            .find(|card| card.character_id == self.id)
            .expect("card interface");
        (found.scripts, found.unsupported, found.opening, found.mvu)
    }
}

fn import(label: &str, bytes: &[u8]) -> Outcome {
    let root = TestRoot::new(label);
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let imported = import_character_file(
        root.path(),
        &world_id,
        bytes,
        "#3366ff",
        "zh-TW",
        &data::test_exclusive(&world_id),
    )
    .unwrap();
    Outcome {
        id: imported.value.id,
        image_dropped: imported.image_dropped,
        root,
        world_id,
    }
}

fn baseline_interface() -> (Vec<InterfaceScript>, Option<String>, Option<String>, bool) {
    let outcome = import("card-image-baseline", card_json().as_bytes());
    let interface = outcome.interface();
    assert!(!interface.0.is_empty() && interface.2.is_some() && interface.3);
    interface
}

/// 顯示圖：存在、過嚴驗、沒有卡片資料，且就是原卡圖剝掉卡文字。
fn assert_display_of(outcome: &Outcome, card: &[u8], label: &str) {
    let display = fs::read(outcome.file("png")).unwrap();
    assert_eq!(display, strip_card_text(card).unwrap(), "{label}");
    assert!(
        validate_png_image(&display, STORED_IMAGE_LIMITS).is_ok(),
        "{label}"
    );
    assert_eq!(
        super::card_io::find_png_card(&display).unwrap(),
        None,
        "{label}"
    );
}

#[test]
fn valid_card_png_is_stored_byte_for_byte() {
    let png = card_png(&card_json());
    let outcome = import("card-image-valid", &png);
    assert!(!outcome.image_dropped);
    assert_eq!(fs::read(outcome.file("card.png")).unwrap(), png);
    assert_display_of(&outcome, &png, "valid");
    assert!(!outcome.file("import.json").exists());
    assert_eq!(outcome.interface(), baseline_interface());
}

#[test]
fn trailing_data_is_cut_at_iend_keeping_original_bytes() {
    for (label, png) in [
        ("card-image-trailing", card_png(&card_json())),
        (
            "card-image-palette-trailing",
            with_chara(&palette_png(4, 3, 2, 1), &card_json()),
        ),
    ] {
        let outcome = import(label, &with_trailing(png.clone()));
        assert!(!outcome.image_dropped, "{label}");
        assert_eq!(fs::read(outcome.file("card.png")).unwrap(), png, "{label}");
        assert_display_of(&outcome, &png, label);
        assert!(!outcome.file("import.json").exists(), "{label}");
        assert_eq!(outcome.interface(), baseline_interface(), "{label}");
    }
}

#[test]
fn reencoded_card_image_keeps_card_text() {
    let bad_note = {
        let mut chunk = png_chunk(b"tEXt", b"note\0hi");
        let last = chunk.len() - 1;
        chunk[last] ^= 1;
        chunk
    };
    for (label, png) in [
        (
            "card-image-text-crc",
            insert_before_idat(&card_png(&card_json()), &bad_note),
        ),
        (
            "card-image-oversized",
            with_chara(&real_png(8200, 2), &card_json()),
        ),
    ] {
        let outcome = import(label, &png);
        assert!(!outcome.image_dropped, "{label}");
        let stored = fs::read(outcome.file("card.png")).unwrap();
        assert_ne!(stored, png, "{label}");
        assert!(
            validate_png_image(&stored, STORED_IMAGE_LIMITS).is_ok(),
            "{label}"
        );
        assert_display_of(&outcome, &stored, label);
        assert!(!outcome.file("import.json").exists(), "{label}");
        assert_eq!(outcome.interface(), baseline_interface(), "{label}");
    }
}

#[test]
fn unrecoverable_image_falls_back_to_import_json() {
    for (label, png) in [
        ("card-image-minimal", minimal_png(&card_json())),
        (
            "card-image-idat-crc",
            break_crc(&card_png(&card_json()), b"IDAT"),
        ),
    ] {
        let outcome = import(label, &png);
        assert!(outcome.image_dropped, "{label}");
        assert!(!outcome.file("png").exists(), "{label}");
        assert!(!outcome.file("card.png").exists(), "{label}");
        assert_eq!(
            fs::read(outcome.file("import.json")).unwrap(),
            card_json().as_bytes(),
            "{label}"
        );
        assert_eq!(outcome.interface(), baseline_interface(), "{label}");
    }
}

/// 圖救得回、卡文字搬不過去（chara 在 IEND 之後，截到 IEND 就沒了）：原卡退成 .import.json，
/// 顯示圖照樣由救回的圖獨立存下。
#[test]
fn salvaged_image_without_card_text_still_stores_display() {
    let clean = real_png(4, 3);
    let png = {
        let mut png = clean.clone();
        let text = format!(
            "chara\0{}",
            super::card_io::base64_encode(card_json().as_bytes())
        );
        png.extend_from_slice(&png_chunk(b"tEXt", text.as_bytes()));
        png
    };
    let outcome = import("card-image-chara-after-iend", &png);
    assert!(!outcome.image_dropped);
    assert!(!outcome.file("card.png").exists());
    assert_display_of(&outcome, &clean, "chara-after-iend");
    assert_eq!(
        fs::read(outcome.file("import.json")).unwrap(),
        card_json().as_bytes()
    );
    assert_eq!(outcome.interface(), baseline_interface());
}

#[test]
fn json_card_is_unchanged() {
    let outcome = import("card-image-json", card_json().as_bytes());
    assert!(!outcome.image_dropped);
    assert!(!outcome.file("png").exists());
    assert!(!outcome.file("card.png").exists());
    assert_eq!(
        fs::read(outcome.file("import.json")).unwrap(),
        card_json().as_bytes()
    );
}

fn characters_left(root: &Path, world_id: &str) -> Vec<std::ffi::OsString> {
    fs::read_dir(root.join(format!("worlds/{world_id}/characters")))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

/// 原卡、`.import.json` 或顯示圖寫不進去（改名失敗或寫到一半）：整次匯入回錯，剛建的角色 md、
/// 已寫的檔與暫存檔全收掉，不留半匯入的角色。
#[test]
fn card_or_display_write_failure_rolls_back_the_new_character() {
    enum Fault {
        Rename(&'static str, u32),
        Partial(&'static str, u32),
    }
    let png = card_png(&card_json());
    let json = card_json().into_bytes();
    let cases: [(&str, &[u8], Fault); 6] = [
        ("card-rename-fail", &png, Fault::Rename(".card.png", 0)),
        // `.png` 結尾第一支是 `.card.png`，放行它、打在顯示圖
        ("display-rename-fail", &png, Fault::Rename(".png", 1)),
        ("json-rename-fail", &json, Fault::Rename(".import.json", 0)),
        (
            "card-partial-write",
            &png,
            Fault::Partial(".card.png.tmp", 0),
        ),
        ("display-partial-write", &png, Fault::Partial(".png.tmp", 1)),
        (
            "json-partial-write",
            &json,
            Fault::Partial(".import.json.tmp", 0),
        ),
    ];
    for (label, bytes, fault) in cases {
        let root = TestRoot::new(label);
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let result = {
            let _rename = match fault {
                Fault::Rename(suffix, skip) => {
                    Some(data::RenameFailGuard::fail_ending_after(suffix, skip, 1))
                }
                Fault::Partial(..) => None,
            };
            let _partial = match fault {
                Fault::Partial(suffix, skip) => {
                    Some(data::WriteFailGuard::partial_ending_after(suffix, skip, 1))
                }
                Fault::Rename(..) => None,
            };
            import_character(root.path(), &world_id, bytes, "#3366ff", "zh-TW")
        };
        let error = result.err().expect(label).to_string();
        assert!(!error.contains("rollback"), "{label}: {error}");
        assert!(
            data::list_characters(root.path(), &world_id)
                .unwrap()
                .is_empty(),
            "{label}"
        );
        let left = characters_left(root.path(), &world_id);
        assert!(left.is_empty(), "{label}: {left:?}");
    }
}

/// 雙故障：顯示圖改名失敗、回滾時 `.card.png` 也刪不掉——回錯同時帶原錯、角色 id 與殘檔名。
#[test]
fn failed_rollback_reports_the_leftover_files() {
    let root = TestRoot::new("rollback-fails");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let error = {
        let _rename = data::RenameFailGuard::fail_ending_after(".png", 1, 1);
        let _remove = data::RemoveFailGuard::fail_ending(".card.png", 1);
        import_character(
            root.path(),
            &world_id,
            &card_png(&card_json()),
            "#3366ff",
            "zh-TW",
        )
        .unwrap_err()
        .to_string()
    };
    let left = characters_left(root.path(), &world_id);
    assert_eq!(left.len(), 1, "{left:?}");
    let name = left[0].to_string_lossy().into_owned();
    let id = name.strip_suffix(".card.png").expect("留下的是原卡");
    assert!(
        error.contains("character_import_rollback_failed"),
        "{error}"
    );
    assert!(error.contains(id) && error.contains(&name), "{error}");
    assert!(error.contains(".png.tmp"), "原寫入錯誤要帶上：{error}");
    assert!(data::list_characters(root.path(), &world_id)
        .unwrap()
        .is_empty());
}

/// 雙故障：顯示圖改名失敗、暫存檔 `<id>.png.tmp` 也刪不掉——殘檔清單列出那支暫存檔。
#[test]
fn failed_rollback_lists_a_stuck_temp_file() {
    let root = TestRoot::new("rollback-temp-stuck");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let error = {
        let _rename = data::RenameFailGuard::fail_ending_after(".png", 1, 1);
        let _remove = data::RemoveFailGuard::fail_ending(".png.tmp", 10);
        import_character(
            root.path(),
            &world_id,
            &card_png(&card_json()),
            "#3366ff",
            "zh-TW",
        )
        .unwrap_err()
        .to_string()
    };
    let left = characters_left(root.path(), &world_id);
    assert_eq!(left.len(), 1, "{left:?}");
    let name = left[0].to_string_lossy().into_owned();
    assert!(name.ends_with(".png.tmp") && !name.ends_with(".card.png.tmp"));
    let Some(UiMsg::CharacterImportRollbackFailed { files, id, .. }) = UiMsg::parse(&error) else {
        panic!("{error}");
    };
    assert_eq!(files, format!("characters/{name}"));
    assert_eq!(format!("{id}.png.tmp"), name);
}

fn pending_markers(root: &Path, world_id: &str) -> usize {
    fs::read_dir(root.join("worlds").join(world_id))
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("import-pending-")
        })
        .count()
}

fn import_book(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
) -> data::DataResult<files::Imported<data::WorldbookImport>> {
    import_worldbook_file(root, world_id, bytes, "書", &data::test_exclusive(world_id))
}

#[test]
fn gm_image_is_salvaged_like_character_cards() {
    let png = card_png(&card_json());
    let bad_note = {
        let mut chunk = png_chunk(b"tEXt", b"note\0hi");
        let last = chunk.len() - 1;
        chunk[last] ^= 1;
        chunk
    };
    for (label, bytes, expect_exact) in [
        ("gm-valid", png.clone(), Some(png.clone())),
        ("gm-trailing", with_trailing(png.clone()), Some(png.clone())),
        ("gm-text-crc", insert_before_idat(&png, &bad_note), None),
        (
            "gm-oversized",
            with_chara(&real_png(8200, 2), &card_json()),
            None,
        ),
    ] {
        let root = TestRoot::new(label);
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let imported = import_book(root.path(), &world_id, &bytes).unwrap();
        assert!(!imported.image_dropped, "{label}");
        let stored = fs::read(data::gm_image_path(root.path(), &world_id).unwrap()).unwrap();
        assert!(
            validate_png_image(&stored, STORED_IMAGE_LIMITS).is_ok(),
            "{label}"
        );
        if let Some(exact) = expect_exact {
            assert_eq!(stored, exact, "{label}");
        }
        // 原卡（介面資料）原樣存，不受圖處理影響
        let world_card = data::world_card_path(root.path(), &world_id, "png").unwrap();
        assert_eq!(fs::read(world_card).unwrap(), bytes, "{label}");
    }
}

#[test]
fn unrecoverable_gm_image_is_skipped_and_reported() {
    let root = TestRoot::new("gm-unrecoverable");
    let world_id = data::create_world(root.path(), "酒館").unwrap();
    let imported = import_book(root.path(), &world_id, &minimal_png(&card_json())).unwrap();
    assert!(imported.image_dropped);
    assert!(!data::gm_image_path(root.path(), &world_id)
        .unwrap()
        .exists());
    assert_eq!(pending_markers(root.path(), &world_id), 0);
}

#[test]
fn gm_image_write_failure_keeps_old_image_and_pending_marker() {
    let old = card_png(&card_json());
    let new = with_chara(&real_png(5, 5), &card_json());
    for (label, partial) in [("gm-fail-write", true), ("gm-fail-rename", false)] {
        let root = TestRoot::new(label);
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        import_book(root.path(), &world_id, &old).unwrap();
        let gm_path = data::gm_image_path(root.path(), &world_id).unwrap();
        assert_eq!(fs::read(&gm_path).unwrap(), old);
        let result = if partial {
            let _guard = data::WriteFailGuard::partial_ending("gm.png.tmp", 1);
            import_book(root.path(), &world_id, &new)
        } else {
            let _guard = data::RenameFailGuard::fail_ending("gm.png", 1);
            import_book(root.path(), &world_id, &new)
        };
        assert!(result.is_err(), "{label}");
        assert_eq!(fs::read(&gm_path).unwrap(), old, "{label}");
        assert!(!gm_path.with_extension("png.tmp").exists(), "{label}");
        assert_eq!(pending_markers(root.path(), &world_id), 1, "{label}");
    }
}
