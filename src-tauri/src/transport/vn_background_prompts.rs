//! VN 背景生圖提示詞（plans/vn-mode-pkg3.md §2）：橫式、無人物、無文字；有參考圖＝同地點只改光線。
//! 回覆暗號與 codex 的 `$imagegen` 由取圖層補（generated_image::cli_fetch）。

const ONCE: &str = "Call the image generation tool exactly once; do not retry, edit, crop, composite or post-process the result with any other tool or script; return that single generated file as is.";

const SPEC: &str = "Visual novel background art. Landscape orientation, 16:9 wide. An empty scene: no people, no characters, no animals in focus, no text, no captions, no signatures, no UI. Clean anime visual novel art style, painterly detail, eye-level view with open space in the lower middle where character sprites will stand.";

fn lighting(period: &str) -> &'static str {
    match period {
        "morning" => "early morning: soft low sunlight, cool fresh air, long gentle shadows",
        "dusk" => {
            "dusk: warm orange and pink sunset light, long shadows, the sky fading toward evening"
        }
        "night" => "night: dark sky, moonlight and artificial lights only, deep blue shadows",
        _ => "daytime: bright, clear, neutral midday light",
    }
}

/// 從零生，或以同地點另一時段為參考圖只改光線（構圖、物件、視角都不動）
pub(crate) fn background_prompt(
    location: &str,
    period: &str,
    setting: &str,
    has_reference: bool,
) -> String {
    let lighting = lighting(period);
    let setting = match setting.trim() {
        "" => String::new(),
        setting => format!(
            "\nWorld setting (for style and era only; do not depict text from it):\n{setting}"
        ),
    };
    match has_reference {
        true => format!(
            "The attached image is a visual novel background of this location. Produce the same background at a different time of day: {lighting}. Keep the exact same composition, camera angle, framing, buildings, objects and art style; change only the lighting, sky and colors to match the time of day. {SPEC} {ONCE}\nLocation: {location}{setting}"
        ),
        false => format!("{SPEC} Time of day: {lighting}. {ONCE}\nLocation: {location}{setting}"),
    }
}
