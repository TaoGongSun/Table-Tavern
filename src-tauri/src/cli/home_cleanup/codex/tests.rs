use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

const A: &str = "01a18237-6c98-7783-9d89-ca15d7ebe9b8";
const B: &str = "01a18237-7d66-7b12-b828-cf8c71696a7e";
const C: &str = "01a18237-8e13-70b3-82ad-48b0f02ad690";
const D: &str = "01a18237-9f00-7000-8000-000000000004";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "tt-codex-cleanup-{tag}-{}",
            nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn set_mtime(path: &Path, when: SystemTime) {
    fs::File::open(path).unwrap().set_modified(when).unwrap();
}

fn far_future() -> SystemTime {
    SystemTime::now() + RETENTION + Duration::from_secs(3600)
}

fn rollout(home: &Path, day: &str, id: &str) -> PathBuf {
    let dir = home.join("sessions").join(day);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("rollout-2026-10-01T10-00-00-{id}.jsonl"));
    fs::write(&path, "{}").unwrap();
    path
}

#[test]
fn rollout_names() {
    assert_eq!(
        rollout_id(&format!("rollout-2026-10-28T19-13-16-{A}.jsonl")),
        Some(A)
    );
    assert_eq!(
        rollout_id(&format!("rollout-2026-10-28T19-13-16-{A}.jsonl.zst")),
        Some(A)
    );
    assert_eq!(rollout_id(&format!("{A}.jsonl")), None);
    assert_eq!(rollout_id(&format!("rollout-x{A}.jsonl")), None);
    assert_eq!(rollout_id(&format!("rollout-2026-{A}.json")), None);
    assert_eq!(
        rollout_id("rollout-2026-not-a-uuid-at-all-0000000000000.jsonl"),
        None
    );
}

