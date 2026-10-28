use super::grok::TTL_DAYS;
use super::*;
use std::collections::HashSet;
use std::fs;
use std::time::Duration;

const A: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const B: &str = "bbbbbbbb-0000-4000-8000-000000000002";
const C: &str = "cccccccc-0000-4000-8000-000000000003";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "tt-home-cleanup-{tag}-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // 測試可能把資料夾收成唯讀；還原後才刪得掉
            for entry in walk(&self.0) {
                if fs::symlink_metadata(&entry).is_ok_and(|meta| meta.is_dir()) {
                    let _ = fs::set_permissions(&entry, fs::Permissions::from_mode(0o700));
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![dir.to_path_buf()];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
                out.extend(walk(&path));
            }
        }
    }
    out
}

fn base(home: &Path) -> PathBuf {
    home.join(".gemini/antigravity-cli")
}

/// agy 1.4.8 一個對話的全套檔＋一個不屬於對話的 implicit。
fn plant_agy(home: &Path, id: &str) {
    let base = base(home);
    for (sub, name) in [
        ("conversations", format!("{id}.db")),
        ("conversations", format!("{id}.db-wal")),
        ("conversations", format!("{id}.db-shm")),
        ("annotations", format!("{id}.pbtxt")),
        ("presence", format!("{id}.lock")),
    ] {
        fs::create_dir_all(base.join(sub)).unwrap();
        fs::write(base.join(sub).join(name), "x").unwrap();
    }
    let brain = base.join("brain").join(id).join(".system_generated/logs");
    fs::create_dir_all(&brain).unwrap();
    fs::write(brain.join("transcript.jsonl"), "x").unwrap();
    fs::create_dir_all(base.join("implicit")).unwrap();
    fs::write(base.join("implicit").join(format!("{id}.pb")), "x").unwrap();
}

fn agy_files(home: &Path, id: &str) -> Vec<PathBuf> {
    let base = base(home);
    vec![
        base.join(format!("conversations/{id}.db")),
        base.join(format!("conversations/{id}.db-wal")),
        base.join(format!("conversations/{id}.db-shm")),
        base.join(format!("annotations/{id}.pbtxt")),
        base.join(format!("presence/{id}.lock")),
        base.join(format!("brain/{id}")),
    ]
}

fn gone(home: &Path, id: &str) -> bool {
    agy_files(home, id)
        .iter()
        .all(|path| fs::symlink_metadata(path).is_err())
}

fn intact(home: &Path, id: &str) -> bool {
    agy_files(home, id).iter().all(|path| path.exists())
}

/// Windows 改時間要寫入權限（唯讀開檔會 Access is denied）；unix 的資料夾開不了寫入，退回唯讀開。
fn open_for_times(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .or_else(|_| fs::File::open(path))
}

fn set_mtime(path: &Path, when: SystemTime) {
    for entry in walk(path)
        .into_iter()
        .chain(std::iter::once(path.to_path_buf()))
    {
        if let Ok(file) = open_for_times(&entry) {
            let _ = file.set_modified(when);
        }
    }
    if let Ok(file) = open_for_times(path) {
        file.set_modified(when).unwrap();
    }
}

#[test]
fn uuid_shape() {
    assert!(is_uuid(A));
    for bad in [
        "",
        "AAAAAAAA-0000-4000-8000-000000000001",
        "aaaaaaaa00000-4000-8000-000000000001",
        "../aaaa-0000-4000-8000-000000000001",
        "aaaaaaaa-0000-4000-8000-0000000000012",
    ] {
        assert!(!is_uuid(bad), "{bad}");
    }
}

#[test]
fn remove_conversation_takes_only_its_files() {
    let scratch = Scratch::new("agy-remove");
    let home = scratch.0.join("agy-home");
    plant_agy(&home, A);
    plant_agy(&home, B);
    remove_conversation(&home, A).unwrap();
    assert!(gone(&home, A));
    assert!(intact(&home, B));
    // implicit 的 uuid 不是對話 id，不屬於對話
    assert!(base(&home).join(format!("implicit/{A}.pb")).exists());
    // 不存在當已刪、不合格的 id 拒收
    remove_conversation(&home, A).unwrap();
    assert!(remove_conversation(&home, "../x").is_err());
}

