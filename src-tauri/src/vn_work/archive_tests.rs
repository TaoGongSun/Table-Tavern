//! 惡意 zip：路徑逃逸、重複條目、連結、條目過多、實讀超量、不是 zip（plans/vn-mode-pkg8.md §3）。
use super::*;
use crate::vn_work::format::LIMITS;
use std::io::Write;
use zip::write::SimpleFileOptions;

fn pack(label: &str, files: &[(&str, &[u8])]) -> std::path::PathBuf {
    let path = crate::smart_free::test_root(label).join("p.ttvn");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, bytes) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    path
}

fn error_of(path: &std::path::Path, limits: Limits) -> String {
    Pack::open(path, limits).err().expect("應該拒收")
}

#[test]
fn unsafe_names_are_refused() {
    for name in [
        "../x.png", "/abs.png", "a\\b.png", "C:x.png", "a/./b", "a\u{1}b",
    ] {
        let path = pack("vn-archive-name", &[(name, b"x")]);
        assert_eq!(
            error_of(&path, LIMITS),
            UiMsg::ArchivePathUnsafe.to_string(),
            "{name}"
        );
    }
}

#[test]
fn duplicate_entries_collapse_to_one_view() {
    let path = pack("vn-archive-dup", &[("dupA1", b"x"), ("dupA2", b"y")]);
    // 改成同名（中央目錄與本地標頭都改）：解析後只剩一筆，驗與讀都是同一筆
    let mut bytes = std::fs::read(&path).unwrap();
    for i in 0..bytes.len().saturating_sub(5) {
        if &bytes[i..i + 5] == b"dupA2" {
            bytes[i + 4] = b'1';
        }
    }
    std::fs::write(&path, bytes).unwrap();
    let mut opened = Pack::open(&path, LIMITS).unwrap();
    assert_eq!(opened.index.len(), 1);
    assert!(opened.read("dupA1", 10).unwrap().is_some());
}

#[test]
fn duplicate_names_still_count_against_entry_limit() {
    let names: Vec<String> = (0..5).map(|i| format!("dup0{i}")).collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .map(|name| (name.as_str(), &b"x"[..]))
        .collect();
    let path = pack("vn-archive-dup-many", &files);
    // 5 筆全改成同名：crate 去重後只剩 1 筆，原始條目數仍是 5
    let mut bytes = std::fs::read(&path).unwrap();
    for i in 0..bytes.len().saturating_sub(5) {
        if bytes[i..i + 4] == *b"dup0" {
            bytes[i + 4] = b'0';
        }
    }
    std::fs::write(&path, bytes).unwrap();
    let limits = Limits {
        entries: 4,
        ..LIMITS
    };
    assert_eq!(error_of(&path, limits), too_large());
    assert_eq!(Pack::open(&path, LIMITS).unwrap().index.len(), 1);
}

/// 一份全部同名的包：`count` 筆條目，crate 去重後只剩 1 筆
fn all_same_name(label: &str, count: usize) -> Vec<u8> {
    let names: Vec<String> = (0..count).map(|i| format!("d{i:04}")).collect();
    let files: Vec<(&str, &[u8])> = names
        .iter()
        .map(|name| (name.as_str(), &b"x"[..]))
        .collect();
    let mut bytes = std::fs::read(pack(label, &files)).unwrap();
    for i in 0..bytes.len().saturating_sub(5) {
        if bytes[i] == b'd' && bytes[i + 1..i + 5].iter().all(u8::is_ascii_digit) {
            bytes[i + 1..i + 5].copy_from_slice(b"0000");
        }
    }
    bytes
}

/// Sol 重現：5,000 同名條目＋尾端 70,000 位元組垃圾，結尾目錄落在舊的檔尾掃描窗外，crate 照樣找得到
#[test]
fn trailing_garbage_cannot_hide_the_directory() {
    let mut bytes = all_same_name("vn-archive-tail", 5_000);
    bytes.extend(std::iter::repeat_n(0u8, 70_000));
    let path = crate::smart_free::test_root("vn-archive-tail2").join("p.ttvn");
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(error_of(&path, LIMITS), too_large());
}

