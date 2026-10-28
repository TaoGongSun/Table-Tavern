//! 各家的更新與還原指令（plans/cli-version-gate.md 包 2 第 3 點「指令」「更新模式的環境」「四家還原契約」）。
//! 一律用 CLI 自己的指令或官方安裝腳本；還原不靠剛裝上、可能已經壞掉的 CLI 本身。
//! 版本字串只來自 `tested_versions` 常數或解析過的 `semver::Version`，只含數字與點，可安全放進腳本。

use semver::Version;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "version")]
pub(crate) enum Target {
    /// 指定版（已測版）
    Version(String),
    Latest,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Step {
    pub argv: Vec<String>,
    pub envs: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Restore {
    Run(Step),
    /// 本機還留著原版（unix）：把這幾個連結指回去，不用下載
    Relink {
        links: Vec<PathBuf>,
        dest: PathBuf,
    },
}

/// 更新模式一律拿掉的變數：關閉自動更新的（claude、grok）與 app 的假家目錄（grok、codex 要裝進真家目錄，
/// 實測 1.0.46 副本在假 GROK_HOME 下 `grok update` 只裝進假目錄）。系統代理變數保留。
/// agy 例外：帶 app 的 agy-home（`agy update` 換的是被呼叫的那支），由 `Official` 補回。
pub(crate) const REMOVED_ENVS: [&str; 4] = [
    "DISABLE_AUTOUPDATER",
    "GROK_DISABLE_AUTOUPDATER",
    "GROK_HOME",
    "CODEX_HOME",
];

pub(crate) trait Recipes: Send + Sync {
    fn install(&self, cli: &str, binary: &Path, target: &Target) -> Step;
    /// 依序要試的還原方式：本機留存的原版優先、官方腳本殿後。agy 回空：還原靠更新前備份的副本（流程本身處理）。
    fn restore(&self, cli: &str, original: &Version) -> Vec<Restore>;
    /// 還原成功後要清掉的新版檔：(指向它的連結, 只准刪這個目錄裡的)。grok 的下載目錄會留著每一版。
    fn stale_after_restore(&self, _cli: &str) -> Option<(PathBuf, PathBuf)> {
        None
    }
    /// `--version` 與其他非更新呼叫要帶的環境（agy 的 app 假 HOME）。
    fn probe_envs(&self, cli: &str) -> Vec<(String, String)>;
}

/// 正式的指令表。
pub(crate) struct Official {
    /// 使用者真正的家目錄（claude 還原連結用）
    pub home: PathBuf,
    /// app 的 agy-home 環境（`agy_envs`）
    pub agy_envs: Vec<(String, String)>,
}

fn binary_argv(binary: &Path, args: &[&str]) -> Vec<String> {
    std::iter::once(binary.to_string_lossy().into_owned())
        .chain(args.iter().map(|arg| (*arg).to_owned()))
        .collect()
}

// pipefail：`curl | bash` 下載失敗時 bash 讀到空輸入照樣 exit 0，不開就會誤判成功、也觸發不了退路
#[cfg(not(windows))]
fn shell(script: String) -> Vec<String> {
    vec![
        "bash".to_owned(),
        "-o".to_owned(),
        "pipefail".to_owned(),
        "-c".to_owned(),
        script,
    ]
}

// $ErrorActionPreference='Stop'：irm|iex 內部錯誤預設不改 exit code，會誤判成功
#[cfg(windows)]
fn shell(script: String) -> Vec<String> {
    vec![
        "powershell".to_owned(),
        "-NoProfile".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-Command".to_owned(),
        format!("$ErrorActionPreference='Stop'; {script}"),
    ]
}

#[cfg(not(windows))]
const CODEX_SCRIPT: &str = "curl -fsSL https://chatgpt.com/codex/install.sh | sh";
#[cfg(windows)]
const CODEX_SCRIPT: &str = "irm https://chatgpt.com/codex/install.ps1 | iex";

/// codex 官方腳本：版本走 `CODEX_RELEASE`，一律 `CODEX_NON_INTERACTIVE=1`（不問要不要開 TUI、不起 daemon）。
fn codex_step(version: Option<&str>) -> Step {
    let mut envs = vec![("CODEX_NON_INTERACTIVE".to_owned(), "1".to_owned())];
    if let Some(version) = version {
        envs.push(("CODEX_RELEASE".to_owned(), version.to_owned()));
    }
    Step {
        argv: shell(CODEX_SCRIPT.to_owned()),
        envs,
    }
}

/// claude 官方安裝腳本裝指定版（還原與首次安裝用）。
fn claude_script(version: &str) -> Step {
    #[cfg(not(windows))]
    let script = format!("curl -fsSL https://claude.ai/install.sh | bash -s {version}");
    #[cfg(windows)]
    let script =
        format!("& ([scriptblock]::Create((irm https://claude.ai/install.ps1))) {version}");
    Step {
        argv: shell(script),
        envs: Vec::new(),
    }
}

/// grok 官方安裝腳本裝指定版（還原與首次安裝用）：mac `bash -s <ver>`，Windows `GROK_VERSION`。
fn grok_script(version: &str) -> Step {
    #[cfg(not(windows))]
    return Step {
        argv: shell(format!(
            "curl -fsSL https://x.ai/cli/install.sh | bash -s {version}"
        )),
        envs: Vec::new(),
    };
    #[cfg(windows)]
    return Step {
        argv: shell("irm https://x.ai/cli/install.ps1 | iex".to_owned()),
        envs: vec![("GROK_VERSION".to_owned(), version.to_owned())],
    };
}

impl Recipes for Official {
    fn install(&self, cli: &str, binary: &Path, target: &Target) -> Step {
        let version = match target {
            Target::Version(version) => Some(version.as_str()),
            Target::Latest => None,
        };
        match (cli, version) {
            ("claude", Some(version)) => Step {
                argv: binary_argv(binary, &["install", version]),
                envs: Vec::new(),
            },
            ("claude", None) => Step {
                argv: binary_argv(binary, &["update"]),
                envs: Vec::new(),
            },
            ("codex", version) => codex_step(version),
            ("grok", Some(version)) => Step {
                argv: binary_argv(binary, &["update", "--version", version]),
                envs: Vec::new(),
            },
            ("grok", None) => Step {
                argv: binary_argv(binary, &["update"]),
                envs: Vec::new(),
            },
            // agy 只能裝最新版
            _ => Step {
                argv: binary_argv(binary, &["update"]),
                envs: self.agy_envs.clone(),
            },
        }
    }

    fn restore(&self, cli: &str, original: &Version) -> Vec<Restore> {
        let version = original.to_string();
        let mut candidates = Vec::new();
        if cfg!(not(windows)) {
            candidates.extend(self.local_original(cli, &version));
        }
        match cli {
            "claude" => candidates.push(Restore::Run(claude_script(&version))),
            "codex" => candidates.push(Restore::Run(codex_step(Some(&version)))),
            "grok" => candidates.push(Restore::Run(grok_script(&version))),
            _ => {}
        }
        candidates
    }

    fn stale_after_restore(&self, cli: &str) -> Option<(PathBuf, PathBuf)> {
        match cli {
            "grok" if cfg!(not(windows)) => Some((
                self.home.join(".grok/bin/grok"),
                self.home.join(".grok/downloads"),
            )),
            _ => None,
        }
    }

    fn probe_envs(&self, cli: &str) -> Vec<(String, String)> {
        match cli {
            "agy" => self.agy_envs.clone(),
            _ => Vec::new(),
        }
    }
}

impl Official {
    /// 本機還留著的原版（unix 三家都是連結指向版本檔）：
    /// claude `~/.local/share/claude/versions/<v>`；codex `~/.codex/packages/standalone/releases/<v>-<平台>`
    /// （`current` 連結指回去）；grok `~/.grok/downloads/grok-<v>-<平台>`，官方腳本裝的不帶版號
    /// （`grok-<平台>`），也列進來，指回去後一樣以 `--version` 驗證，不對就試下一個。
    fn local_original(&self, cli: &str, version: &str) -> Vec<Restore> {
        let is_link = |path: &Path| path.symlink_metadata().is_ok_and(|meta| meta.is_symlink());
        let entries = |directory: PathBuf| -> Vec<PathBuf> {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
                .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
                .unwrap_or_default();
            paths.sort();
            paths
        };
        let name = |path: &Path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        match cli {
            "claude" => {
                let link = self.home.join(".local/bin/claude");
                let dest = self.home.join(".local/share/claude/versions").join(version);
                match dest.is_file() {
                    true => vec![Restore::Relink {
                        links: vec![link],
                        dest,
                    }],
                    false => Vec::new(),
                }
            }
            "codex" => {
                let standalone = self.home.join(".codex/packages/standalone");
                let current = standalone.join("current");
                if !is_link(&current) {
                    return Vec::new();
                }
                let prefix = format!("{version}-");
                entries(standalone.join("releases"))
                    .into_iter()
                    .filter(|path| name(path).starts_with(&prefix) && path.is_dir())
                    .map(|dest| Restore::Relink {
                        links: vec![current.clone()],
                        dest,
                    })
                    .collect()
            }
            "grok" => {
                let bin = self.home.join(".grok/bin");
                if !is_link(&bin.join("grok")) {
                    return Vec::new();
                }
                let versioned = format!("grok-{version}-");
                let files: Vec<PathBuf> = entries(self.home.join(".grok/downloads"))
                    .into_iter()
                    .filter(|path| path.is_file() && !name(path).contains(".tmp"))
                    .collect();
                // 先試版號對得上的，再試官方腳本裝的不帶版號檔（名字裡沒有數字點數字）
                let unversioned = |path: &PathBuf| {
                    let name = name(path);
                    name.starts_with("grok-")
                        && !name
                            .as_bytes()
                            .windows(3)
                            .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit())
                };
                files
                    .iter()
                    .filter(|path| name(path).starts_with(&versioned))
                    .chain(files.iter().filter(|path| unversioned(path)))
                    .map(|dest| Restore::Relink {
                        links: vec![bin.join("grok"), bin.join("agent")],
                        dest: dest.clone(),
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

/// 首次安裝的官方腳本指令（`commands/cli_setup.rs` 的 mac 終端腳本與 `install.rs` 的 Windows 安裝共用）：
/// claude、codex、grok 帶已測版；agy 只能裝最新版。回 (指定版指令, 最新版指令)。
pub(crate) fn first_install(cli: &str, tested: Option<&str>) -> Option<(Option<Step>, Step)> {
    let latest = match cli {
        "claude" => Step {
            #[cfg(not(windows))]
            argv: shell("curl -fsSL https://claude.ai/install.sh | bash".to_owned()),
            #[cfg(windows)]
            argv: shell("irm https://claude.ai/install.ps1 | iex".to_owned()),
            envs: Vec::new(),
        },
        "codex" => codex_step(None),
        "agy" => Step {
            #[cfg(not(windows))]
            argv: shell("curl -fsSL https://antigravity.google/cli/install.sh | bash".to_owned()),
            #[cfg(windows)]
            argv: shell("irm https://antigravity.google/cli/install.ps1 | iex".to_owned()),
            envs: Vec::new(),
        },
        "grok" => Step {
            #[cfg(not(windows))]
            argv: shell("curl -fsSL https://x.ai/cli/install.sh | bash".to_owned()),
            #[cfg(windows)]
            argv: shell("irm https://x.ai/cli/install.ps1 | iex".to_owned()),
            envs: Vec::new(),
        },
        _ => return None,
    };
    let pinned = match (cli, tested) {
        ("claude", Some(version)) => Some(claude_script(version)),
        ("codex", Some(version)) => Some(codex_step(Some(version))),
        ("grok", Some(version)) => Some(grok_script(version)),
        _ => None,
    };
    Some((pinned, latest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn official() -> Official {
        Official {
            home: PathBuf::from("/nonexistent-home"),
            agy_envs: vec![("HOME".to_owned(), "/app/agy-home".to_owned())],
        }
    }

    fn joined(step: &Step) -> String {
        step.argv.join(" ")
    }

    #[test]
    fn pinned_and_latest_commands_per_cli() {
        let recipes = official();
        let bin = Path::new("/bin/x");
        let v = Target::Version("1.2.3".to_owned());
        assert_eq!(
            joined(&recipes.install("claude", bin, &v)),
            "/bin/x install 1.2.3"
        );
        assert_eq!(
            joined(&recipes.install("claude", bin, &Target::Latest)),
            "/bin/x update"
        );
        assert_eq!(
            joined(&recipes.install("grok", bin, &v)),
            "/bin/x update --version 1.2.3"
        );
        assert_eq!(
            joined(&recipes.install("grok", bin, &Target::Latest)),
            "/bin/x update"
        );
        // agy 不管指定什麼都是最新版，且帶 app 的 agy-home
        let agy = recipes.install("agy", bin, &v);
        assert_eq!(joined(&agy), "/bin/x update");
        assert_eq!(agy.envs, recipes.agy_envs);
        let codex = recipes.install("codex", bin, &v);
        assert!(joined(&codex).contains("codex/install."));
        assert!(codex
            .envs
            .contains(&("CODEX_RELEASE".to_owned(), "1.2.3".to_owned())));
        assert!(codex
            .envs
            .contains(&("CODEX_NON_INTERACTIVE".to_owned(), "1".to_owned())));
        let codex_latest = recipes.install("codex", bin, &Target::Latest);
        assert!(!codex_latest
            .envs
            .iter()
            .any(|(key, _)| key == "CODEX_RELEASE"));
        assert!(codex_latest
            .envs
            .contains(&("CODEX_NON_INTERACTIVE".to_owned(), "1".to_owned())));
    }

    /// 還原一律不呼叫 CLI 本身；本機沒有原版時只剩官方腳本。
    #[test]
    fn restores_never_call_the_cli_itself() {
        let recipes = official();
        let original = Version::new(1, 0, 46);
        for cli in ["claude", "codex", "grok"] {
            let candidates = recipes.restore(cli, &original);
            let [Restore::Run(step)] = candidates.as_slice() else {
                panic!("{cli}：家目錄沒有原版本時應只跑官方腳本");
            };
            assert!(!step.argv[0].ends_with(cli), "{cli}：{:?}", step.argv);
            let text = format!("{} {:?}", joined(step), step.envs);
            assert!(text.contains("1.0.46"), "{cli}：{text}");
        }
        assert!(recipes.restore("agy", &original).is_empty());
    }

    #[cfg(unix)]
    fn temp_home(tag: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("tt-recipe-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        home
    }

    #[cfg(unix)]
    fn last_is_script(candidates: &[Restore]) -> bool {
        matches!(candidates.last(), Some(Restore::Run(_)))
    }

    /// 本機留著原版就先指回去（不下載），官方腳本殿後。
    #[cfg(unix)]
    #[test]
    fn restore_prefers_the_original_kept_on_disk() {
        let home = temp_home("local");
        let recipes = Official {
            home: home.clone(),
            agy_envs: Vec::new(),
        };
        // claude：versions/<v>
        let versions = home.join(".local/share/claude/versions");
        std::fs::create_dir_all(&versions).unwrap();
        std::fs::write(versions.join("2.1.300"), "bin").unwrap();
        let claude = recipes.restore("claude", &Version::new(2, 1, 300));
        assert_eq!(
            claude[0],
            Restore::Relink {
                links: vec![home.join(".local/bin/claude")],
                dest: versions.join("2.1.300"),
            }
        );
        assert!(last_is_script(&claude));
        // codex：current 是連結時，releases/<v>-<平台> 指回去
        let standalone = home.join(".codex/packages/standalone");
        let release = standalone.join("releases/0.163.0-aarch64-apple-darwin");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::create_dir_all(standalone.join("releases/0.164.0-aarch64-apple-darwin")).unwrap();
        std::os::unix::fs::symlink(
            standalone.join("releases/0.164.0-aarch64-apple-darwin"),
            standalone.join("current"),
        )
        .unwrap();
        let codex = recipes.restore("codex", &Version::new(0, 163, 0));
        assert_eq!(
            codex[0],
            Restore::Relink {
                links: vec![standalone.join("current")],
                dest: release,
            }
        );
        assert_eq!(codex.len(), 2);
        // grok：版號對得上的先、不帶版號的（官方腳本裝的）次之，.tmp 與別的版號不列
        let downloads = home.join(".grok/downloads");
        let bin = home.join(".grok/bin");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for name in [
            "grok-1.0.60-macos-aarch64",
            "grok-1.0.70-macos-aarch64",
            "grok-macos-aarch64",
            "grok-macos-aarch64.tmp.123",
        ] {
            std::fs::write(downloads.join(name), "bin").unwrap();
        }
        std::os::unix::fs::symlink(
            downloads.join("grok-1.0.70-macos-aarch64"),
            bin.join("grok"),
        )
        .unwrap();
        let grok = recipes.restore("grok", &Version::new(1, 0, 60));
        let dests: Vec<_> = grok
            .iter()
            .filter_map(|candidate| match candidate {
                Restore::Relink { links, dest } => {
                    assert_eq!(links, &vec![bin.join("grok"), bin.join("agent")]);
                    Some(dest.clone())
                }
                Restore::Run(_) => None,
            })
            .collect();
        assert_eq!(
            dests,
            vec![
                downloads.join("grok-1.0.60-macos-aarch64"),
                downloads.join("grok-macos-aarch64"),
            ]
        );
        assert!(last_is_script(&grok));
        assert_eq!(
            recipes.stale_after_restore("grok"),
            Some((bin.join("grok"), downloads))
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    /// pipefail：下載失敗（curl 非 0）時整條管線非 0，退路才觸發得了。
    #[cfg(unix)]
    #[test]
    fn pipelines_fail_when_the_download_fails() {
        let mut step = shell("false | bash".to_owned());
        let program = step.remove(0);
        let status = std::process::Command::new(program)
            .args(step)
            .status()
            .unwrap();
        assert!(!status.success());
    }

    #[cfg(unix)]
    #[test]
    fn first_install_pins_tested_versions_except_agy() {
        let (pinned, latest) = first_install("claude", Some("2.1.311")).unwrap();
        assert!(joined(&pinned.unwrap()).ends_with("install.sh | bash -s 2.1.311"));
        assert!(joined(&latest).ends_with("install.sh | bash"));
        let (pinned, _) = first_install("grok", Some("1.0.70")).unwrap();
        assert!(joined(&pinned.unwrap()).ends_with("x.ai/cli/install.sh | bash -s 1.0.70"));
        let (pinned, _) = first_install("codex", Some("0.164.0")).unwrap();
        assert!(pinned
            .unwrap()
            .envs
            .contains(&("CODEX_RELEASE".to_owned(), "0.164.0".to_owned())));
        let (pinned, _) = first_install("agy", Some("1.4.9")).unwrap();
        assert!(pinned.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn first_install_pins_tested_versions_on_windows() {
        let (pinned, latest) = first_install("claude", Some("2.1.311")).unwrap();
        let pinned = joined(&pinned.unwrap());
        assert!(pinned.starts_with(
            "powershell -NoProfile -ExecutionPolicy Bypass -Command $ErrorActionPreference='Stop'; "
        ));
        assert!(pinned.contains("claude.ai/install.ps1") && pinned.ends_with(" 2.1.311"));
        assert!(joined(&latest).ends_with("irm https://claude.ai/install.ps1 | iex"));
        let (pinned, _) = first_install("grok", Some("1.0.70")).unwrap();
        let pinned = pinned.unwrap();
        assert!(joined(&pinned).ends_with("irm https://x.ai/cli/install.ps1 | iex"));
        assert!(pinned
            .envs
            .contains(&("GROK_VERSION".to_owned(), "1.0.70".to_owned())));
        let (pinned, _) = first_install("codex", Some("0.164.0")).unwrap();
        assert!(pinned
            .unwrap()
            .envs
            .contains(&("CODEX_RELEASE".to_owned(), "0.164.0".to_owned())));
        let (pinned, _) = first_install("agy", Some("1.4.9")).unwrap();
        assert!(pinned.is_none());
    }

    #[test]
    fn update_mode_drops_auto_update_switches_and_fake_homes() {
        for key in [
            "DISABLE_AUTOUPDATER",
            "GROK_DISABLE_AUTOUPDATER",
            "GROK_HOME",
            "CODEX_HOME",
        ] {
            assert!(REMOVED_ENVS.contains(&key));
        }
        // agy 照帶 app 的 agy-home
        assert_eq!(official().probe_envs("agy"), official().agy_envs);
        assert!(official().probe_envs("grok").is_empty());
    }
}
