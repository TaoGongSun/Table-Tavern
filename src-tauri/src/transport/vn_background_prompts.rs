//! VN 背景生圖提示詞（plans/vn-mode-pkg3.md §2）：橫式、無人物、無文字；有參考圖＝同地點只改光線，或封面只取畫風。
//! VN 卡地點的描述與尺度、畫風（玩家優先、否則卡作者）、背景規則與避免項（plans/vn-mode-pkg9b.md §2.5–§2.6）以「卡作者提供」標頭另段接上。
//! 回覆暗號與 codex 的 `$imagegen` 由取圖層補（generated_image::cli_fetch）。
use super::vn_sprite_prompts::{authored, card_style, ArtStyle};
use crate::data::vn_art::CardLocation;

const ONCE: &str = "Call the image generation tool exactly once; do not retry, edit, crop, composite or post-process the result with any other tool or script; return that single generated file as is.";

const SPEC: &str = "Visual novel background art. Landscape orientation, 16:9 wide. An empty scene: no people, no characters, no animals in focus, no text, no captions, no signatures, no UI.";

const FRAMING: &str =
    "Eye-level view with open space in the lower middle where character sprites will stand.";

/// 參考圖種類（plans/vn-mode-pkg9b.md §2.6）：同地點另一時段鎖構圖只改光線、封面只取畫風
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackgroundReference {
    None,
    SameLocation,
    CoverStyle,
}

/// 固定四時段的光線句；卡自訂的時段鍵（如 moonlight、crisis）照寫
fn lighting(period: &str) -> String {
    match period {
        "morning" => "Time of day: early morning: soft low sunlight, cool fresh air, long gentle shadows".to_owned(),
        "day" => "Time of day: daytime: bright, clear, neutral midday light".to_owned(),
        "dusk" => {
            "Time of day: dusk: warm orange and pink sunset light, long shadows, the sky fading toward evening".to_owned()
        }
        "night" => "Time of day: night: dark sky, moonlight and artificial lights only, deep blue shadows".to_owned(),
        custom => format!("Time of day / mood variant: {custom}"),
    }
}

/// `place`＝卡內地點（有就用卡上的描述與尺度）；`location`＝顯示名
pub(crate) fn background_prompt(
    location: &str,
    place: Option<&CardLocation>,
    period: &str,
    setting: &str,
    reference: BackgroundReference,
    art: ArtStyle,
) -> String {
    let lighting = lighting(period);
    let setting = match setting.trim() {
        "" => String::new(),
        setting => format!(
            "\nWorld setting (for style and era only; do not depict text from it):\n{setting}"
        ),
    };
    let (styled, mut style) = card_style(art);
    if let Some(rules) = art.background_rules() {
        // 卡作者的背景規則常夾帶畫風字眼；玩家有填畫風時只取構圖
        let title = match art.player_styled() {
            true => "Background rules (composition only; ignore any art style named here, the player's art style takes priority)",
            false => "Background rules",
        };
        style.push_str(&authored(title, rules));
    }
    let style_line = match styled {
        true => "Painterly detail, in the art style given at the end.",
        false => "Clean anime visual novel art style, painterly detail.",
    };
    let mut described = format!("\nLocation: {location}");
    if let Some(place) = place {
        if !place.prompt.is_empty() {
            described.push_str(&authored("Location description", &place.prompt));
        }
        if !place.scale.is_empty() {
            described.push_str(&authored("Scale notes", &place.scale));
        }
    }
    let head = match reference {
        BackgroundReference::SameLocation => format!(
            "The attached image is a visual novel background of this location. Produce the same background at a different time of day. {lighting}. Keep the exact same composition, camera angle, framing, buildings, objects and art style; change only the lighting, sky and colors to match the time of day. {SPEC} {FRAMING}"
        ),
        BackgroundReference::CoverStyle => format!(
            "The attached image is the cover art of this story. Use it only as the reference for the art style (line work, coloring, shading and lighting); do not draw its characters and do not copy its composition. {SPEC} {style_line} {FRAMING} {lighting}."
        ),
        BackgroundReference::None => format!("{SPEC} {style_line} {FRAMING} {lighting}."),
    };
    format!("{head} {ONCE}{described}{setting}{style}")
}
