//! 抹寫／截尾失敗的原因：哪一步（stage）壞、原本的錯誤字串（detail）。
//! 落帳本前先遮路徑再截斷——帳本會被貼進問題回報，不必帶出家目錄。

use std::path::Path;

/// detail 落帳的上限（Unicode 字元數）。
const DETAIL_MAX_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewriteFailure {
    /// load／find-segment／erase-segment／prefix-assistant／truncate／write
    pub stage: &'static str,
    pub detail: String,
}

/// 把 session_file 某一步的錯誤字串標上 stage。
pub(super) fn at<T>(stage: &'static str, result: Result<T, String>) -> Result<T, RewriteFailure> {
    result.map_err(|detail| RewriteFailure { stage, detail })
}

/// 這次呼叫已知的路徑 → 換成什麼。長的先換，免得父目錄先吃掉檔案路徑的前半段。
pub(super) fn known_paths(
    session: &Path,
    claude_home: &Path,
    working_dir: &Path,
) -> Vec<(String, String)> {
    let file_name = session
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<file>".to_owned());
    let mut pairs = vec![(session.display().to_string(), file_name)];
    if let Some(parent) = session.parent() {
        pairs.push((parent.display().to_string(), "<dir>".to_owned()));
    }
    pairs.push((claude_home.display().to_string(), "<dir>".to_owned()));
    pairs.push((working_dir.display().to_string(), "<dir>".to_owned()));
    pairs
}

/// 先遮路徑（已知字串整段比對，含空白、反斜線都照樣命中），再按字元截斷。
pub(super) fn mask_detail(detail: &str, paths: &[(String, String)]) -> String {
    let mut pairs: Vec<&(String, String)> = paths
        .iter()
        .filter(|(needle, _)| !needle.is_empty())
        .collect();
    pairs.sort_by_key(|(needle, _)| std::cmp::Reverse(needle.len()));
    let mut masked = detail.to_owned();
    for (needle, replacement) in pairs {
        masked = masked.replace(needle.as_str(), replacement);
    }
    if masked.chars().count() <= DETAIL_MAX_CHARS {
        return masked;
    }
    let mut cut: String = masked.chars().take(DETAIL_MAX_CHARS).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_labels_injected_failures_with_their_stage() {
        // 正常流程碰不到的兩步（truncate、write）用注入驗 stage 映射
        for stage in ["truncate", "write"] {
            let failure = at::<()>(stage, Err("boom".to_owned())).unwrap_err();
            assert_eq!(
                failure,
                RewriteFailure {
                    stage,
                    detail: "boom".to_owned()
                }
            );
        }
        assert_eq!(at("load", Ok::<u8, String>(7)), Ok(7));
    }

    #[test]
    fn masks_posix_paths_with_spaces_longest_first() {
        let session = Path::new("/Users/a b/.claude/projects/-ws/abc.jsonl");
        let pairs = known_paths(
            session,
            Path::new("/Users/a b/.claude"),
            Path::new("/tmp/w s"),
        );
        let detail = format!(
            "無法讀取 session 檔 {}：No such file；工作目錄 /tmp/w s",
            session.display()
        );
        assert_eq!(
            mask_detail(&detail, &pairs),
            "無法讀取 session 檔 abc.jsonl：No such file；工作目錄 <dir>"
        );
    }

    #[test]
    fn masks_windows_paths_given_as_known_strings() {
        let pairs = vec![
            (
                r"C:\Users\a b\.claude\projects\x\s.jsonl".to_owned(),
                "s.jsonl".to_owned(),
            ),
            (r"C:\Users\a b\.claude".to_owned(), "<dir>".to_owned()),
        ];
        assert_eq!(
            mask_detail(
                r"無法以暫存 session 檔替換 C:\Users\a b\.claude\projects\x\s.jsonl：denied",
                &pairs
            ),
            "無法以暫存 session 檔替換 s.jsonl：denied"
        );
    }

    #[test]
    fn truncates_after_masking_on_char_boundaries() {
        let long_path = format!("/very/{}/s.jsonl", "長".repeat(400));
        let pairs = vec![(long_path.clone(), "s.jsonl".to_owned())];
        // 遮完很短：不該被截
        assert_eq!(
            mask_detail(&format!("壞在 {long_path}"), &pairs),
            "壞在 s.jsonl"
        );
        // 遮完仍超長：按字元截到上限，多位元組字元不被切半
        let cut = mask_detail(&"字".repeat(500), &pairs);
        assert_eq!(cut.chars().count(), DETAIL_MAX_CHARS + 1);
        assert!(cut.ends_with('…'));
    }
}
