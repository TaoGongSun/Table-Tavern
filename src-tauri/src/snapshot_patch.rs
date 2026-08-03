/// applied＝上次已傳達給模型的素材全文；current＝本輪最新素材全文。
/// 相同回 None；不同回一則補丁文字（附在本輪 prompt 裡告知模型哪些段落變了）。
pub(crate) fn render_patch(applied: &str, current: &str) -> Option<String> {
    if applied == current {
        return None;
    }

    let applied_chunks = split_chunks(applied);
    let current_chunks = split_chunks(current);
    let applied_by_key: std::collections::BTreeMap<_, _> = applied_chunks
        .iter()
        .map(|chunk| (chunk.key(), chunk))
        .collect();
    let current_by_key: std::collections::BTreeMap<_, _> = current_chunks
        .iter()
        .map(|chunk| (chunk.key(), chunk))
        .collect();

    let mut replacements = Vec::new();
    for chunk in &current_chunks {
        if applied_by_key
            .get(&chunk.key())
            .is_none_or(|previous| previous.content != chunk.content)
        {
            if chunk.title.is_empty() {
                replacements.push(format!("（開頭指示更新）\n{}", chunk.content));
            } else {
                replacements.push(chunk.content.clone());
            }
        }
    }

    let removed: Vec<_> = applied_chunks
        .iter()
        .filter(|chunk| !chunk.title.is_empty() && !current_by_key.contains_key(&chunk.key()))
        .map(|chunk| format!("〈{}〉", chunk.title))
        .collect();

    let mut patch =
        "## 設定更新\n以下設定剛剛變更，請以此處版本為準（同標題段落整段取代先前內容）：\n"
            .to_owned();
    if !replacements.is_empty() {
        patch.push('\n');
        patch.push_str(&replacements.join("\n\n"));
        patch.push('\n');
    }
    if !removed.is_empty() {
        patch.push('\n');
        patch.push_str("（已移除段落：");
        patch.push_str(&removed.join("、"));
        patch.push_str("）\n");
    }
    Some(patch)
}

#[derive(Debug)]
struct Chunk {
    title: String,
    occurrence: usize,
    content: String,
}

impl Chunk {
    fn key(&self) -> (String, usize) {
        (self.title.clone(), self.occurrence)
    }
}

/// 保留原始換行，讓補丁內的段落能逐字取代舊版素材。
fn split_chunks(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_title = String::new();
    let mut current_content = String::new();
    let mut seen_titles = std::collections::BTreeMap::<String, usize>::new();
    let mut occurrence = 0;

    for line in text.split_inclusive('\n') {
        let title = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or(line.strip_suffix('\n').unwrap_or(line));
        if title.starts_with("## ") || title.starts_with("### ") {
            chunks.push(Chunk {
                title: current_title,
                occurrence,
                content: current_content,
            });
            current_title = title.to_owned();
            let next_occurrence = seen_titles.entry(current_title.clone()).or_default();
            occurrence = *next_occurrence;
            *next_occurrence += 1;
            current_content = line.to_owned();
        } else {
            current_content.push_str(line);
        }
    }
    chunks.push(Chunk {
        title: current_title,
        occurrence,
        content: current_content,
    });
    chunks
}

#[cfg(test)]
mod tests {
    use super::render_patch;

    #[test]
    fn identical_material_has_no_patch() {
        assert_eq!(render_patch("## 卡片\n內容\n", "## 卡片\n內容\n"), None);
    }

    #[test]
    fn changed_section_only_renders_the_new_section() {
        let patch = render_patch("## A\n舊\n### B\n不變\n", "## A\n新\n### B\n不變\n").unwrap();
        assert!(patch.contains("## 設定更新"));
        assert!(patch.contains("## A\n新\n"));
        assert!(!patch.contains("### B"));
    }

    #[test]
    fn added_section_is_rendered() {
        let patch = render_patch("## A\n內容\n", "## A\n內容\n### B\n新增\n").unwrap();
        assert!(patch.contains("### B\n新增\n"));
    }

    #[test]
    fn removed_section_is_listed() {
        let patch = render_patch("## A\n內容\n### B\n移除\n", "## A\n內容\n").unwrap();
        assert!(patch.contains("（已移除段落：〈### B〉）"));
    }

    #[test]
    fn duplicate_titles_match_by_occurrence() {
        let patch = render_patch(
            "### 卡\n第一張\n### 卡\n第二張\n",
            "### 卡\n第一張\n### 卡\n已改\n",
        )
        .unwrap();
        assert!(patch.contains("### 卡\n已改\n"));
        assert_eq!(patch.matches("### 卡").count(), 1);
    }

    #[test]
    fn plain_text_uses_the_leading_chunk() {
        let patch = render_patch("舊指示", "新指示").unwrap();
        assert!(patch.contains("（開頭指示更新）\n新指示"));
    }
}
