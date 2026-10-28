use super::*;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// (暫存根, 真家目錄, codex-home, 目標 auth.json)；目標預設已存在（已登入）
fn setup() -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let dir = Scratch(std::env::temp_dir().join(format!(
        "tt-codex-home-{}-{}",
        std::process::id(),
        ulid::Ulid::generate()
    )));
    let real = dir.0.join("real");
    let target = auth_target(&real);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"{\"tokens\":1}").unwrap();
    let home = dir.0.join("codex-home");
    prepare_home(&home).unwrap();
    (dir, real, home, target)
}

#[test]
fn codex_envs_set_only_codex_home() {
    assert_eq!(
        codex_envs(Path::new("/cfg/codex-home")),
        vec![("CODEX_HOME".to_owned(), "/cfg/codex-home".to_owned())]
    );
}

#[test]
fn prepare_home_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, _, home, _) = setup();
    let mode = std::fs::metadata(&home).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn symlink_is_created_idempotent_and_shares_writes() {
    let (_dir, real, home, target) = setup();
    ensure_auth_link(&home, &real).unwrap();
    let slot = home.join(AUTH_FILE);
    assert_eq!(std::fs::read_link(&slot).unwrap(), target);
    ensure_auth_link(&home, &real).unwrap();
    assert_eq!(std::fs::read_link(&slot).unwrap(), target);
    // 沒留下臨時檔
    assert_eq!(std::fs::read_dir(&home).unwrap().count(), 1);
    // 原地覆寫（codex 的 save 行為）穿過連結寫到使用者那份
    std::fs::write(&slot, b"refreshed").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"refreshed");
}

#[test]
fn symlink_repoints_wrong_target() {
    let (dir, real, home, target) = setup();
    let slot = home.join(AUTH_FILE);
    std::os::unix::fs::symlink(dir.0.join("elsewhere.json"), &slot).unwrap();
    ensure_auth_link(&home, &real).unwrap();
    assert_eq!(std::fs::read_link(&slot).unwrap(), target);
}

/// 沒登入：連結照建、不算錯（模型清單、換幕容量走得到），要跑 codex 時 require_auth 才擋。
#[test]
fn not_logged_in_keeps_link_and_only_require_auth_fails() {
    let (_dir, real, home, target) = setup();
    std::fs::remove_file(&target).unwrap();
    ensure_auth_link(&home, &real).unwrap();
    assert_eq!(std::fs::read_link(home.join(AUTH_FILE)).unwrap(), target);
    let error = require_auth(&real).unwrap_err();
    assert!(error.contains("codex_auth_missing"), "{error}");
    assert!(error.contains(&target.display().to_string()), "{error}");
    // 使用者之後在終端機登入，連結就生效
    std::fs::write(&target, b"login").unwrap();
    ensure_auth_link(&home, &real).unwrap();
    require_auth(&real).unwrap();
}

/// 換帳號：codex 的登入流程先登出（刪檔）再建新檔。登入跑在使用者原本的環境，連結照指同一個路徑，
/// 不會被佔位，app 下一輪直接讀到新帳號。
#[test]
fn switching_account_in_users_home_is_picked_up() {
    let (_dir, real, home, target) = setup();
    ensure_auth_link(&home, &real).unwrap();
    std::fs::remove_file(&target).unwrap();
    std::fs::write(&target, b"new account").unwrap();
    ensure_auth_link(&home, &real).unwrap();
    require_auth(&real).unwrap();
    assert_eq!(std::fs::read(home.join(AUTH_FILE)).unwrap(), b"new account");
}

#[test]
fn real_file_in_slot_is_kept_and_reported() {
    let (_dir, real, home, _) = setup();
    let slot = home.join(AUTH_FILE);
    std::fs::write(&slot, b"someone else's login").unwrap();
    let error = ensure_auth_link(&home, &real).unwrap_err();
    assert!(error.contains("codex_auth_slot_occupied"), "{error}");
    assert!(error.contains(&slot.display().to_string()), "{error}");
    assert_eq!(std::fs::read(&slot).unwrap(), b"someone else's login");
}

