//! 簡繁字形折疊（比對專用）：模型回報的人名、狀態路徑可能換了字形（贺铮／賀錚），app 用字面比對就對不上。
//! 兩邊各自折成簡體字元再比；只比對、不改任何顯示或落檔文字。字表見 `opencc/README.md`。
//!
//! 折疊不保證無誤撞：沈郁／沈鬱、於／于、鍾／鐘、範／范、餘／余、乾／幹／干、發／髮會折成同一字。
//! 呼叫端一律先用原文精確比對，折疊只當兜底，並自己處理多個候選同時命中的歧義。

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

const TS_CHARACTERS: &str = include_str!("opencc/TSCharacters.txt");

fn table() -> &'static HashMap<char, char> {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for line in TS_CHARACTERS.lines() {
            let Some((from, to)) = line.split_once('\t') else {
                continue;
            };
            let mut from_chars = from.chars();
            let (Some(from), None) = (from_chars.next(), from_chars.next()) else {
                continue;
            };
            // 一字多候選（「於\t于 於」）取第一個，與 OpenCC 預設轉換一致
            let Some(to) = to.split(' ').next().and_then(|first| {
                let mut chars = first.chars();
                chars.next().filter(|_| chars.next().is_none())
            }) else {
                continue;
            };
            map.insert(from, to);
        }
        map
    })
}

/// 把字串折成簡體字形；沒有任何字需要折時不配置新字串。
pub(crate) fn fold_glyphs(text: &str) -> Cow<'_, str> {
    let table = table();
    if !text.chars().any(|ch| table.contains_key(&ch)) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|ch| table.get(&ch).copied().unwrap_or(ch))
            .collect(),
    )
}

/// 兩個字串折疊後相等（只差字形）。
pub(crate) fn glyph_equal(a: &str, b: &str) -> bool {
    a == b || fold_glyphs(a) == fold_glyphs(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_traditional_to_simplified_and_leaves_others() {
        assert_eq!(fold_glyphs("賀錚、鐵三"), "贺铮、铁三");
        assert_eq!(fold_glyphs("驻守天数"), "驻守天数");
        assert!(matches!(fold_glyphs("Toren 42"), Cow::Borrowed(_)));
        assert!(glyph_equal("駐守天數", "驻守天数"));
        assert!(!glyph_equal("周掌柜", "周掌门"));
    }

    /// 已知誤撞：折疊後相同，呼叫端必須先做原文精確比對
    #[test]
    fn known_collisions_fold_together() {
        assert!(glyph_equal("沈郁", "沈鬱"));
        assert!(glyph_equal("於", "于"));
        assert!(glyph_equal("鍾", "鐘"));
        assert!(glyph_equal("餘", "余"));
        assert!(glyph_equal("發", "髮"));
    }
}