#[cfg(unix)]
#[test]
fn links_on_the_path_stop_deletion() {
    use std::os::unix::fs::symlink;
    for level in ["home", ".gemini", "antigravity-cli", "conversations"] {
        let scratch = Scratch::new("agy-links");
        let outside = scratch.0.join("outside");
        plant_agy(&outside, A);
        let home = scratch.0.join("agy-home");
        match level {
            "home" => symlink(&outside, &home).unwrap(),
            ".gemini" => {
                fs::create_dir_all(&home).unwrap();
                symlink(outside.join(".gemini"), home.join(".gemini")).unwrap();
            }
            "antigravity-cli" => {
                fs::create_dir_all(home.join(".gemini")).unwrap();
                symlink(base(&outside), base(&home)).unwrap();
            }
            _ => {
                fs::create_dir_all(base(&home)).unwrap();
                symlink(
                    base(&outside).join("conversations"),
                    base(&home).join("conversations"),
                )
                .unwrap();
            }
        }
        assert!(remove_conversation(&home, A).is_err(), "{level}");
        assert!(intact(&outside, A), "{level} 外部檔被刪");
        assert!(sweep_agy(&home, || Ok(HashSet::new()), far_future()).is_err());
        assert!(intact(&outside, A), "{level} 回收刪到外部");
    }
}

#[cfg(unix)]
#[test]
fn linked_entry_loses_only_the_link() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("agy-entry-link");
    let home = scratch.0.join("agy-home");
    let target = scratch.0.join("precious");
    fs::create_dir_all(target.join("inner")).unwrap();
    fs::write(target.join("inner/keep.txt"), "x").unwrap();
    fs::create_dir_all(base(&home).join("brain")).unwrap();
    symlink(&target, base(&home).join("brain").join(A)).unwrap();
    // keychain 連結與目標：不在可刪範圍
    let keychains = scratch.0.join("real-keychains");
    fs::create_dir_all(&keychains).unwrap();
    fs::write(keychains.join("login.keychain-db"), "x").unwrap();
    fs::create_dir_all(home.join("Library")).unwrap();
    symlink(&keychains, home.join("Library/Keychains")).unwrap();
    remove_conversation(&home, A).unwrap();
    assert!(fs::symlink_metadata(base(&home).join("brain").join(A)).is_err());
    assert!(target.join("inner/keep.txt").exists());
    sweep_agy(&home, || Ok(HashSet::new()), far_future()).unwrap();
    assert!(fs::symlink_metadata(home.join("Library/Keychains")).is_ok());
    assert!(keychains.join("login.keychain-db").exists());
}

fn far_future() -> SystemTime {
    SystemTime::now() + RETENTION + Duration::from_secs(3600)
}

#[test]
fn sweep_keeps_referenced_and_recent() {
    let scratch = Scratch::new("agy-sweep");
    let home = scratch.0.join("agy-home");
    for id in [A, B, C] {
        plant_agy(&home, id);
    }
    let now = far_future();
    // C 在回收時點看來是最近才動過
    set_mtime(&base(&home).join(format!("conversations/{C}.db")), now);
    // 不認得的後綴不算進組、也不刪
    fs::write(
        base(&home).join(format!("conversations/{A}.db-journal")),
        "x",
    )
    .unwrap();
    let removed = sweep_agy(&home, || Ok(HashSet::from([B.to_owned()])), now).unwrap();
    assert_eq!(removed, 1);
    assert!(gone(&home, A));
    assert!(intact(&home, B));
    assert!(intact(&home, C));
    assert!(base(&home)
        .join(format!("conversations/{A}.db-journal"))
        .exists());
}

#[test]
fn sweep_stops_when_mark_fails_or_group_changes() {
    let scratch = Scratch::new("agy-mark");
    let home = scratch.0.join("agy-home");
    plant_agy(&home, A);
    let now = far_future();
    assert!(sweep_agy(&home, || Err("lanes.json 壞了".to_owned()), now).is_err());
    assert!(intact(&home, A));
    // 候選列出後、刪之前被動到（模擬正在續聊）：重新 stat 看到新 mtime，不刪
    let db = base(&home).join(format!("conversations/{A}.db"));
    let removed = sweep_agy(
        &home,
        || {
            set_mtime(&db, now);
            Ok(HashSet::new())
        },
        now,
    )
    .unwrap();
    assert_eq!(removed, 0);
    assert!(intact(&home, A));
}

