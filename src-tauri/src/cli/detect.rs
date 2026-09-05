use super::types::CliInfo;
use std::path::{Path, PathBuf};
use std::time::Duration;
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
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .unwrap_or_default();
    Some(CliInfo {
        id: id.to_owned(),
        path: path.to_string_lossy().into_owned(),
        version,
    })
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

