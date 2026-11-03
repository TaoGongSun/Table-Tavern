//! VN 立繪生圖提示詞（plans/vn-mode-pkg7.md §4）。措辭照包 7 前補測定稿，數值見實測報告。
//! 回覆暗號與 codex 的 `$imagegen` 由取圖層補（generated_image::cli_fetch）。

/// 只呼叫一次：codex 不帶時各生 2–3 張、耗時加倍（補測第 2 項）
const ONCE: &str = "Call the image generation tool exactly once; do not retry, edit, crop, composite or post-process the result with any other tool or script; return that single generated file as is.";

/// 定稿規格塊：七分身、正面直立、中性閉嘴；不寫左右（三家都會畫反）。背景另接 `background`
const BASE_SPEC: &str = "Visual novel character sprite. Framing: seven-tenths body shot, from the top of the head down to mid-calf; feet are not shown. The character faces the viewer squarely in a front-facing neutral standing pose, arms relaxed at the sides, calm neutral expression, mouth closed. Portrait orientation, the whole figure centered with a little empty margin above the head. Clean anime visual novel art style, flat even lighting.";

/// Codex 出得了真透明；agy／API 寫透明會畫假棋盤格（補測），改素色淺灰底再去背
fn background(transparent: bool) -> &'static str {
    match transparent {
        true => "Fully transparent background: a PNG with a real alpha channel, no backdrop, no floor, no cast shadow, nothing outside the character.",
        false => "Plain flat light gray background (one solid color), no backdrop details, no floor, no cast shadow, nothing outside the character.",
    }
}

/// 定稿：規格塊＋角色名＋公開設定；有卡圖就當外觀參考
pub(crate) fn base_prompt(
    name: &str,
    description: &str,
    has_reference: bool,
    transparent: bool,
) -> String {
    let reference = match has_reference {
        true => "The attached image is this character's existing card art. Use it only as the reference for the character's appearance (face, hair, eyes, outfit and colors); follow the framing, pose, background and style below instead of its composition.\n",
        false => "",
    };
    let background = background(transparent);
    format!("{reference}{BASE_SPEC} {background} {ONCE}\nCharacter name: {name}\nCharacter description:\n{description}")
}

/// 變體身體：以定稿為參考，只換服裝或姿態（玩家填的變體名就是描述），同畫布同頭位
pub(crate) fn variant_prompt(change: &str, transparent: bool) -> String {
    let background = background(transparent);
    format!(
        "The attached image is this character's visual novel sprite. Draw the same character again with this change to the outfit or pose: {change}. Keep the same canvas size and framing (top of the head down to mid-calf), the same head position, size and angle, the same face, hairstyle and art style; change only what the description asks. {background} {ONCE}"
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

/// 表情差分（裁頭重繪）：只改表情、不重新取景、同畫布同頭位、透明區維持透明
pub(crate) fn expression_prompt(expression: &str, transparent: bool) -> Option<String> {
    let change = expression_text(expression)?;
    let background = match transparent {
        true => "areas that are transparent stay fully transparent (PNG with a real alpha channel)",
        false => "the plain background stays as it is",
    };
    Some(format!(
        "The attached image is a close-up crop of a character's head taken from a visual novel sprite. Produce an edited version of this exact crop with exactly one change: the facial expression becomes {change}. Do not reframe or zoom: keep the same canvas, the same head position, size and angle, the same hair, any visible neck, clothing and shoulders, the same colors, line art and shading style; {background}. Only the face changes. {ONCE}"
    ))
}

/// 臉框：眉上到下巴、兩頰外緣、不含耳與髮；回一行 JSON
pub(crate) fn face_box_prompt(width: u32, height: u32) -> String {
    format!(
        "The attached image is a character sprite, {width} pixels wide and {height} pixels tall. Return the bounding box of the character's face region that changes with facial expressions: from just above the eyebrows down to the bottom of the chin, and from the outer edge of one cheek to the other (ears and hair excluded). Use pixel coordinates of the attached image, origin at the top-left. Reply with only one line of JSON: {{\"x0\":int,\"y0\":int,\"x1\":int,\"y1\":int}}"
    )
}