fn same_file(a: &Path, b: &Path) -> bool {
    file_identity(a).unwrap() == file_identity(b).unwrap()
}

#[test]
fn hardlink_is_created_with_marker_and_shares_writes() {
    let (_dir, _, home, target) = setup();
    ensure_auth_hardlink(&home, &target).unwrap();
    let slot = home.join(AUTH_FILE);
    assert!(same_file(&slot, &target));
    let marker = std::fs::read_to_string(home.join(HARDLINK_MARKER)).unwrap();
    assert_eq!(marker, file_identity(&target).unwrap());
    // 冪等
    ensure_auth_hardlink(&home, &target).unwrap();
    assert!(same_file(&slot, &target));
    // 原地覆寫兩邊都看得到
    std::fs::write(&slot, b"refreshed").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"refreshed");
}

#[test]
fn detached_hardlink_is_replaced_after_relogin() {
    let (_dir, _, home, target) = setup();
    ensure_auth_hardlink(&home, &target).unwrap();
    // 使用者重新登入：codex 先刪舊檔再建新檔，舊連結分離
    std::fs::remove_file(&target).unwrap();
    std::fs::write(&target, b"new login").unwrap();
    let slot = home.join(AUTH_FILE);
    assert!(!same_file(&slot, &target));
    ensure_auth_hardlink(&home, &target).unwrap();
    assert!(same_file(&slot, &target));
    assert_eq!(std::fs::read(&slot).unwrap(), b"new login");
    assert_eq!(
        std::fs::read_to_string(home.join(HARDLINK_MARKER)).unwrap(),
        file_identity(&target).unwrap()
    );
}

#[test]
fn unrelated_file_or_stale_marker_is_never_replaced() {
    let (_dir, _, home, target) = setup();
    let slot = home.join(AUTH_FILE);
    // 沒有標記的實體檔
    std::fs::write(&slot, b"keep").unwrap();
    let error = ensure_auth_hardlink(&home, &target).unwrap_err();
    assert!(error.contains("codex_auth_slot_occupied"), "{error}");
    assert_eq!(std::fs::read(&slot).unwrap(), b"keep");
    // 標記在、但記的不是槽位現在那個檔：一樣不碰
    std::fs::write(home.join(HARDLINK_MARKER), "0:0").unwrap();
    let error = ensure_auth_hardlink(&home, &target).unwrap_err();
    assert!(error.contains("codex_auth_slot_occupied"), "{error}");
    assert_eq!(std::fs::read(&slot).unwrap(), b"keep");
}

#[test]
fn same_file_without_marker_gets_marker() {
    let (_dir, _, home, target) = setup();
    std::fs::hard_link(&target, home.join(AUTH_FILE)).unwrap();
    ensure_auth_hardlink(&home, &target).unwrap();
    assert_eq!(
        std::fs::read_to_string(home.join(HARDLINK_MARKER)).unwrap(),
        file_identity(&target).unwrap()
    );
}

#[test]
fn missing_target_clears_only_our_old_hardlink() {
    let (_dir, _, home, target) = setup();
    ensure_auth_hardlink(&home, &target).unwrap();
    // 使用者在終端機登出：目標被刪，我們的舊連結清掉
    std::fs::remove_file(&target).unwrap();
    ensure_auth_hardlink(&home, &target).unwrap();
    assert!(!home.join(AUTH_FILE).exists());
    assert!(!home.join(HARDLINK_MARKER).exists());
    // 槽位上是不相干的檔：不刪、回錯
    std::fs::write(home.join(AUTH_FILE), b"keep").unwrap();
    let error = ensure_auth_hardlink(&home, &target).unwrap_err();
    assert!(error.contains("codex_auth_slot_occupied"), "{error}");
    assert_eq!(std::fs::read(home.join(AUTH_FILE)).unwrap(), b"keep");
}