#[cfg(unix)]
#[test]
fn unreadable_group_member_keeps_the_group() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("agy-unreadable");
    let home = scratch.0.join("agy-home");
    plant_agy(&home, A);
    let brain = base(&home).join("brain").join(A);
    fs::set_permissions(&brain, fs::Permissions::from_mode(0o000)).unwrap();
    let removed = sweep_agy(&home, || Ok(HashSet::new()), far_future()).unwrap();
    fs::set_permissions(&brain, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(removed, 0);
    assert!(intact(&home, A));
}

#[cfg(unix)]
#[test]
fn sweep_logs_keeps_current_and_recent() {
    let scratch = Scratch::new("agy-logs");
    let home = scratch.0.join("agy-home");
    let log = base(&home).join("log");
    fs::create_dir_all(&log).unwrap();
    for name in ["cli-1.log", "cli-2.log", "cli-3.log", "other.txt"] {
        fs::write(log.join(name), "x").unwrap();
    }
    std::os::unix::fs::symlink("log/cli-2.log", base(&home).join("cli.log")).unwrap();
    let now = far_future();
    set_mtime(&log.join("cli-3.log"), now);
    sweep_agy(&home, || Ok(HashSet::new()), now).unwrap();
    assert!(!log.join("cli-1.log").exists());
    assert!(log.join("cli-2.log").exists(), "cli.log 指著的那份");
    assert!(log.join("cli-3.log").exists(), "最近的");
    assert!(log.join("other.txt").exists());
}

#[cfg(unix)]
#[test]
fn grok_remove_session_does_not_follow_links() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("grok-links");
    let outside = scratch.0.join("outside");
    let sentinel = outside.join("sessions/g/").join(A);
    fs::create_dir_all(&sentinel).unwrap();
    fs::write(sentinel.join("chat_history.jsonl"), "x").unwrap();
    // home 是連結
    let home = scratch.0.join("home-link");
    symlink(&outside, &home).unwrap();
    assert!(remove_session(&home, A).is_err());
    // sessions 是連結：不進去找
    let home2 = scratch.0.join("home2");
    fs::create_dir_all(&home2).unwrap();
    symlink(outside.join("sessions"), home2.join("sessions")).unwrap();
    assert_eq!(remove_session(&home2, A).unwrap(), 0);
    // 群組是連結：不進去找
    let home3 = scratch.0.join("home3");
    fs::create_dir_all(home3.join("sessions")).unwrap();
    symlink(outside.join("sessions/g"), home3.join("sessions/g")).unwrap();
    remove_session(&home3, A).unwrap();
    assert!(sentinel.join("chat_history.jsonl").exists());
    // 正常的群組照刪
    let home4 = scratch.0.join("home4");
    let own = home4.join("sessions/g").join(A);
    fs::create_dir_all(&own).unwrap();
    remove_session(&home4, A).unwrap();
    assert!(!own.exists());
}

#[test]
fn prompt_history_only_that_name() {
    let scratch = Scratch::new("grok-history");
    let home = scratch.0.join("grok-home");
    let group = home.join("sessions/%2Fws");
    fs::create_dir_all(group.join(A)).unwrap();
    fs::write(group.join("prompt_history.jsonl"), "私設").unwrap();
    fs::write(group.join(A).join("chat_history.jsonl"), "x").unwrap();
    fs::write(home.join("sessions/session_search.sqlite"), "x").unwrap();
    remove_prompt_history(&home).unwrap();
    assert!(!group.join("prompt_history.jsonl").exists());
    assert!(group.join(A).join("chat_history.jsonl").exists());
    assert!(home.join("sessions/session_search.sqlite").exists());
}

const GROK_OWN: &str = "[marketplace]\ndefault_skills_installs_purged = true\n\n[[marketplace.sources]]\nname = \"xAI Official\"\n";

fn grok_home(scratch: &Scratch) -> PathBuf {
    let home = scratch.0.join("grok-home");
    fs::create_dir_all(&home).unwrap();
    home
}

#[test]
fn ttl_created_fresh() {
    let scratch = Scratch::new("ttl-fresh");
    let home = grok_home(&scratch);
    assert_eq!(ensure_ttl(&home).unwrap(), TtlOutcome::Written);
    let text = fs::read_to_string(home.join("config.toml")).unwrap();
    assert_eq!(text, format!("[storage]\ncleanup_ttl_days = {TTL_DAYS}\n"));
    assert_eq!(ensure_ttl(&home).unwrap(), TtlOutcome::AlreadySet);
}

