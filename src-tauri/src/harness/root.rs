//! 測試根目錄驗證、identifier 層級互斥鎖、`--fresh` 清理。全部在建出 Tauri 之前跑完。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub(crate) const HARNESS_IDENTIFIER: &str = "com.tabletavern.app.harness";

/// 測試包會讀寫、而且必須與 root 完全不重疊的固定位置。
pub(super) struct Layout {
    pub home: PathBuf,
    /// 正式版的 data_root 與 config_root。
    pub production: Vec<PathBuf>,
    /// 鎖檔所在的控制目錄：不屬於任何清理範圍。
    pub control: PathBuf,
    /// 依 identifier 存、`--fresh` 要一併清的共用目錄。
    pub shared: Vec<PathBuf>,
}

impl Layout {
    pub fn from_home(home: PathBuf) -> Self {
        let support = home.join("Library").join("Application Support");
        Self {
            production: vec![
                home.join("Documents").join("TableTavern"),
                support.join("TableTavern"),
            ],
            control: support.join(format!("{HARNESS_IDENTIFIER}.control")),
            shared: vec![
                support.join(HARNESS_IDENTIFIER),
                home.join("Library").join("WebKit").join(HARNESS_IDENTIFIER),
                home.join("Library").join("Caches").join(HARNESS_IDENTIFIER),
            ],
            home,
        }
    }
}

/// 解析系統別名（/var → /private/var）後的絕對路徑；路徑不存在時解析最近存在的祖先再接回剩餘段。
pub(super) fn canonical_lenient(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "路徑必須是絕對路徑",
        ));
    }
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match fs::canonicalize(&existing) {
            Ok(canonical) => {
                let mut out = canonical;
                for part in rest.iter().rev() {
                    // push 會重新解析尾段：Windows verbatim 路徑裡的 `a/x/../b` 在這裡才被拆開、
                    // `..` 被吃掉，結果繞過別名解析。尾段必須原樣就是單一一段名稱。
                    let reparsed: Vec<Component> = Path::new(part).components().collect();
                    if !matches!(reparsed.as_slice(), [Component::Normal(name)] if name == part) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("路徑段不合法：{}", Path::new(part).display()),
                        ));
                    }
                    out.push(part);
                }
                return Ok(out);
            }
            Err(_) => {
                let Some(name) = existing.file_name().map(|n| n.to_owned()) else {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "找不到存在的祖先"));
                };
                rest.push(name);
                if !existing.pop() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "找不到存在的祖先"));
                }
            }
        }
    }
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// root 合法才回 canonical 路徑。規則見 .ai/plans/test-harness.md「資料隔離與啟停」。
pub(super) fn validate_root(raw: &str, layout: &Layout) -> Result<PathBuf, String> {
    let raw_path = Path::new(raw);
    if raw.trim().is_empty() || !raw_path.is_absolute() {
        return Err(format!("TT_HARNESS_ROOT 必須是絕對路徑：{raw:?}"));
    }
    if raw_path
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(format!("TT_HARNESS_ROOT 不得含 ..：{raw}"));
    }
    check_destination(raw_path, layout)
}

/// 持鎖後、任何寫入與背景工作之前：root（可能是重用的）內不得有 symlink，建 data／config，
/// 確認實際寫入目的地仍在 root 內且不碰受保護路徑，再視需要複製設定檔。
pub(super) fn prepare(
    root: &Path,
    layout: &Layout,
    config_from: Option<&Path>,
) -> Result<(), String> {
    reject_symlinks(root)?;
    let data = root.join("data");
    let config = root.join("config");
    for dir in [root, data.as_path(), config.as_path()] {
        fs::create_dir_all(dir).map_err(|e| format!("建立 {} 失敗：{e}", dir.display()))?;
    }
    reject_symlinks(root)?;
    for dir in [&data, &config] {
        let actual = check_destination(dir, layout)?;
        if !actual.starts_with(root) {
            return Err(format!(
                "{} 實際落在 root 外：{}",
                dir.display(),
                actual.display()
            ));
        }
    }
    set_mode(root, 0o700).map_err(|e| format!("設定 root 權限失敗：{e}"))?;
    if let Some(from) = config_from {
        let target = config.join("config.json");
        let actual = check_destination(&target, layout)?;
        if !actual.starts_with(root)
            || fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(format!("設定檔目的地不安全：{}", target.display()));
        }
        fs::copy(from, &target).map_err(|e| format!("複製設定 {} 失敗：{e}", from.display()))?;
        set_mode(&target, 0o600).map_err(|e| format!("設定檔權限失敗：{e}"))?;
    }
    Ok(())
}

