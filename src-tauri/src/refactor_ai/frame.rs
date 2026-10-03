//! 外框候選條目：只規定輸出容器排法、容器裡只有佔位說明的介面條目（例如「正文放 <maintext>、狀態欄放
//! <Status_block>」，容器內只有 `${有狀態欄的時候放在這個位置}`）。這種條目本身沒有欄位與初始值，展開時
//! 模型會把別條定義的欄位再抄一份，合併就撞衝突、guide 也重複。
//!
//! 這裡只做程式判定「候選」，規則刻意嚴格（寧可漏判、走一般展開，也不能把帶固定內容的條目當外框消耗掉）：
//! - 條目裡至少一個 `<Tag>…</Tag>` 容器；
//! - 每個容器裡去掉佔位（`${…}`、`{{…}}`）後只剩空白——任何固定文字（例如 `糧草＝300石`）都不算外框；
//! - 容器外沒有佔位、也沒有縮排的「鍵: 值」欄位定義（容器外的規則條列 `- …`、頂層標題 `rule:` 不算）。
//! 真正當外框還要同一次重構裡另有定義條目的骨架含相同容器標籤（前端在定義條目展開完後比對，見
//! refactor-frame.ts）；對不上就照一般條目展開。
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// 外框候選：條目 uid 與它的容器標籤（依第一次出現的順序，不重複）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorFrameCandidate {
    pub uid: String,
    pub tags: Vec<String>,
}

fn open_tag() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"<([A-Za-z_][\w.-]*)>").expect("open tag regex"))
}

fn placeholder() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$\{[^}]*\}|\{\{[^{}]*\}\}").expect("placeholder regex"))
}

fn outside_field_line() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^\s+[^\s\-#:：][^:：]*[:：]\s*\S").expect("outside field regex")
    })
}

/// 條目文字是外框候選就回容器標籤（出現順序、不重複），否則 None。
pub fn frame_candidate_tags(text: &str) -> Option<Vec<String>> {
    let mut tags: Vec<String> = Vec::new();
    let mut outside = String::new();
    let mut cursor = 0;
    while let Some(found) = open_tag().captures_at(text, cursor) {
        let whole = found.get(0).expect("match");
        let tag = &found[1];
        let close = format!("</{tag}>");
        let Some(end) = text[whole.end()..].find(&close) else {
            outside.push_str(&text[cursor..whole.end()]);
            cursor = whole.end();
            continue;
        };
        outside.push_str(&text[cursor..whole.start()]);
        let inner = &text[whole.end()..whole.end() + end];
        if !placeholder().replace_all(inner, "").trim().is_empty() {
            return None;
        }
        if !tags.iter().any(|known| known == tag) {
            tags.push(tag.to_owned());
        }
        cursor = whole.end() + end + close.len();
    }
    outside.push_str(&text[cursor..]);
    if placeholder().is_match(&outside)
        || outside
            .lines()
            .any(|line| outside_field_line().is_match(line))
    {
        return None;
    }
    (!tags.is_empty()).then_some(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: &str = "# 防止掉格式\nrule:\n  - 对话内容放在<maintext></maintext>标签中.\n  - 状态栏放在 <Status_block></Status_block> 标签,位于 </maintext> 后面\nformate: |-\n<maintext>\n${LLM输出的角色对话内容}\n</maintext>\n <Status_block>\n${有状态栏的时候放在这个位置,没有的话移除Status_block标签 }\n</Status_block>";

    const DEFINING: &str = "rule: 生成的yaml数据位于 <Status_block></Status_block> 中间\nformate |-\n<Status_block>\n状态栏:\n  日期和时间: \"${按照格式输出,示例:⏰ 大雍永和三年十月初七 酉时 }\"\n  人物列表:\n    - 人物:\n        名字: \"${👤 姓名 }\"\n</Status_block>";

    #[test]
    fn placeholder_only_containers_are_a_frame_candidate() {
        assert_eq!(
            frame_candidate_tags(FRAME),
            Some(vec!["maintext".to_owned(), "Status_block".to_owned()])
        );
    }

    #[test]
    fn containers_with_fields_or_fixed_text_are_not_a_frame() {
        assert_eq!(frame_candidate_tags(DEFINING), None);
        assert_eq!(frame_candidate_tags("<A>\n<b>x</b>\n</A>"), None);
        assert_eq!(frame_candidate_tags("<A>\n- 一項\n</A>"), None);
        // 同容器但含固定文字：不是外框，照一般條目展開（固定內容不會被當外框消耗掉）
        assert_eq!(
            frame_candidate_tags("<Status_block>糧草＝300石</Status_block>"),
            None
        );
        assert_eq!(
            frame_candidate_tags("<maintext>${正文}</maintext>\n<Status_block>\n${放這裡}\n糧草＝300石\n</Status_block>"),
            None
        );
        assert_eq!(frame_candidate_tags("<A>\n狀態：放這裡\n</A>"), None);
        // 佔位符裡的冒號、多個佔位符都允許
        assert_eq!(
            frame_candidate_tags("<A>${示例: 正文} {{本回合.正文}}</A>"),
            Some(vec!["A".to_owned()])
        );
    }

    #[test]
    fn field_definitions_outside_containers_are_not_a_frame() {
        // 容器外有模板佔位或縮排的「鍵: 值」欄位定義：這條自己定義了欄位
        assert_eq!(
            frame_candidate_tags(
                "状态栏:\n  地点: \"${📍 地点}\"\n<Status_block>${放這裡}</Status_block>"
            ),
            None
        );
        assert_eq!(
            frame_candidate_tags("状态栏:\n  粮草: 三百石\n<Status_block></Status_block>"),
            None
        );
        // 容器外只有規則條列與頂層標題：仍是外框候選
        assert_eq!(
            frame_candidate_tags("rule:\n  - 正文放在<maintext></maintext>中\nformate: |-\n<maintext>\n${正文}\n</maintext>"),
            Some(vec!["maintext".to_owned()])
        );
    }

    #[test]
    fn entries_without_containers_are_not_a_frame() {
        assert_eq!(frame_candidate_tags("只有規則文字，沒有容器"), None);
        assert_eq!(frame_candidate_tags("<A> 沒有結尾"), None);
    }
}