#[cfg(unix)]
#[test]
fn ttl_merges_and_keeps_grok_sections_and_mode() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("ttl-merge");
    let home = grok_home(&scratch);
    let path = home.join("config.toml");
    fs::write(
        &path,
        format!("{GROK_OWN}\n[storage]\ncleanup_ttl_days = 30\n"),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(ensure_ttl(&home).unwrap(), TtlOutcome::Written);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("default_skills_installs_purged = true"));
    assert!(text.contains("name = \"xAI Official\""));
    assert!(text.contains(&format!("cleanup_ttl_days = {TTL_DAYS}")));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    // 沒有臨時檔殘留
    let leftovers: Vec<_> = fs::read_dir(&home)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tt-"))
        .collect();
    assert!(leftovers.is_empty());
}

#[cfg(unix)]
#[test]
fn ttl_refuses_unsafe_files() {
    let scratch = Scratch::new("ttl-refuse");
    // storage 不是 table
    let home = grok_home(&scratch);
    let path = home.join("config.toml");
    fs::write(&path, "storage = 3\n").unwrap();
    assert!(ensure_ttl(&home).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "storage = 3\n");
    // 解析失敗
    fs::write(&path, "[[[broken").unwrap();
    assert!(ensure_ttl(&home).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "[[[broken");
    // config.toml 是連結：不寫穿
    fs::remove_file(&path).unwrap();
    let outside = scratch.0.join("outside.toml");
    fs::write(&outside, GROK_OWN).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(ensure_ttl(&home).is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), GROK_OWN);
}

#[test]
fn ttl_skips_while_grok_holds_init_lock() {
    let scratch = Scratch::new("ttl-lock");
    let home = grok_home(&scratch);
    fs::write(home.join("config.toml"), GROK_OWN).unwrap();
    let held = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join(".config-init.lock"))
        .unwrap();
    held.lock().unwrap();
    assert_eq!(ensure_ttl(&home).unwrap(), TtlOutcome::LockBusy);
    assert_eq!(
        fs::read_to_string(home.join("config.toml")).unwrap(),
        GROK_OWN
    );
    held.unlock().unwrap();
    assert_eq!(ensure_ttl(&home).unwrap(), TtlOutcome::Written);
}

#[test]
fn ttl_concurrent_writers_write_once() {
    let scratch = Scratch::new("ttl-race");
    let home = grok_home(&scratch);
    fs::write(home.join("config.toml"), GROK_OWN).unwrap();
    let outcomes: Vec<_> = (0..4)
        .map(|_| {
            let home = home.clone();
            std::thread::spawn(move || ensure_ttl(&home).unwrap())
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == TtlOutcome::Written)
            .count(),
        1
    );
    let text = fs::read_to_string(home.join("config.toml")).unwrap();
    assert_eq!(text.matches("cleanup_ttl_days").count(), 1);
    assert!(text.contains("xAI Official"));
}

fn world(root: &Path) -> String {
    let id = ulid::Ulid::generate().to_string();
    let dir = root.join("worlds").join(&id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("format.json"),
        r#"{"format_version":1,"app_version":"test"}"#,
    )
    .unwrap();
    id
}

fn lanes_json(dir: &Path, ids: &[&str]) {
    fs::create_dir_all(dir).unwrap();
    let store: serde_json::Map<_, _> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                format!("chars:m:{index}"),
                serde_json::json!({ "session_id": id }),
            )
        })
        .collect();
    fs::write(
        dir.join("lanes.json"),
        serde_json::to_string(&store).unwrap(),
    )
    .unwrap();
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = walk(root)
        .into_iter()
        .flat_map(|dir| {
            fs::read_dir(dir)
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
        })
        .filter(|path| path.is_file())
        .map(|path| (path.clone(), fs::read(&path).unwrap()))
        .collect();
    files.sort();
    files
}

#[test]
fn mark_reads_live_sidecars_and_reset_roots_without_touching() {
    let scratch = Scratch::new("mark");
    let root = scratch.0.join("data");
    let id = world(&root);
    let worlds = root.join("worlds");
    lanes_json(&worlds.join(&id), &[A]);
    lanes_json(&worlds.join(format!(".tt-pre-{id}")), &[B]);
    lanes_json(&worlds.join(format!(".tt-reset-{id}/worlds/{id}")), &[C]);
    let before = snapshot(&root);
    let ids = crate::lanes::referenced_session_ids(&root).unwrap();
    assert_eq!(
        ids,
        HashSet::from([A.to_owned(), B.to_owned(), C.to_owned()])
    );
    assert_eq!(snapshot(&root), before, "背景清理不能改桌子的檔");
    // 沒有 lanes.json 的桌當空
    let _empty = world(&root);
    assert_eq!(
        crate::lanes::referenced_session_ids(&root).unwrap().len(),
        3
    );
}