/// 任何會被寫入的路徑：canonical 後不得是家目錄或其祖先，也不得與受保護路徑重疊。回 canonical 路徑。
pub(super) fn check_destination(path: &Path, layout: &Layout) -> Result<PathBuf, String> {
    let target =
        canonical_lenient(path).map_err(|e| format!("解析 {} 失敗：{e}", path.display()))?;
    let home = canonical_lenient(&layout.home).map_err(|e| format!("解析家目錄失敗：{e}"))?;
    if home.starts_with(&target) {
        return Err(format!("{} 不得是家目錄或其祖先", target.display()));
    }
    let guarded = layout
        .production
        .iter()
        .chain(std::iter::once(&layout.control))
        .chain(layout.shared.iter());
    for guard in guarded {
        let canonical =
            canonical_lenient(guard).map_err(|e| format!("解析 {} 失敗：{e}", guard.display()))?;
        if overlaps(&target, &canonical) {
            return Err(format!(
                "{} 與受保護路徑 {} 重疊",
                target.display(),
                canonical.display()
            ));
        }
    }
    Ok(target)
}

/// 重用的 root 裡若有 symlink（不論深淺），寫入就可能被導到 root 外：一律拒絕。不跟隨 symlink。
/// 只有尚不存在的頂層 root 可略過；既有目錄掃不完（權限不足等）就拒絕，不當成「沒有 symlink」。
pub(super) fn reject_symlinks(root: &Path) -> Result<(), String> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("無法檢查 {}：{error}", root.display())),
        Ok(_) => scan_for_symlinks(root),
    }
}

fn scan_for_symlinks(dir: &Path) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("無法掃描 {}：{error}", dir.display());
    for entry in fs::read_dir(dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("無法檢查 {}：{e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("root 內不得有 symlink：{}", path.display()));
        }
        if meta.is_dir() {
            scan_for_symlinks(&path)?;
        }
    }
    Ok(())
}

/// 取 identifier 層級排他鎖；取不到回傳鎖檔內記載的持有者。鎖檔永不刪除。
pub(super) fn acquire_lock(control: &Path, holder: &str) -> Result<File, String> {
    fs::create_dir_all(control).map_err(|e| format!("建立控制目錄失敗：{e}"))?;
    let path = control.join("harness.lock");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("開啟鎖檔失敗：{e}"))?;
    if let Err(error) = file.try_lock() {
        let mut owner = String::new();
        let _ = file.read_to_string(&mut owner);
        return Err(format!(
            "已有測試實例持有 {}（{error}）：{}",
            path.display(),
            owner.trim()
        ));
    }
    file.set_len(0).map_err(|e| format!("寫鎖檔失敗：{e}"))?;
    file.write_all(holder.as_bytes())
        .and_then(|_| file.flush())
        .map_err(|e| format!("寫鎖檔失敗：{e}"))?;
    Ok(file)
}

/// 只在持鎖時呼叫：清 root 內容（不刪 root 本身）與依 identifier 存的共用目錄。不跟 symlink。
pub(super) fn wipe(root: &Path, shared: &[PathBuf]) -> Result<(), String> {
    if root.exists() {
        for entry in fs::read_dir(root).map_err(|e| format!("讀 root 失敗：{e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            remove_entry(&entry.path())?;
        }
    }
    for dir in shared {
        if fs::symlink_metadata(dir).is_ok() {
            remove_entry(dir)?;
        }
    }
    Ok(())
}

fn remove_entry(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let result = if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|e| format!("刪除 {} 失敗：{e}", path.display()))
}

