//! GM 這一份怎麼拿到卡片開場白（greeting-context-st-parity）：ST 只把選中的開場白當聊天第 0 則送原文，
//! 不放進人設。卡片的開場白段由 `chat_assembly::greeting` 決定剝不剝；這裡只管 GM 送出層的兩件事。

use super::arrivals::Side;
use super::messages::scaffold_en;

/// GM 送出層的開場白政策。預設＝現況（開場照 text、沒有範例）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GmOpening {
    /// 本幕的開場事件改送原文（`raw`，沒有就退回 `text`）：卡片自帶的狀態欄等格式範例跟著回來。
    /// 只換 GM 送出層；世界書掃描、巨集、角色線照用 `text`。
    pub raw_history: bool,
    /// 本幕沒有開場事件時，本分支最近一則歷史開場的原文：放 GM system 獨立一段當格式範例。
    pub example: Option<String>,
}

impl GmOpening {
    /// GM 歷史渲染用的視角。
    pub fn side(&self) -> Side {
        match self.raw_history {
            true => Side::GmOpeningRaw,
            false => Side::Gm,
        }
    }
}

/// GM system 的格式範例段（前後各帶換行）；沒有範例回空字串。不進世界書掃描看得到的任何欄位。
pub(super) fn example_section(opening: &GmOpening, lang: &str) -> String {
    let Some(example) = opening
        .example
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return String::new();
    };
    let heading = match scaffold_en(lang) {
        true => {
            "\n## Opening of this story (format example only)\nThis is how the story opened. Use it only as an \
             example of the reply format the card expects; the status values in it are from back then, not the \
             current ones.\n"
        }
        false => {
            "\n## 這段故事的開場（僅供格式範例）\n這是故事開場時的內容，只當這張卡要求的回覆格式範例；其中的狀態是當時的值，\
             不是目前的值。\n"
        }
    };
    format!("{heading}{example}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_section_only_when_present() {
        assert_eq!(example_section(&GmOpening::default(), "zh-TW"), "");
        let blank = GmOpening {
            raw_history: false,
            example: Some("  \n".into()),
        };
        assert_eq!(example_section(&blank, "zh-TW"), "");
        let opening = GmOpening {
            raw_history: false,
            example: Some("開場<details>x</details>".into()),
        };
        let zh = example_section(&opening, "zh-TW");
        assert!(zh.contains("僅供格式範例") && zh.ends_with("開場<details>x</details>\n"));
        assert!(example_section(&opening, "en").contains("format example only"));
    }

    #[test]
    fn side_follows_raw_history() {
        assert_eq!(GmOpening::default().side(), Side::Gm);
        let raw = GmOpening {
            raw_history: true,
            example: None,
        };
        assert_eq!(raw.side(), Side::GmOpeningRaw);
    }
}
