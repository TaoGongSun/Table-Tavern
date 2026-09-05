use super::detect::{find_binary, hidden_output};
use super::types::ModelOption;
use std::path::PathBuf;

/// codex 快取解析：跳過內部項與 hidden，依 priority 排序，label 用 display_name
pub fn parse_codex_catalog(json: &str) -> Vec<ModelOption> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(models) = value.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    let mut ranked: Vec<(i64, ModelOption)> = models
        .iter()
        .filter_map(|item| {
            let slug = item.get("slug").and_then(|s| s.as_str())?.trim();
            if slug.is_empty() || slug == "codex-auto-review" {
                return None;
            }
            if matches!(
                item.get("visibility").and_then(|v| v.as_str()),
                Some("hidden") | Some("none")
            ) {
                return None;
            }
            let label = item
                .get("display_name")
                .and_then(|s| s.as_str())
                .unwrap_or(slug);
            let priority = item.get("priority").and_then(|p| p.as_i64()).unwrap_or(0);
            Some((
                priority,
                ModelOption {
                    id: slug.to_owned(),
                    label: label.to_owned(),
                },
            ))
        })
        .collect();
    ranked.sort_by_key(|(priority, _)| *priority);
    ranked.into_iter().map(|(_, option)| option).collect()
}

/// 掃 Claude CLI 執行檔內建的模型註冊表（`{id:"…",family:"…",display_name:"…"`）。
/// 官方每次改版都換一份表，因此新模型上線後 `claude update` 即自動出現，不必改本程式。
/// 逐塊掃描：執行檔數百 MB，不整支讀進記憶體；跨塊邊界靠重疊窗＋去重涵蓋。
pub fn parse_claude_registry(source: impl std::io::Read) -> Vec<ModelOption> {
    let Ok(pattern) = regex::bytes::Regex::new(
        r#"\{id:"(claude-[^"]{1,64})",family:"[^"]{1,32}",display_name:"([^"]{1,64})""#,
    ) else {
        return Vec::new();
    };
    const CHUNK: usize = 4 << 20;
    const OVERLAP: usize = 512; // 單筆前綴最長約 200 bytes
    let mut reader = std::io::BufReader::new(source);
    let mut buffer = vec![0u8; CHUNK];
    let mut window: Vec<u8> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut options = Vec::new();
    while let Ok(read) = std::io::Read::read(&mut reader, &mut buffer) {
        if read == 0 {
            break;
        }
        window.extend_from_slice(&buffer[..read]);
        for capture in pattern.captures_iter(&window) {
            let id = String::from_utf8_lossy(&capture[1]).into_owned();
            // 3.x 世代已退場，不佔用下拉版面
            if id.starts_with("claude-3-") || !seen.insert(id.clone()) {
                continue;
            }
            options.push(ModelOption {
                id,
                label: String::from_utf8_lossy(&capture[2]).into_owned(),
            });
        }
        let keep = window.len().min(OVERLAP);
        window.drain(..window.len() - keep);
    }
    options.reverse(); // 註冊表按世代遞增排列，UI 要最新的在最上面
    options
}

/// agy models 每列是 `id\t顯示名`；只有第一欄能當 id 傳回 CLI（整列含顯示名會被拒）。
/// 進度訊息走 stderr，這裡讀的 stdout 只有模型列。
pub fn parse_agy_catalog(output: &str) -> Vec<ModelOption> {
    output
        .lines()
        .filter_map(|line| {
            let (id, label) = line.split_once('\t').unwrap_or((line, line));
            let id = id.trim();
            (!id.is_empty()).then(|| ModelOption {
                id: id.to_owned(),
                label: label.trim().to_owned(),
            })
        })
        .collect()
}

/// grok models 只認縮排列；保留原列為 label，去掉預設標記後作為可傳入的 id。
pub fn parse_grok_catalog(output: &str) -> Vec<ModelOption> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let label = trimmed.strip_prefix('*')?.trim();
            let id = label.strip_suffix(" (default)").unwrap_or(label).trim();
            (!id.is_empty()).then(|| ModelOption {
                id: id.to_owned(),
                label: label.to_owned(),
            })
        })
        .collect()
}