#[cfg(unix)]
pub(super) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
pub(super) fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tt-harness-root-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    fn layout(home: &Path) -> Layout {
        Layout::from_home(home.to_path_buf())
    }

    #[test]
    fn tmpdir_root_is_accepted_through_symlinked_var() {
        let home = scratch("home-a");
        // $TMPDIR 在 /var/folders，/var 是 symlink：不存在的 root 也要能解析通過。
        let raw = std::env::temp_dir().join("tt-harness-not-yet-created/inner");
        let root = validate_root(raw.to_str().unwrap(), &layout(&home)).unwrap();
        assert!(root.ends_with("tt-harness-not-yet-created/inner"));
        assert!(
            !root.starts_with("/var/"),
            "應解析成 canonical：{}",
            root.display()
        );
    }

    #[test]
    fn dangerous_roots_are_rejected() {
        let home = scratch("home-b");
        let l = layout(&home);
        let support = home.join("Library/Application Support");
        for bad in [
            "/".to_owned(),
            home.display().to_string(),
            home.join("Documents").display().to_string(),
            home.join("Documents/TableTavern/sub").display().to_string(),
            support.join("TableTavern").display().to_string(),
            support.display().to_string(),
            support
                .join(format!("{HARNESS_IDENTIFIER}.control"))
                .display()
                .to_string(),
            home.join("Library/WebKit")
                .join(HARNESS_IDENTIFIER)
                .display()
                .to_string(),
            "relative/path".to_owned(),
            format!("{}/x/../y", home.display()),
        ] {
            assert!(validate_root(&bad, &l).is_err(), "應拒絕 {bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_alias_into_production_is_rejected() {
        let home = scratch("home-c");
        let docs = home.join("Documents/TableTavern");
        fs::create_dir_all(&docs).unwrap();
        let alias = home.join("alias");
        std::os::unix::fs::symlink(&docs, &alias).unwrap();
        assert!(validate_root(alias.join("x").to_str().unwrap(), &layout(&home)).is_err());
    }

    /// Windows：junction 指向假正式資料，verbatim 輸入用 `/x/../` 讓尾段在接回時才被拆開。
    /// 驗 root 是啟動第一步，在這裡就拒絕，取鎖、`--fresh` 清理與任何寫入都不會發生。
    #[cfg(windows)]
    #[test]
    fn verbatim_tail_cannot_hide_a_junction_into_production() {
        let home = scratch("home-junction");
        let prod = home.join("Documents").join("TableTavern");
        fs::create_dir_all(prod.join("sub")).unwrap();
        fs::write(prod.join("sub").join("keep.txt"), "PROD").unwrap();
        let alias = home.join("alias");
        let plain = |p: &Path| {
            p.display()
                .to_string()
                .trim_start_matches(r"\\?\")
                .to_owned()
        };
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &plain(&alias), &plain(&prod)])
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "建不了 junction：{}",
            String::from_utf8_lossy(&made.stderr)
        );
        assert_eq!(
            fs::read_to_string(alias.join("sub").join("keep.txt")).unwrap(),
            "PROD"
        );
        let raw = format!("{}\\alias/x/../sub", home.display());
        assert!(raw.starts_with(r"\\?\"), "要測 verbatim 輸入：{raw}");
        assert!(validate_root(&raw, &layout(&home)).is_err(), "應拒絕 {raw}");
        assert_eq!(
            fs::read_to_string(prod.join("sub").join("keep.txt")).unwrap(),
            "PROD"
        );
        fs::remove_dir(&alias).unwrap();
    }

    /// 假家目錄＋假正式資料，回 (layout, 正式 data, 正式 config.json, 可重用的 root)。
    #[cfg(unix)]
    fn reuse_fixture(tag: &str) -> (Layout, PathBuf, PathBuf, PathBuf) {
        let home = scratch(tag);
        let prod_data = home.join("Documents/TableTavern");
        let prod_config = home.join("Library/Application Support/TableTavern");
        fs::create_dir_all(&prod_data).unwrap();
        fs::create_dir_all(&prod_config).unwrap();
        let prod_json = prod_config.join("config.json");
        fs::write(&prod_json, "PROD").unwrap();
        let root = home.join("work/root");
        fs::create_dir_all(&root).unwrap();
        (layout(&home), prod_data, prod_json, root)
    }

    #[cfg(unix)]
    #[test]
    fn reused_root_with_data_symlink_into_production_is_rejected() {
        let (l, prod_data, _, root) = reuse_fixture("reuse-data");
        std::os::unix::fs::symlink(&prod_data, root.join("data")).unwrap();
        assert!(prepare(&root, &l, None).is_err());
        assert_eq!(fs::read_dir(&prod_data).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn config_json_symlink_is_not_followed_by_copy() {
        let (l, _, prod_json, root) = reuse_fixture("reuse-json");
        fs::create_dir_all(root.join("config")).unwrap();
        std::os::unix::fs::symlink(&prod_json, root.join("config/config.json")).unwrap();
        let from = root.parent().unwrap().join("from.json");
        fs::write(&from, "TEST").unwrap();
        assert!(prepare(&root, &l, Some(&from)).is_err());
        assert_eq!(fs::read_to_string(&prod_json).unwrap(), "PROD");
    }

    #[cfg(unix)]
    #[test]
    fn nested_symlink_in_reused_root_is_rejected() {
        let (l, prod_data, _, root) = reuse_fixture("reuse-nested");
        fs::create_dir_all(root.join("data/worlds")).unwrap();
        std::os::unix::fs::symlink(&prod_data, root.join("data/worlds/w1")).unwrap();
        assert!(prepare(&root, &l, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unlistable_directory_in_reused_root_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let (l, prod_data, _, root) = reuse_fixture("reuse-unlistable");
        let hidden = root.join("data/worlds");
        fs::create_dir_all(&hidden).unwrap();
        std::os::unix::fs::symlink(&prod_data, hidden.join("w1")).unwrap();
        // 只可穿越、不可列：read_dir 失敗，但路徑仍能被寫入穿過。
        fs::set_permissions(&hidden, fs::Permissions::from_mode(0o111)).unwrap();
        let result = prepare(&root, &l, None);
        fs::set_permissions(&hidden, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err(), "掃不完的目錄必須拒絕");
        assert_eq!(fs::read_dir(&prod_data).unwrap().count(), 0);
    }

    #[test]
    fn missing_root_is_skipped_by_symlink_scan() {
        let base = scratch("scan-missing");
        assert!(reject_symlinks(&base.join("not-yet")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn clean_reused_root_prepares_and_copies_private_config() {
        use std::os::unix::fs::PermissionsExt;
        let (l, _, prod_json, root) = reuse_fixture("reuse-ok");
        fs::create_dir_all(root.join("data/worlds")).unwrap();
        let from = root.parent().unwrap().join("from.json");
        fs::write(&from, "TEST").unwrap();
        prepare(&root, &l, Some(&from)).unwrap();
        let copied = root.join("config/config.json");
        assert_eq!(fs::read_to_string(&copied).unwrap(), "TEST");
        assert_eq!(
            fs::metadata(&copied).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::read_to_string(&prod_json).unwrap(), "PROD");
    }

    #[test]
    fn lock_is_exclusive_and_file_survives_release() {
        let control = scratch("control");
        let first = acquire_lock(&control, "pid=1").unwrap();
        let err = acquire_lock(&control, "pid=2").unwrap_err();
        // 能讀取時附上持有者：Windows 是強制鎖，被鎖住時讀不到，只驗排他性。
        if cfg!(unix) {
            assert!(err.contains("pid=1"), "{err}");
        }
        drop(first);
        assert!(control.join("harness.lock").exists());
        acquire_lock(&control, "pid=3").unwrap();
    }

    #[test]
    fn wipe_clears_contents_but_keeps_root_and_skips_symlink_targets() {
        let base = scratch("wipe");
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(root.join("data/a")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep.txt"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let shared = base.join("shared");
        fs::create_dir_all(shared.join("x")).unwrap();
        wipe(&root, std::slice::from_ref(&shared)).unwrap();
        assert!(root.exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert!(!shared.exists());
        assert!(outside.join("keep.txt").exists());
    }
}
