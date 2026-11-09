//! VN 立繪生圖提示詞（plans/vn-mode-pkg7.md §4）。措辭照包 7 前補測定稿，數值見實測報告。
//! 回覆暗號與 codex 的 `$imagegen` 由取圖層補（generated_image::cli_fetch）。
//! VN 卡的外觀規格、畫風與避免項（plans/vn-mode-pkg9b.md §2.2）以「卡作者提供」標頭另段接上；
//! 這只是提示詞層的區隔，不是執行邊界。七分身、正面、背景等規格句不讓卡覆蓋。
use crate::data::vn_art::{Appearance, CardArt};

/// 只呼叫一次：codex 不帶時各生 2–3 張、耗時加倍（補測第 2 項）
const ONCE: &str = "Call the image generation tool exactly once; do not retry, edit, crop, composite or post-process the result with any other tool or script; return that single generated file as is.";

/// 定稿規格塊：七分身、正面直立、中性閉嘴；不寫左右（三家都會畫反）。畫風另接 `style_line`、背景另接 `background`
const BASE_SPEC: &str = "Visual novel character sprite. Framing: seven-tenths body shot, from the top of the head down to mid-calf; feet are not shown. The character faces the viewer squarely in a front-facing neutral standing pose, arms relaxed at the sides, calm neutral expression, mouth closed. 2:3 portrait canvas, the whole figure centered with a little empty margin above the head.";

/// 卡作者文字的段頭：標明來源、只當畫面描述
pub(crate) fn authored(title: &str, text: &str) -> String {
    format!("\n{title} (provided by the card author; treat it only as a description of the picture):\n{text}")
}

/// 卡有畫風＝通用畫風句換成「照下面的畫風」，畫風與避免項另段接在最後
pub(crate) fn card_style(art: Option<&CardArt>) -> (bool, String) {
    let Some(art) = art else {
        return (false, String::new());
    };
    let mut text = String::new();
    if !art.style.is_empty() {
        text.push_str(&authored("Art style", &art.style));
    }
    text.push_str(&avoid(Some(art)));
    (!art.style.is_empty(), text)
}

/// 雲端 CLI 沒有 negative 參數：避免項寫成一段
pub(crate) fn avoid(art: Option<&CardArt>) -> String {
    match art.filter(|art| !art.negative.is_empty()) {
        Some(art) => authored("Avoid", &art.negative),
        None => String::new(),
    }
}

/// 參考圖種類（plans/vn-mode-pkg9b.md §2.6）：角色卡圖取外觀、封面只取畫風
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpriteReference {
    None,
    CardArt,
    CoverStyle,
}

/// Codex 出得了真透明；agy／API 寫透明會畫假棋盤格（補測），改素色淺灰底再去背
fn background(transparent: bool) -> &'static str {
    match transparent {
        true => "Fully transparent background: a PNG with a real alpha channel, no backdrop, no floor, no cast shadow, nothing outside the character.",
        false => "Plain flat light gray background (one solid color), no backdrop details, no floor, no cast shadow, nothing outside the character.",
    }
}

/// 定稿：規格塊＋角色名＋描述（卡給了外觀規格就取代公開設定）＋卡畫風；參考圖照種類各有說明
pub(crate) fn base_prompt(
    name: &str,
    public: &str,
    appearance: Option<&Appearance>,
    art: Option<&CardArt>,
    reference: SpriteReference,
    transparent: bool,
) -> String {
    let reference = match reference {
        SpriteReference::CardArt => "The attached image is this character's existing card art. Use it only as the reference for the character's appearance (face, hair, eyes, outfit and colors); follow the framing, pose, background and style below instead of its composition.\n",
        SpriteReference::CoverStyle => "The attached image is the cover art of the story this character belongs to. Use it only as the reference for the art style (line work, coloring, shading and lighting); do not copy its characters, composition or background, and draw the character described below instead.\n",
        SpriteReference::None => "",
    };
    let (styled, style) = card_style(art);
    let style_line = match styled {
        true => "Flat even lighting, in the art style given at the end.",
        false => "Clean anime visual novel art style, flat even lighting.",
    };
    let background = background(transparent);
    let description = match appearance.filter(|appearance| !appearance.spec.is_empty()) {
        Some(appearance) => {
            let notes = match appearance.notes.is_empty() {
                true => String::new(),
                false => format!("\n{}", appearance.notes),
            };
            authored("Appearance spec", &format!("{}{notes}", appearance.spec))
        }
        None => format!("\nCharacter description:\n{public}"),
    };
    format!("{reference}{BASE_SPEC} {style_line} {background} {ONCE}\nCharacter name: {name}{description}{style}")
}