/// 組下拉目錄：claude 固定前置官方別名（CLI 穩定介面）再接快取；快取讀不到就只剩別名。
/// codex 純靠快取；agy／grok 即時讀 CLI 輸出，讀不到回空（UI 都保留「自訂」手填逃生口）。
/// `envs`：grok 需要 `grok_envs()` 那組隔離環境，否則讀到的是使用者終端機的登入態，
/// 與旁白實跑用的 app profile 不同步（設定頁會顯示已登入、真發言卻要求登入）。
pub async fn cli_model_catalog(cli: &str, envs: &[(String, String)]) -> Vec<ModelOption> {
    let read = |rel: &[&str]| -> Option<String> {
        let mut path = PathBuf::from(std::env::var_os("HOME")?);
        for part in rel {
            path.push(part);
        }
        std::fs::read_to_string(path).ok()
    };
    match cli {
        "claude" => {
            let mut options: Vec<ModelOption> = ["fable", "opus", "sonnet", "haiku"]
                .iter()
                .map(|alias| ModelOption {
                    id: (*alias).to_owned(),
                    label: format!("{alias}（官方別名）"),
                })
                .collect();
            // 掃數百 MB 執行檔是 CPU 密集的同步工作，丟去 blocking 池免得佔住 async worker
            if let Some(path) = find_binary("claude") {
                options.extend(
                    tokio::task::spawn_blocking(move || {
                        std::fs::File::open(path)
                            .ok()
                            .map(parse_claude_registry)
                            .unwrap_or_default()
                    })
                    .await
                    .unwrap_or_default(),
                );
            }
            options
        }
        "codex" => read(&[".codex", "models_cache.json"])
            .map(|json| parse_codex_catalog(&json))
            .unwrap_or_default(),
        "agy" => match find_binary("agy") {
            Some(program) => hidden_output(program, "models", envs)
                .await
                .map(|output| parse_agy_catalog(&String::from_utf8_lossy(&output.stdout)))
                .unwrap_or_default(),
            None => Vec::new(),
        },
        "grok" => match find_binary("grok") {
            Some(program) => hidden_output(program, "models", envs)
                .await
                .map(|output| parse_grok_catalog(&String::from_utf8_lossy(&output.stdout)))
                .unwrap_or_default(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_catalog_skips_internal_and_hidden_and_sorts_by_priority() {
        let json = r#"{"models":[
            {"slug":"codex-auto-review","display_name":"內部"},
            {"slug":"gpt-5.4","display_name":"GPT-5.4","priority":2},
            {"slug":"gpt-5.6-sol","display_name":"GPT-5.6-Sol","priority":1},
            {"slug":"secret-model","visibility":"hidden"}
        ]}"#;
        let ids: Vec<_> = parse_codex_catalog(json)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["gpt-5.6-sol", "gpt-5.4"]);
        assert!(parse_codex_catalog("not json").is_empty());
    }

    /// 一筆註冊表記錄（與 CLI 執行檔內的形態一致，後面還接著其他欄位）
    fn registry_entry(id: &str, label: &str) -> String {
        format!(r#"{{id:"{id}",family:"opus",display_name:"{label}",knowledge_cutoff:"May 2026"}},"#)
    }

    #[test]
    fn claude_registry_lists_newest_first_and_drops_legacy() {
        let binary = format!(
            "somejsnoise{}{}{}{}",
            registry_entry("claude-3-5-haiku", "Haiku 3.5"),
            registry_entry("claude-opus-4-6", "Opus 4.6"),
            registry_entry("claude-opus-5", "Opus 5"),
            registry_entry("claude-opus-4-6", "Opus 4.6"), // 表內重複定義只留一筆
        );
        let options = parse_claude_registry(binary.as_bytes());
        assert_eq!(
            options,
            vec![
                ModelOption {
                    id: "claude-opus-5".to_owned(),
                    label: "Opus 5".to_owned(),
                },
                ModelOption {
                    id: "claude-opus-4-6".to_owned(),
                    label: "Opus 4.6".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn claude_registry_catches_entry_across_chunk_boundary() {
        // 讓記錄剛好橫跨 4 MiB 讀取塊的邊界，驗證重疊窗有接住
        let entry = registry_entry("claude-opus-5", "Opus 5");
        let pad = (4 << 20) - entry.len() / 2;
        let binary = format!("{}{entry}", "x".repeat(pad));
        let options = parse_claude_registry(binary.as_bytes());
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].id, "claude-opus-5");
    }

    #[test]
    fn agy_catalog_takes_id_column_only() {
        // 實測輸出：每列 `id\t顯示名`，整列當 id 會讓 CLI 拒收
        let output = "gemini-3.6-flash-high\tGemini 3.6 Flash (High)\nclaude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n\n";
        assert_eq!(
            parse_agy_catalog(output),
            vec![
                ModelOption {
                    id: "gemini-3.6-flash-high".to_owned(),
                    label: "Gemini 3.6 Flash (High)".to_owned()
                },
                ModelOption {
                    id: "claude-sonnet-4-6".to_owned(),
                    label: "Claude Sonnet 4.6 (Thinking)".to_owned()
                },
            ]
        );
    }

    #[test]
    fn grok_catalog_ignores_noise_and_strips_default_marker() {
        let output = "You are not authenticated.\nDefault model: grok-4.5\nAvailable models:\n  * grok-4.5 (default)\n  * grok-4.1-fast\n";
        assert_eq!(
            parse_grok_catalog(output),
            vec![
                ModelOption {
                    id: "grok-4.5".to_owned(),
                    label: "grok-4.5 (default)".to_owned()
                },
                ModelOption {
                    id: "grok-4.1-fast".to_owned(),
                    label: "grok-4.1-fast".to_owned()
                },
            ]
        );
    }

}
