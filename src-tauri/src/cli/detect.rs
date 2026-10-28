use super::types::CliInfo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tokio::time::timeout;

fn candidate_dirs() -> Vec<PathBuf> {
    // PATH 之外補常見安裝位置：GUI App 由 Finder 啟動時 PATH 往往不含它們
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".claude/local"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            dirs.push(profile.join(".local").join("bin"));
            dirs.push(profile.join(".grok").join("bin"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(
                local
                    .join("Programs")
                    .join("OpenAI")
                    .join("Codex")
                    .join("bin"),
            );
            dirs.push(local.join("agy").join("bin"));
        }
    }
    dirs
}

fn is_executable(path: &Path) -> bool {
    // 執行位元檢查僅限 unix；Windows（日後支援）改查副檔名慣例即可，先保住編譯路
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub(crate) fn find_binary(name: &str) -> Option<PathBuf> {
    // Windows 執行檔帶 .exe 副檔名（四家官方安裝器皆落 .exe）
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    candidate_dirs()
        .into_iter()
        .map(|directory| directory.join(&filename))
        .find(|path| is_executable(path))
}

/// 跑 `<program> <arg>` 取 stdout，Windows 下隱藏主控台視窗。
/// 10 秒上限＋kill_on_drop：agy／grok 的 models 是即時網路查詢，卡住時不能無限期
/// 掛著呼叫端（比 probe_cli 的 5 秒寬，因為這條路在背景跑、慢網路多等無妨）。
pub(super) async fn hidden_output(
    program: PathBuf,
    arg: &str,
    envs: &[(String, String)],
) -> Option<std::process::Output> {
    let mut command = Command::new(program);
    command.arg(arg).kill_on_drop(true).envs(
        envs.iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    timeout(Duration::from_secs(10), command.output())
        .await
        .ok()
        .and_then(Result::ok)
        .filter(|output| output.status.success())
}

/// 單支 CLI 探測。5 秒上限＋kill_on_drop：某支卡住只損失自己，不拖垮其餘三支。
async fn probe_cli(id: &str) -> Option<CliInfo> {
    let path = find_binary(id)?;
    let mut command = Command::new(&path);
    command.arg("--version").kill_on_drop(true);
    // GUI app 下 console 子程序會閃出黑視窗，一律 CREATE_NO_WINDOW
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let version = timeout(Duration::from_secs(5), command.output())
        .await
        .ok()
        .and_then(Result::ok)
        .filter(|output| output.status.success())
        .map(|output| first_line(&output.stdout))
        .unwrap_or_default();
    Some(CliInfo {
        id: id.to_owned(),
        path: path.to_string_lossy().into_owned(),
        version,
    })
}

/// `--version` 輸出取第一行（設定頁的 CLI 偵測與帳本的 `cli_version` 共用）。
fn first_line(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// 帳本 `cli_version` 用的版本快取：鍵＝（執行檔路徑, 修改時間），CLI 一升級 mtime 就變、自動重查。
/// 只記成功的值；失敗下次呼叫再試。
fn version_cache() -> &'static Mutex<HashMap<(PathBuf, SystemTime), String>> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, SystemTime), String>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// 在與正式呼叫相同的執行環境（系統代理、去 ANTHROPIC_*、同一組 envs 與工作目錄）跑
/// `<program> --version` 取第一行。5 秒上限、失敗回 None，從不阻斷呼叫本身。
pub(crate) async fn cli_version(
    program: PathBuf,
    working_dir: PathBuf,
    envs: Vec<(String, String)>,
) -> Option<String> {
    let modified = std::fs::metadata(&program)
        .and_then(|meta| meta.modified())
        .ok()?;
    let key = (program.clone(), modified);
    if let Some(version) = version_cache().lock().ok()?.get(&key) {
        return Some(version.clone());
    }
    let mut command = Command::new(&program);
    super::proxy::apply_system_proxy(&mut command);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("ANTHROPIC_") {
            command.env_remove(&name);
        }
    }
    command
        .current_dir(&working_dir)
        .arg("--version")
        .envs(envs)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let version = timeout(Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()
        .filter(|output| output.status.success())
        .map(|output| first_line(&output.stdout))
        .filter(|version| !version.is_empty())?;
    version_cache().lock().ok()?.insert(key, version.clone());
    Some(version)
}

pub async fn detect_clis() -> Vec<CliInfo> {
    // 四支並行：總耗時取決於最慢一支而非累加（序列版遇冷啟動／防毒即時掃描要等數十秒）
    let (claude, codex, agy, grok) = tokio::join!(
        probe_cli("claude"),
        probe_cli("codex"),
        probe_cli("agy"),
        probe_cli("grok"),
    );
    [claude, codex, agy, grok].into_iter().flatten().collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// 第一行原樣記下；同一支執行檔沒換過就不再跑第二次，換過（mtime 變）才重查
    #[tokio::test]
    async fn cli_version_is_cached_per_binary_and_mtime() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tt-cli-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-cli");
        let counter = dir.join("count");
        let write = |version: &str| {
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\necho x >> \"$COUNT_FILE\"\necho '{version}'\necho 'second line'\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        write("codex-cli 0.164.0");
        let envs = vec![(
            "COUNT_FILE".to_owned(),
            counter.to_string_lossy().into_owned(),
        )];
        let run = || cli_version(script.clone(), dir.clone(), envs.clone());
        assert_eq!(run().await.as_deref(), Some("codex-cli 0.164.0"));
        assert_eq!(run().await.as_deref(), Some("codex-cli 0.164.0"));
        let calls = || std::fs::read_to_string(&counter).unwrap().lines().count();
        assert_eq!(calls(), 1);
        // 換版：改內容並把 mtime 推後，避免同一秒內改檔時間相同
        write("codex-cli 0.165.0");
        let later = SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&script)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(run().await.as_deref(), Some("codex-cli 0.165.0"));
        assert_eq!(calls(), 2);
        // 跑不起來＝None，不擋呼叫
        assert_eq!(
            cli_version(dir.join("missing"), dir.clone(), Vec::new()).await,
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