#[test]
fn mark_fails_closed() {
    let scratch = Scratch::new("mark-fail");
    let root = scratch.0.join("data");
    let id = world(&root);
    let live = root.join("worlds").join(&id);
    // 解析不了
    fs::write(live.join("lanes.json"), "{oops").unwrap();
    assert!(crate::lanes::referenced_session_ids(&root).is_err());
    // 有進行中的交易紀錄
    lanes_json(&live, &[A]);
    let log = root.join("worlds").join(format!(".tt-op-{id}.json"));
    fs::write(&log, "{}").unwrap();
    assert!(crate::lanes::referenced_session_ids(&root).is_err());
    fs::remove_file(&log).unwrap();
    // 待修復（主資料夾不見、只剩副本）
    let lost = world(&root);
    fs::rename(
        root.join("worlds").join(&lost),
        root.join("worlds").join(format!(".tt-pre-{lost}")),
    )
    .unwrap();
    assert!(crate::lanes::referenced_session_ids(&root).is_err());
}

/// 單發善後：正常結束、沒拿到 id、future 中途被丟棄三種情況。
#[tokio::test]
async fn oneshot_guards_clean_on_every_exit() {
    let scratch = Scratch::new("oneshot");
    let agy = scratch.0.join("agy-home");
    plant_agy(&agy, A);
    plant_agy(&agy, B);
    // 正常結束：回報的 id 刪掉
    let slot = std::sync::Mutex::new(Some(A.to_owned()));
    drop(AgyOneshot {
        home: agy.clone(),
        slot: &slot,
    });
    assert!(gone(&agy, A));
    // 沒拿到 id：什麼都不刪
    let empty = std::sync::Mutex::new(None);
    drop(AgyOneshot {
        home: agy.clone(),
        slot: &empty,
    });
    assert!(intact(&agy, B));
    // future 在等待中被丟棄（呼叫被取消）：guard 照樣刪
    let slot = std::sync::Mutex::new(None);
    let call = async {
        let _cleanup = AgyOneshot {
            home: agy.clone(),
            slot: &slot,
        };
        *slot.lock().unwrap() = Some(B.to_owned());
        std::future::pending::<()>().await;
    };
    assert!(tokio::time::timeout(Duration::from_millis(20), call)
        .await
        .is_err());
    assert!(gone(&agy, B));
    // grok：指定的 session 刪掉
    let grok = scratch.0.join("grok-home");
    let session = grok.join("sessions/%2Fws").join(C);
    fs::create_dir_all(&session).unwrap();
    let call = async {
        let _cleanup = GrokOneshot {
            home: grok.clone(),
            session: C.to_owned(),
        };
        std::future::pending::<()>().await;
    };
    assert!(tokio::time::timeout(Duration::from_millis(20), call)
        .await
        .is_err());
    assert!(!session.exists());
}

/// `.tt-reset-` 讀不到（權限錯誤）：引用集回錯，回收整輪停、一個都不刪。
#[cfg(unix)]
#[test]
fn unreadable_reset_root_stops_the_sweep() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("mark-reset-perm");
    let root = scratch.0.join("data");
    let id = world(&root);
    let reset = root.join("worlds").join(format!(".tt-reset-{id}"));
    lanes_json(&reset.join("worlds").join(&id), &[A]);
    fs::set_permissions(&reset, fs::Permissions::from_mode(0o000)).unwrap();
    let home = scratch.0.join("agy-home");
    plant_agy(&home, A);
    let result = sweep_agy(
        &home,
        || crate::lanes::referenced_session_ids(&root),
        far_future(),
    );
    fs::set_permissions(&reset, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(intact(&home, A));
}

/// 兩個群組其中一個讀不到：判定不了 id 是否唯一，不刪。
#[cfg(unix)]
#[test]
fn grok_session_unreadable_group_blocks_deletion() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("grok-unreadable");
    let home = scratch.0.join("grok-home");
    let own = home.join("sessions/%2Fa").join(A);
    fs::create_dir_all(&own).unwrap();
    let locked = home.join("sessions/%2Fb");
    fs::create_dir_all(locked.join(A)).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let result = remove_session(&home, A);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(own.exists());
    assert!(locked.join(A).exists());
}