/// 引用集內、7 天內、日期層是連結的都不刪；生圖目錄整樹都舊才刪。
#[test]
fn sweep_keeps_referenced_recent_and_linked() {
    let scratch = Scratch::new("sweep");
    let home = scratch.0.join("codex-home");
    let now = far_future();
    rollout(&home, "2026/10/01", A);
    rollout(&home, "2026/10/01", B);
    let recent = rollout(&home, "2026/10/02", C);
    set_mtime(&recent, now);
    // 日期層是連結：不跟進去
    let outside = scratch.0.join("outside");
    rollout(&outside, "2026/10/03", D);
    std::os::unix::fs::symlink(
        outside.join("sessions/2026/10"),
        home.join("sessions/2026/11"),
    )
    .unwrap();
    let images = home.join("generated_images");
    fs::create_dir_all(images.join(A)).unwrap();
    fs::write(images.join(A).join("call_1.png"), "png").unwrap();
    fs::create_dir_all(images.join(C)).unwrap();
    let fresh = images.join(C).join("call_2.png");
    fs::write(&fresh, "png").unwrap();
    set_mtime(&fresh, now);

    let mut asked = Vec::new();
    let result = sweep_codex(
        &home,
        || Ok(HashSet::from([B.to_owned()])),
        now,
        |id| {
            asked.push(id.to_owned());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result, (1, 1));
    assert_eq!(asked, vec![A.to_owned()]);
    assert!(!images.join(A).exists());
    assert!(fresh.exists());
}

#[test]
fn sweep_stops_on_mark_failure_and_rechecks_before_delete() {
    let scratch = Scratch::new("mark");
    let home = scratch.0.join("codex-home");
    let now = far_future();
    let path = rollout(&home, "2026/10/01", A);
    let mut asked = 0;
    assert!(sweep_codex(
        &home,
        || Err("壞了".to_owned()),
        now,
        |_| {
            asked += 1;
            Ok(())
        }
    )
    .is_err());
    // 候選列出後才被動到（正在續聊）：重新 stat 看到新 mtime，不刪
    let result = sweep_codex(
        &home,
        || {
            set_mtime(&path, now);
            Ok(HashSet::new())
        },
        now,
        |_| {
            asked += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result, (0, 0));
    assert_eq!(asked, 0);
}

/// 家目錄本身是連結：不清。
#[test]
fn linked_home_is_refused() {
    let scratch = Scratch::new("linked");
    let real = scratch.0.join("real");
    rollout(&real, "2026/10/01", A);
    let home = scratch.0.join("codex-home");
    std::os::unix::fs::symlink(&real, &home).unwrap();
    assert!(sweep_codex(&home, || Ok(HashSet::new()), far_future(), |_| Ok(())).is_err());
}

/// 假 codex：腳本內容自訂，`$LOG` 是共用紀錄檔。
fn fake_codex(dir: &Path, body: &str) -> PathBuf {
    fake_codex_named(dir, "codex", body)
}

fn fake_codex_named(dir: &Path, name: &str, body: &str) -> PathBuf {
    let script = dir.join(name);
    fs::write(
        &script,
        format!("#!/bin/bash\nLOG={}\n{body}\n", dir.join("log").display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn log_lines(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn delete_passes_codex_home_and_reports_rc() {
    let _serial = crate::inflight::lock_real_process_tests();
    let scratch = Scratch::new("delete");
    let home = scratch.0.join("codex-home");
    fs::create_dir_all(&home).unwrap();
    let ok = fake_codex(&scratch.0, r#"echo "$* $CODEX_HOME $PWD" >> $LOG; exit 0"#);
    delete_thread(&ok, &home, &scratch.0, A).await.unwrap();
    let line = &log_lines(&scratch.0)[0];
    assert!(
        line.starts_with(&format!("delete {A} --force {}", home.display())),
        "{line}"
    );
    let fail = fake_codex(
        &scratch.0,
        r#"echo "Error: failed to delete session" >&2; exit 1"#,
    );
    let error = delete_thread(&fail, &home, &scratch.0, A)
        .await
        .unwrap_err();
    assert!(error.contains("failed to delete session"), "{error}");
    // spawn 失敗只回錯
    assert!(
        delete_thread(&scratch.0.join("missing"), &home, &scratch.0, A)
            .await
            .is_err()
    );
    // 不合法的 id、沒有 codex-home：不呼叫
    assert!(delete_thread(&ok, &home, &scratch.0, "../x").await.is_err());
    let before = log_lines(&scratch.0).len();
    delete_thread(&ok, &scratch.0.join("none"), &scratch.0, A)
        .await
        .unwrap();
    assert_eq!(log_lines(&scratch.0).len(), before);
}

/// 前一次逾時或呼叫端被取消時，下一次要等前一個程序真的退出（收屍）才啟動。下一個假 codex
/// 啟動時用 `kill -0` 查前一個的 pid：沒收屍的殭屍程序也算還在。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn next_delete_waits_for_previous_process_to_exit() {
    let _serial = crate::inflight::lock_real_process_tests();
    let scratch = Scratch::new("serial");
    let home = scratch.0.join("codex-home");
    fs::create_dir_all(&home).unwrap();
    let slow = fake_codex(&scratch.0, r#"echo "start $2 $$" >> $LOG; exec sleep 30"#);
    let probe = fake_codex_named(
        &scratch.0,
        "probe",
        r#"for pid in $(awk '/^start/ {print $3}' $LOG); do
  if kill -0 $pid 2>/dev/null; then echo "alive $pid" >> $LOG; fi
done
echo "probe $2" >> $LOG"#,
    );
    // ① 逾時：kill＋wait 收屍後才返回
    let error = delete_thread_within(&slow, &home, &scratch.0, A, Duration::from_secs(3))
        .await
        .unwrap_err();
    assert!(error.contains("超過"), "{error}");
    delete_thread(&probe, &home, &scratch.0, C).await.unwrap();
    // ② 呼叫端被取消：刪除任務照樣持鎖做完（等到自己的逾時、kill＋wait），下一次排在後面
    let started = std::time::Instant::now();
    let cancelled = tokio::time::timeout(
        Duration::from_millis(800),
        delete_thread_within(&slow, &home, &scratch.0, B, Duration::from_secs(3)),
    )
    .await;
    assert!(cancelled.is_err());
    delete_thread(&probe, &home, &scratch.0, D).await.unwrap();
    assert!(
        started.elapsed() >= Duration::from_secs(3),
        "取消後刪除任務沒有做完就放鎖"
    );
    // 不寫死行數（慢的假 codex 在被殺前不一定來得及寫 start）：探針沒看到還活著的程序，
    // 而且 start A → probe C → start B → probe D 的先後不亂
    let lines = log_lines(&scratch.0);
    assert!(
        !lines.iter().any(|line| line.starts_with("alive")),
        "{lines:?}"
    );
    let at = |prefix: String| lines.iter().position(|line| line.starts_with(&prefix));
    let probe_c = at(format!("probe {C}")).expect("probe C");
    let probe_d = at(format!("probe {D}")).expect("probe D");
    assert!(probe_c < probe_d, "{lines:?}");
    if let Some(start_a) = at(format!("start {A}")) {
        assert!(start_a < probe_c, "{lines:?}");
    }
    if let Some(start_b) = at(format!("start {B}")) {
        assert!(probe_c < start_b && start_b < probe_d, "{lines:?}");
    }
}
