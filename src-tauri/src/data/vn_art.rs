//! VN 卡的美術欄位（plans/vn-mode-pkg9b.md §2.1）：匯入時驗過、正規化好的型別，存在 `vn.json` 的 `card`，
//! 匯入後不可編輯；只進生圖提示詞、表情／變體清單與背景地點，不進文字模型的設定段。
use serde::{Deserialize, Serialize};

/// 卡層級畫風（`art`）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardArt {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub style: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub background_rules: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub negative: String,
    /// 封面當生圖畫風參考圖
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cover_as_style_ref: bool,
}

/// 卡內地點（`locations[]`；卡上的 `slug` 存成 `key`）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardLocation {
    pub key: String,
    pub name: String,
    /// 這個地點的時段鍵；空＝固定四時段
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub times: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scale: String,
}

/// 卡宣告的一個表情或變體
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtEntry {
    pub key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prompt: String,
}

impl ArtEntry {
    /// 生圖用的描述：沒寫 prompt 退回顯示名
    pub fn description(&self) -> &str {
        match self.prompt.is_empty() {
            true => &self.label,
            false => &self.prompt,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// 英文外觀規格塊：有就取代公開設定當立繪描述
    pub spec: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

/// 逐角色的美術欄位
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CastArt {
    pub appearance: Option<Appearance>,
    /// 固定五鍵以外的表情（卡上的 `expressions_extra`）
    pub expressions: Vec<ArtEntry>,
    /// 卡宣告的變體（保留鍵，玩家按生成才生）
    pub variants: Vec<ArtEntry>,
    pub height_cm: Option<u16>,
}

impl CastArt {
    pub fn expression(&self, key: &str) -> Option<&ArtEntry> {
        self.expressions.iter().find(|entry| entry.key == key)
    }

    pub fn variant(&self, key: &str) -> Option<&ArtEntry> {
        self.variants.iter().find(|entry| entry.key == key)
    }

    pub fn expression_keys(&self) -> Vec<String> {
        self.expressions
            .iter()
            .map(|entry| entry.key.clone())
            .collect()
    }
}

/// 表情、變體、地點、時段鍵：`[a-z0-9_-]{1,24}`（會進檔名；匯入時驗，組路徑時再驗）
pub fn valid_art_key(key: &str) -> bool {
    (1..=24).contains(&key.len())
        && key
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

/// 畫風與避免項的字數上限：卡作者欄位與玩家欄位共用
pub const STYLE_CAP: usize = 600;
pub const AVOID_CAP: usize = 300;

/// 美術文字欄位的清洗：控制字元換空白、去頭尾空白、截到上限。回 (文字, 是否截過)
pub fn clean_art_text(text: &str, cap: usize) -> (String, bool) {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    match text.chars().count() <= cap {
        true => (text.to_owned(), false),
        false => (text.chars().take(cap).collect(), true),
    }
}
