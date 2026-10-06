//! 容量判定用的長度量法。`usage::log::estimate_tokens` 只供診斷（中文一字一 token、符號全不算），
//! 這裡另立一支偏保守的估計：寧可略高，提醒早一點；鎖不靠它單獨決定（要配實報校正，見 capacity）。

/// 長度單位：token 後端量 token；agy 的上限是單則 body 的 UTF-8 bytes（截尾門檻，實測）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Tokens,
    Bytes,
}

/// 保守 token 估計。係數取「比 claude 實測略高」的值（2026-10-06 haiku 五類夾具實報，見下方測試）：
/// 中日韓等非 ASCII 字 1.45（中文實報約 1.33／字）、ASCII 字母 0.3、數字 1.0（數字常各自成 token）、
/// ASCII 符號 0.7（JSON／狀態區塊的 `{}":` 常各自成 token）、空白 0.15。
pub fn budget_tokens(text: &str) -> u64 {
    let mut total = 0.0f64;
    for ch in text.chars() {
        total += token_weight(ch);
    }
    total.ceil() as u64
}

/// 單一字元在保守估計裡的權重（未取整）。
pub fn token_weight(ch: char) -> f64 {
    if !ch.is_ascii() {
        1.45
    } else if ch.is_ascii_alphabetic() {
        0.3
    } else if ch.is_ascii_digit() {
        1.0
    } else if ch.is_ascii_whitespace() {
        0.15
    } else {
        0.7
    }
}

/// 單一字元在某單位下的權重（未取整）：bytes 是 UTF-8 長度。
pub fn char_weight(unit: Unit, ch: char) -> f64 {
    match unit {
        Unit::Tokens => token_weight(ch),
        Unit::Bytes => ch.len_utf8() as f64,
    }
}

/// 一段文字在某單位下的量。bytes 就是 UTF-8 長度（agy 截尾看的就是它）。
pub fn measure(unit: Unit, text: &str) -> u64 {
    match unit {
        Unit::Tokens => budget_tokens(text),
        Unit::Bytes => text.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::log::estimate_tokens;

    #[test]
    fn conservative_against_the_diagnostic_estimate_on_every_kind_of_text() {
        let samples = [
            "雷恩推開酒館的門，雨水順著斗篷滴在地板上。",
            "Rain dripped from his cloak onto the tavern floor as Ren pushed the door open.",
            "雷恩（Ren）說：「Let's go。」HP 12/20，MP 3。",
            r#"{"hp":12,"mp":3,"status":["poisoned","tired"],"loc":{"x":4,"y":-2}}"#,
            "<|>{}[]\"'`~!@#$%^&*()_+-=|\\:;<>,.?/",
        ];
        for text in samples {
            assert!(
                budget_tokens(text) >= estimate_tokens(text),
                "{text}: {} < {}",
                budget_tokens(text),
                estimate_tokens(text)
            );
        }
        // 符號密集的文本：診斷估計幾乎不算，保守估計要算進去
        let symbols = "{}[]\":,{}[]\":,{}[]\":,";
        assert_eq!(estimate_tokens(symbols), 5);
        assert!(budget_tokens(symbols) >= 14);
    }

    /// 2026-10-06 haiku 實報（claude CLI --safe-mode、--system-prompt-file，單字 "x" 的基線 573 已扣）：
    /// 每類夾具重複 20 次送出，保守估計不得低於實報。
    #[test]
    fn never_below_haiku_reports_on_the_five_fixture_kinds() {
        let fixtures: [(&str, u64); 5] = [
        ("雷恩推開酒館的門，雨水順著斗篷滴在地板上。吧台後的老闆抬起頭，眼神在他腰間的長劍上停了一瞬，又若無其事地擦起杯子。角落裡兩個傭兵壓低聲音交談，提到北方山口的封鎖與失蹤的商隊。", 2317), // 中文散文：重複 20 次後的實報
        ("Ren pushed the tavern door open, rain dripping from his cloak onto the floorboards. Behind the bar the keeper looked up, his gaze lingering on the longsword at Ren's hip before he went back to polishing a mug. In the corner two mercenaries spoke in low voices about the blockade at the northern pass and a caravan that never arrived.", 1696), // 英文散文：重複 20 次後的實報
        ("雷恩（Ren）把 3 枚金幣拍在吧台上：「Two ales, and whatever you know about the Northern Pass。」老闆 Garrick 說 HP 12/20、MP 3/10，任務 #42 已更新。", 1438), // 中英混排：重複 20 次後的實報
        ("{\"hp\":12,\"mp\":3,\"status\":[\"poisoned\",\"tired\"],\"loc\":{\"x\":4,\"y\":-2},\"inventory\":[{\"id\":\"sword\",\"qty\":1},{\"id\":\"potion\",\"qty\":3}],\"flags\":{\"met_garrick\":true,\"pass_open\":false}}", 1416), // JSON 狀態：重複 20 次後的實報
        ("<|>{}[]\"'`~!@#$%^&*()_+-=|\\:;<>,.?/ ==> <== ||| ### *** ~~~ --- +++ ::: ;;; ,,, ... ??? !!! (( )) [[ ]] {{ }} << >>", 1354), // 符號密集：重複 20 次後的實報
        ];
        for (unit, reported) in fixtures {
            let text = unit.repeat(20);
            assert!(
                budget_tokens(&text) >= reported,
                "{}: {} < {}",
                &unit[..unit.char_indices().nth(10).map_or(unit.len(), |(i, _)| i)],
                budget_tokens(&text),
                reported
            );
        }
    }

    #[test]
    fn bytes_are_utf8_length() {
        assert_eq!(measure(Unit::Bytes, "測a"), 4);
        assert_eq!(measure(Unit::Tokens, ""), 0);
    }
}
