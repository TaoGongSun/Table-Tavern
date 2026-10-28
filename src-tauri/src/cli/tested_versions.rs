//! app 測過的 CLI 版本與版本門檻（plans/cli-version-gate.md 包 2 第 5 點）。
//! 維護：用 `stream/capture_tests.rs` 錄好新版夾具、契約測試全過之後才改這裡的常數；
//! 綁定測試會擋下「常數改了卻沒錄夾具」與「夾具換版卻沒改常數」。agy 只能裝最新版，常數只供診斷。

use semver::Version;

/// 四家測過的版本（裸版號，與夾具 `.version` 第一行解析出的版號逐字相同）。
pub(crate) fn tested(cli: &str) -> Option<&'static str> {
    match cli {
        "claude" => Some("2.1.311"),
        "codex" => Some("0.164.0"),
        "agy" => Some("1.4.9"),
        "grok" => Some("1.0.70"),
        _ => None,
    }
}

/// app 支援的最低版本；之後 app 開始用某個新旗標時在這裡同步加門檻。
/// agy 1.2.1：`--output-format stream-json`（1.1.8）、`--agent`（1.1.1）與自訂代理的
/// `excludeDefaultComponents`（1.2.1）都要有，精簡工具集代理才成立。
pub(crate) fn minimum(cli: &str) -> Option<&'static str> {
    match cli {
        "agy" => Some("1.2.1"),
        _ => None,
    }
}

/// 從 `--version` 第一行抽出 x.y.z（`2.1.311 (Claude Code)`、`codex-cli 0.164.0`、
/// `grok 1.0.70 (…) [stable]`、`1.4.9`）。抽不出三段數字回 None。
pub(crate) fn parse(line: &str) -> Option<Version> {
    let numbers: Vec<u64> = line
        .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .find_map(|token| {
            let parts: Vec<u64> = token
                .split('.')
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().ok())
                .collect::<Option<Vec<_>>>()?;
            (parts.len() >= 3).then_some(parts)
        })?;
    Some(Version::new(numbers[0], numbers[1], numbers[2]))
}

/// 版本低於門檻：回門檻字串；版本字串解析不出來時放行（寧可讓 CLI 自己報錯，也不因版本字串
/// 換了格式就把整條路擋死）。
pub(crate) fn below_minimum(cli: &str, version_line: &str) -> Option<&'static str> {
    let required = minimum(cli)?;
    let current = parse(version_line)?;
    (current < parse(required)?).then_some(required)
}

/// 已裝版本比測過的舊（claude、codex、grok 的一次性提示與「格式認不得」的按鈕用）。
pub(crate) fn older_than_tested(cli: &str, version_line: &str) -> bool {
    match (parse(version_line), tested(cli).and_then(parse)) {
        (Some(current), Some(tested)) => current < tested,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const CLIS: [&str; 4] = ["claude", "codex", "agy", "grok"];

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/stream/fixtures")
    }

    #[test]
    fn parses_every_version_line_shape() {
        for (line, expected) in [
            ("2.1.311 (Claude Code)", "2.1.311"),
            ("codex-cli 0.164.0", "0.164.0"),
            ("grok 1.0.70 (f50904eaee6c) [stable]", "1.0.70"),
            ("1.4.9", "1.4.9"),
            ("agy version 1.4.8 (darwin)", "1.4.8"),
        ] {
            assert_eq!(
                parse(line),
                Some(Version::parse(expected).unwrap()),
                "{line}"
            );
        }
        assert_eq!(parse(""), None);
        assert_eq!(parse("nightly"), None);
        assert_eq!(parse("1.2"), None);
    }

    #[test]
    fn minimum_gate_lets_unparsable_versions_through() {
        assert_eq!(below_minimum("agy", "1.2.0"), Some("1.2.1"));
        assert_eq!(below_minimum("agy", "1.1.17"), Some("1.2.1"));
        assert_eq!(below_minimum("agy", "1.2.1"), None);
        assert_eq!(below_minimum("agy", "2.0.0"), None);
        assert_eq!(below_minimum("agy", ""), None);
        assert_eq!(below_minimum("claude", "0.0.1"), None);
    }

    /// 常數＝夾具 `.version` 解析出的版號，逐字相同；且不低於最低門檻。
    #[test]
    fn constants_match_recorded_version_files() {
        for cli in CLIS {
            let tested = tested(cli).unwrap();
            let line =
                std::fs::read_to_string(fixtures().join(cli).join(format!("{tested}.version")))
                    .unwrap_or_else(|_| panic!("{cli}：{tested}.version 不見了，先錄夾具再改常數"));
            let recorded = parse(line.lines().next().unwrap_or_default())
                .unwrap_or_else(|| panic!("{cli}：.version 解析不出版號：{line}"));
            assert_eq!(recorded.to_string(), tested, "{cli}");
            if let Some(minimum) = minimum(cli) {
                assert!(
                    recorded >= parse(minimum).unwrap(),
                    "{cli}：測過的版本低於門檻"
                );
            }
        }
    }

    /// 測過的版本要有每種呼叫情境的錄音，且是該 CLI 最新的一版錄音（參數逐字比對只看最新版）。
    #[test]
    fn constants_have_every_scenario_recorded() {
        for cli in CLIS {
            let tested = tested(cli).unwrap();
            let mut scenarios = vec!["oneshot", "think", "lane-open", "lane-resume", "badmodel"];
            if cli != "claude" {
                // claude 不走 CLI 生圖
                scenarios.push("image");
            }
            if matches!(cli, "agy" | "grok") {
                scenarios.push("models");
            }
            for scenario in scenarios {
                let path = fixtures()
                    .join(cli)
                    .join(format!("{tested}-{scenario}.args.json"));
                assert!(path.exists(), "{cli}：缺 {tested}-{scenario} 的錄音");
            }
            let newest = std::fs::read_dir(fixtures().join(cli))
                .unwrap()
                .filter_map(|entry| {
                    let name = entry.ok()?.file_name().to_string_lossy().into_owned();
                    parse(name.strip_suffix(".version")?)
                })
                .max()
                .unwrap();
            assert_eq!(newest.to_string(), tested, "{cli}：有更新的錄音卻沒改常數");
        }
    }

    #[test]
    fn older_than_tested_compares_parsed_versions() {
        assert!(older_than_tested("claude", "2.1.300 (Claude Code)"));
        assert!(!older_than_tested("claude", "2.1.311 (Claude Code)"));
        assert!(!older_than_tested("claude", "2.2.0 (Claude Code)"));
        assert!(!older_than_tested("codex", "garbage"));
        assert!(older_than_tested("grok", "grok 1.0.69 (x) [stable]"));
    }
}