/// 檔尾放一個 crate 必拒的假結尾目錄（本碟號≠目錄碟號），crate 退回前面真的那個：真的那個也要先驗到
#[test]
fn fallback_to_an_earlier_directory_is_checked() {
    let mut bytes = all_same_name("vn-archive-fallback", 5);
    bytes.extend(std::iter::repeat_n(0u8, 70_000));
    let mut fake = b"PK\x05\x06".to_vec();
    fake.extend([1, 0, 0, 0, 1, 0, 1, 0, 46, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend(fake);
    let path = crate::smart_free::test_root("vn-archive-fallback2").join("p.ttvn");
    std::fs::write(&path, bytes).unwrap();
    // 上限夠時開得起來且只剩 1 筆：證明 crate 真的退回去用了前面那個目錄
    assert_eq!(Pack::open(&path, LIMITS).unwrap().index.len(), 1);
    let limits = Limits {
        entries: 4,
        ..LIMITS
    };
    assert_eq!(error_of(&path, limits), too_large());
}

/// 串流換塊時跨在塊邊界上的結尾目錄也驗得到
#[test]
fn directory_across_chunk_boundary_is_checked() {
    let mut eocd = b"PK\x05\x06".to_vec();
    eocd.extend([0, 0, 0, 0, 5, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    for offset in [(1 << 20) - 30, (1 << 20) - 11, (1 << 20) - 2, 1 << 20] {
        let mut bytes = vec![0u8; offset];
        bytes.extend(&eocd);
        bytes.extend([0u8; 100]);
        let path = crate::smart_free::test_root("vn-archive-chunk").join("p.bin");
        std::fs::write(&path, bytes).unwrap();
        let mut file = std::fs::File::open(&path).unwrap();
        assert_eq!(check_directory(&mut file, 4), Err(too_large()), "{offset}");
    }
}

#[test]
fn links_are_refused() {
    let path = crate::smart_free::test_root("vn-archive-link").join("p.ttvn");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.add_symlink("manifest.json", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert_eq!(error_of(&path, LIMITS), UiMsg::ArchiveHasLink.to_string());
}

#[test]
fn size_limits_count_real_bytes() {
    let too_large = UiMsg::VnWorkTooLarge.to_string();
    let path = pack("vn-archive-many", &[("a", b"1"), ("b", b"2"), ("c", b"3")]);
    assert_eq!(
        error_of(
            &path,
            Limits {
                entries: 2,
                ..LIMITS
            }
        ),
        too_large
    );
    assert_eq!(
        error_of(
            &path,
            Limits {
                pack_bytes: 10,
                ..LIMITS
            }
        ),
        too_large
    );
    // 高壓縮比：檔案很小、解壓很大；單檔與總量都以實讀位元組判
    let zeros = vec![0u8; 1 << 20];
    let path = pack("vn-archive-bomb", &[("a", &zeros), ("b", &zeros)]);
    let mut opened = Pack::open(
        &path,
        Limits {
            total_bytes: 1_500_000,
            ..LIMITS
        },
    )
    .unwrap();
    assert_eq!(opened.read("a", 1000).unwrap_err(), too_large);
    let mut opened = Pack::open(
        &path,
        Limits {
            total_bytes: 1_500_000,
            ..LIMITS
        },
    )
    .unwrap();
    assert!(opened.read("a", 2 << 20).unwrap().is_some());
    assert_eq!(opened.read("b", 2 << 20).unwrap_err(), too_large);
    assert!(opened.read("missing", 10).unwrap().is_none());
}

#[test]
fn not_a_zip_is_not_a_pack() {
    let path = crate::smart_free::test_root("vn-archive-junk").join("p.ttvn");
    std::fs::write(&path, b"not a zip").unwrap();
    assert_eq!(error_of(&path, LIMITS), UiMsg::VnWorkNotPack.to_string());
}
