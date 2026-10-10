//! 角色回覆開頭模型自己加的「名字：」（char-line-prefix）。歷史裡每則台詞都帶 `speaker_prefix`，
//! 模型常照抄。只剝本輪角色的正典字面：session 抹寫吃原文，原文本來就以這個前綴開頭，
//! `prefix + 剝餘 == 原文 == session`，正典重建與續聊內容逐字一致。

/// 開頭正好是 `prefix` 且剝餘不是全空白才剝；只剝一層、剝餘原樣（不吃空白）。
/// 串流版的對照基準；落檔的台詞改由 `finish_character_reply` 收尾（char-line-status-strip）。
#[cfg(test)]
fn strip_own_prefix<'a>(text: &'a str, prefix: &str) -> &'a str {
    match text.strip_prefix(prefix) {
        Some(rest) if !prefix.is_empty() && rest.chars().any(|c| !c.is_whitespace()) => rest,
        _ => text,
    }
}

enum Stage {
    /// 累積的字仍可能是前綴的開頭
    Matching,
    /// 前綴已湊齊，後面目前全是空白：等第一個非空白字才確定要剝
    Spaces,
    /// 已判定，之後原樣直通
    Through,
}

/// 串流版的 `strip_own_prefix`：單次 attempt 內，`push` 與 `finish` 的輸出接起來
/// == `strip_own_prefix(全文, prefix)`。每次 attempt 要新建一個。
pub struct OwnPrefixStream {
    prefix: String,
    held: String,
    stage: Stage,
}

impl OwnPrefixStream {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_owned(),
            held: String::new(),
            stage: match prefix.is_empty() {
                true => Stage::Through,
                false => Stage::Matching,
            },
        }
    }

    /// 收一段增量，回傳現在可以放行的字（可能是空字串）。
    pub fn push(&mut self, delta: &str) -> String {
        if matches!(self.stage, Stage::Through) {
            return delta.to_owned();
        }
        self.held.push_str(delta);
        if matches!(self.stage, Stage::Matching) {
            if self.held.len() < self.prefix.len() {
                if self.prefix.starts_with(self.held.as_str()) {
                    return String::new();
                }
                return self.release();
            }
            if !self.held.starts_with(self.prefix.as_str()) {
                return self.release();
            }
            self.stage = Stage::Spaces;
        }
        let rest = &self.held[self.prefix.len()..];
        if rest.chars().all(char::is_whitespace) {
            return String::new();
        }
        let rest = rest.to_owned();
        self.held.clear();
        self.stage = Stage::Through;
        rest
    }

    /// 完成或中止：還沒判定的扣留原樣放行（沒湊齊前綴，或湊齊後全是空白＝不剝）。
    pub fn finish(&mut self) -> String {
        self.release()
    }

    fn release(&mut self) -> String {
        self.stage = Stage::Through;
        std::mem::take(&mut self.held)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZH: &str = "狐狸：";
    const EN: &str = "Ralph: ";

    #[test]
    fn strips_only_the_canonical_prefix_once_and_round_trips() {
        for (text, prefix) in [
            ("狐狸：狐狸擦杯子。", ZH),
            ("狐狸：\n正文", ZH),
            ("狐狸：  縮排正文", ZH),
            ("Ralph: hello", EN),
            ("林教授｜經濟學：林教授｜經濟學轉過身。", "林教授｜經濟學："),
        ] {
            let rest = strip_own_prefix(text, prefix);
            assert_ne!(rest, text, "{text}");
            assert_eq!(format!("{prefix}{rest}"), text);
        }
        assert_eq!(strip_own_prefix("狐狸：\n正文", ZH), "\n正文");
        assert_eq!(strip_own_prefix("狐狸：狐狸：正文", ZH), "狐狸：正文");
    }

    #[test]
    fn leaves_everything_else_untouched() {
        for (text, prefix) in [
            ("狐狸瞇起眼。", ZH),
            ("狐狸人：哈。", ZH),
            ("狐狸:正文", ZH),
            (" 狐狸：正文", ZH),
            ("**狐狸**：正文", ZH),
            ("騎士：正文", ZH),
            ("正文裡的狐狸：", ZH),
            ("狐狸：", ZH),
            ("狐狸：\n\n  ", ZH),
            ("", ZH),
            ("Ralph:hello", EN),
            ("ralph: hello", EN),
            ("狐狸：正文", ""),
        ] {
            assert_eq!(strip_own_prefix(text, prefix), text, "{text:?}");
        }
    }

    fn streamed(chunks: &[&str], prefix: &str) -> String {
        let mut filter = OwnPrefixStream::new(prefix);
        let mut out: String = chunks.iter().map(|chunk| filter.push(chunk)).collect();
        out.push_str(&filter.finish());
        out
    }

    /// 在每個字元邊界切成兩段、以及逐字切，串流接起來都等於批次剝除；
    /// 中止＝在任一邊界 finish，等於剝那段半截。
    #[test]
    fn stream_matches_batch_at_every_split_and_abort_point() {
        let spaces = format!("狐狸：{}正文", " \n".repeat(500));
        let cases = [
            "狐狸：狐狸擦杯子。",
            "狐狸：\n正文",
            "狐狸：\n\n",
            "狐狸：\n\n  縮排正文",
            "狐狸：狐狸：正文",
            "狐狸人：哈",
            "狐狸",
            "狐狸：",
            "狐",
            "騎士：正文",
            "正文",
            spaces.as_str(),
        ];
        for text in cases {
            let bounds: Vec<usize> = text
                .char_indices()
                .map(|(index, _)| index)
                .chain([text.len()])
                .collect();
            for &cut in &bounds {
                let (head, tail) = text.split_at(cut);
                assert_eq!(
                    streamed(&[head, tail], ZH),
                    strip_own_prefix(text, ZH),
                    "{text:?}@{cut}"
                );
                assert_eq!(
                    streamed(&[head], ZH),
                    strip_own_prefix(head, ZH),
                    "abort {text:?}@{cut}"
                );
            }
            let chars: Vec<String> = text.chars().map(String::from).collect();
            let chars: Vec<&str> = chars.iter().map(String::as_str).collect();
            assert_eq!(
                streamed(&chars, ZH),
                strip_own_prefix(text, ZH),
                "{text:?} per char"
            );
        }
        for text in ["Ralph: hello", "Ralph:hello", "Ralph:  ", "Ralph"] {
            let chars: Vec<String> = text.chars().map(String::from).collect();
            let chars: Vec<&str> = chars.iter().map(String::as_str).collect();
            assert_eq!(streamed(&chars, EN), strip_own_prefix(text, EN), "{text:?}");
        }
    }

    #[test]
    fn stream_holds_whitespace_until_body_then_releases_it_with_body() {
        let mut filter = OwnPrefixStream::new(ZH);
        assert_eq!(filter.push("狐狸"), "");
        assert_eq!(filter.push("：\n"), "");
        assert_eq!(filter.push("\n"), "");
        assert_eq!(filter.push("正"), "\n\n正");
        assert_eq!(filter.push("文"), "文");
        assert_eq!(filter.finish(), "");

        let mut filter = OwnPrefixStream::new(ZH);
        assert_eq!(filter.push("狐狸：\n\n"), "");
        assert_eq!(filter.finish(), "狐狸：\n\n");

        let mut filter = OwnPrefixStream::new(ZH);
        assert_eq!(filter.push("狐"), "");
        assert_eq!(filter.push("貍"), "狐貍");
    }
}