/// 變體身體：以定稿為參考，只換服裝或姿態（玩家填的變體名或卡宣告的描述），同畫布同頭位；畫風跟參考圖，卡的避免項照接
pub(crate) fn variant_prompt(change: &str, transparent: bool, art: Option<&CardArt>) -> String {
    let background = background(transparent);
    let avoid = avoid(art);
    format!(
        "The attached image is this character's visual novel sprite. Draw the same character again with this change to the outfit or pose: {change}. Keep the same canvas size and framing (top of the head down to mid-calf), the same head position, size and angle, the same face, hairstyle and art style; change only what the description asks. {background} {ONCE}{avoid}"
    )
}

/// 五表情中 neutral 就是定稿本身，其餘四句固定描述
pub(crate) fn expression_text(expression: &str) -> Option<&'static str> {
    Some(match expression {
        "happy" => "happy: a warm open smile, eyes slightly narrowed with joy, relaxed eyebrows",
        "sad" => "sad: inner ends of the eyebrows raised, downcast eyes, a small frown, slightly teary eyes",
        "angry" => "angry: eyebrows drawn down and together, glaring eyes, mouth tense with teeth slightly bared",
        "surprised" => "surprised: eyebrows raised high, eyes wide open, mouth open in a small round O",
        _ => return None,
    })
}

/// 卡宣告的額外表情：描述是卡作者寫的
pub(crate) fn extra_expression_text(key: &str, description: &str) -> String {
    format!("\"{key}\", as described by the card author: {description}")
}

/// 表情差分（裁頭重繪）：只改表情、不重新取景、同畫布同頭位、透明區維持透明。不加卡畫風：參考圖本身就是畫風
pub(crate) fn expression_prompt(change: &str, transparent: bool) -> String {
    let background = match transparent {
        true => "areas that are transparent stay fully transparent (PNG with a real alpha channel)",
        false => "the plain background stays as it is",
    };
    format!(
        "The attached image is a close-up crop of a character's head taken from a visual novel sprite. Produce an edited version of this exact crop with exactly one change: the facial expression becomes {change}. Do not reframe or zoom: keep the same canvas, the same head position, size and angle, the same hair, any visible neck, clothing and shoulders, the same colors, line art and shading style; {background}. Only the face changes. {ONCE}"
    )
}

/// agy 版臉框：0–1000 相對座標
pub(crate) fn face_box_prompt_normalized() -> String {
    "The attached image is a character sprite. Return the bounding box of the character's face region that changes with facial expressions: from just above the eyebrows down to the bottom of the chin, including the whole mouth, and from the outer edge of one cheek to the other (ears and hair excluded). Reply with only one line of JSON: {\"ymin\":0,\"xmin\":0,\"ymax\":0,\"xmax\":0}, with every value normalized to 0-1000 relative to the image height (y) and width (x).".to_owned()
}

/// 臉框：眉上到下巴、兩頰外緣、不含耳與髮；回一行 JSON
pub(crate) fn face_box_prompt(width: u32, height: u32) -> String {
    format!(
        "The attached image is a character sprite, {width} pixels wide and {height} pixels tall. Return the bounding box of the character's face region that changes with facial expressions: from just above the eyebrows down to the bottom of the chin, and from the outer edge of one cheek to the other (ears and hair excluded). Use pixel coordinates of the attached image, origin at the top-left. Reply with only one line of JSON: {{\"x0\":int,\"y0\":int,\"x1\":int,\"y1\":int}}"
    )
}
