use super::{
    first_sfx, freesound_first, import_entries, license_ok, load_pieces, mp3_ok, parse_pieces,
    pick_piece, search_filter, Piece,
};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn piece(title: &str, filename: &str, feel: &str) -> Piece {
    Piece {
        title: title.to_owned(),
        filename: filename.to_owned(),
        feel: feel
            .split(',')
            .map(|tag| tag.trim().to_lowercase())
            .collect(),
    }
}

#[test]
fn same_table_same_moods_pick_one_track() {
    let pieces = vec![
        piece("A", "a.mp3", "Mysterious, Dark"),
        piece("B", "b.mp3", "Mysterious"),
        piece("C", "c.mp3", "Dark"),
    ];
    let moods = vec!["Mysterious".to_owned(), "Dark".to_owned()];
    let first = pick_piece("table", &moods, &pieces).unwrap().title.clone();
    assert_eq!(first, "A");
    assert_eq!(pick_piece("table", &moods, &pieces).unwrap().title, first);
}

#[test]
fn empty_intersection_falls_back_to_the_first_mood() {
    let pieces = vec![
        piece("Only dark", "dark.mp3", "Dark"),
        piece("Bright", "bright.mp3", "Bright"),
    ];
    let moods = vec!["Dark".to_owned(), "Epic".to_owned()];
    assert_eq!(
        pick_piece("table", &moods, &pieces).unwrap().title,
        "Only dark"
    );
    assert!(pick_piece("table", &["Epic".to_owned()], &pieces).is_none());
}

#[test]
fn bad_index_rows_are_skipped_and_a_non_array_is_rejected() {
    let raw = br#"[
        {"title":"Ok","filename":"ok.mp3","feel":"Dark"},
        {"title":"Bad","filename":"../no.mp3","feel":"Dark"},
        {"title":1,"filename":"x.mp3","feel":"Dark"},
        {"title":"Slash","filename":"a/b.mp3","feel":"Dark"}
    ]"#;
    let pieces = parse_pieces(raw).unwrap();
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].filename, "ok.mp3");
    assert!(parse_pieces(br#"{"title":"no"}"#).is_err());
}

#[tokio::test]
async fn fresh_cache_is_used_without_a_fetch() {
    let dir = std::env::temp_dir().join(format!("tt-audio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cache")).unwrap();
    std::fs::write(
        dir.join("cache").join("incompetech-pieces.json"),
        br#"[{"title":"Old","filename":"old.mp3","feel":"Calming"}]"#,
    )
    .unwrap();
    let pieces = load_pieces(&dir).await;
    assert_eq!(pieces[0].title, "Old");
    assert!(pick_piece("t", &["Calming".to_owned()], &pieces).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mp3_header_accepts_id3_and_frame_sync() {
    assert!(mp3_ok(b"ID3\x03\x00\x00"));
    assert!(mp3_ok(b"ID3\x04\x00\x00"));
    assert!(!mp3_ok(b"ID3\x05\x00\x00"));
    assert!(!mp3_ok(b"ID3"));
    assert!(mp3_ok(&[0xFF, 0xFB, 0x90, 0x00]));
    assert!(!mp3_ok(&[0xFF, 0xF9, 0x90, 0x00]));
    assert!(!mp3_ok(&[0xFF, 0xFB, 0xF0, 0x00]));
}

#[tokio::test]
async fn freesound_without_a_token_does_not_call() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let found = freesound_first("", "door creak", move |_url, _token| {
        let hits2 = hits2.clone();
        async move {
            hits2.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    })
    .await;
    assert!(found.is_none());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn freesound_empty_results_are_not_an_error() {
    let found = freesound_first("token", "door", |_url, token| async move {
        assert_eq!(token, "token");
        Ok(br#"{"results":[]}"#.to_vec())
    })
    .await;
    assert!(found.is_none());
}

#[test]
fn freesound_skips_a_bad_license_and_keeps_the_next() {
    let body = br#"{
        "results":[
            {"name":"no","username":"a","url":"https://freesound.org/s/1/","license":"Attribution NonCommercial","previews":{"preview-hq-mp3":"https://cdn.freesound.org/a.mp3"}},
            {"name":"yes","username":"b","url":"https://freesound.org/s/2/","license":"Attribution","previews":{"preview-hq-mp3":"https://cdn.freesound.org/b.mp3"}},
            {"name":"offhost","username":"c","url":"https://freesound.org/s/3/","license":"Creative Commons 0","previews":{"preview-hq-mp3":"https://evil.example/c.mp3"}}
        ]
    }"#;
    let hit = first_sfx(body).unwrap();
    assert_eq!(hit.title, "yes");
    assert!(search_filter().contains("duration:[0 TO 15]"));
    assert!(search_filter().contains(r#"license:("Creative Commons 0" OR "Attribution")"#));
}

#[test]
fn license_rejects_a_forged_url_and_a_longer_attribution_name() {
    assert!(!license_ok(
        "https://invalid.example/creativecommons.org/licenses/by/4.0/"
    ));
    assert!(!license_ok("Attribution NonCommercial"));
    assert!(license_ok(" Attribution "));
    assert!(license_ok("Creative Commons 0"));
    let forged = br#"{"results":[
        {"name":"fake","username":"a","url":"https://freesound.org/s/1/","license":"https://invalid.example/creativecommons.org/licenses/by/4.0/","previews":{"preview-hq-mp3":"https://cdn.freesound.org/a.mp3"}},
        {"name":"nc","username":"a","url":"https://freesound.org/s/2/","license":"Attribution NonCommercial","previews":{"preview-hq-mp3":"https://cdn.freesound.org/b.mp3"}}
    ]}"#;
    assert!(first_sfx(forged).is_none());
}

#[test]
fn import_drops_sfx_and_unknown_keys_and_rejects_too_many() {
    let bytes = br#"[
        {"kind":"bgm","key":"theme","title":"T","author":"Kevin MacLeod","source_url":"https://incompetech.com/a","license":"CC BY 4.0"},
        {"kind":"sfx","key":"abcdef0123456789","title":"door","author":"x","source_url":"https://freesound.org/s","license":"Creative Commons 0"},
        {"kind":"bgm","key":"Nope","title":"x","author":"x","source_url":"","license":""},
        {"kind":"bgm","key":"dark","title":"missing","author":"x","source_url":"","license":""}
    ]"#;
    let mut keys = BTreeSet::new();
    keys.insert("theme".to_owned());
    let entries = import_entries(bytes, &keys).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "theme");
    let many = format!(
        "[{}]",
        vec![r#"{"kind":"bgm","key":"theme"}"#; 201].join(",")
    );
    assert!(import_entries(many.as_bytes(), &keys).is_err());
}

#[test]
fn prompts_name_bgm_sfx_and_the_situation_map() {
    for lang in ["en", "zh-TW"] {
        let text = crate::transport::vn_format_rules(lang, false, false);
        assert!(text.contains("[bgm:"), "{lang}");
        assert!(text.contains("[sfx:"), "{lang}");
        assert!(text.contains("Mysterious"), "{lang}");
        assert!(text.contains("Humorous"), "{lang}");
        assert!(text.contains("door creak"), "{lang}");
        let guard = if lang == "en" {
            "Only for a clear one-off event such as a door, thunder or breaking glass; never for every line and never for mood."
        } else {
            "只配明確的單次事件（開門、雷聲、碎裂），不配每句台詞、不配情緒。"
        };
        assert!(text.contains(guard), "{lang}");
    }
}
