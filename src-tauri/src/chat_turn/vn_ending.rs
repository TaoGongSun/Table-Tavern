//! VN 桌 `[chapter]`／`[ending]` 的提交時驗證（plans/vn-mode-pkg5.md §2.2）：只在 GM 回合提交時判一次，
//! 結果寫回事件文字（剝掉不成立的標記、強制收尾補 `[ending: default]`）與狀態樹（`ended`／`finale`），
//! 之後讀取一律信任存下的文字。
use super::vn_script::{terminal_of, Terminal, VnStop};
use crate::data::vn::{Ending, DEFAULT_ENDING};
use crate::data::{Mechanism, StateNode};
use std::collections::BTreeMap;

/// 判定的輸入：結局表（沒聖經＝只有 `default`）、章號與上限、是否強制收尾、套完本拍更新後的樹。
pub(super) struct Judge<'a> {
    pub endings: &'a [Ending],
    pub chapter: u32,
    pub max: u32,
    pub finale: bool,
    pub tree: &'a BTreeMap<String, StateNode>,
    pub mechanism: &'a Mechanism,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Verdict {
    /// `[chapter]` 留在文字（＝章結束流程的觸發）
    keep_chapter: bool,
    /// 接受的結局 id
    pub ending: Option<String>,
    /// GM 沒給有效結局、引擎補一行 `[ending: default]`
    append_default: bool,
    /// 第 M 章的 `[chapter]` 轉成強制收尾
    pub finale: bool,
    /// 下一輪給模型看的回饋句
    pub notes: Vec<String>,
}

impl Judge<'_> {
    fn accepts(&self, id: &str) -> bool {
        self.endings
            .iter()
            .find(|ending| ending.id == id)
            .is_some_and(|ending| {
                (self.finale || id != DEFAULT_ENDING)
                    && ending.when.iter().all(|condition| {
                        crate::mechanism::condition_holds(self.tree, self.mechanism, condition)
                    })
            })
    }

    pub(super) fn judge(&self, text: &str) -> Verdict {
        let marks: Vec<Terminal> = text.lines().filter_map(terminal_of).collect();
        let ids: Vec<&str> = marks
            .iter()
            .filter_map(|mark| match mark {
                Terminal::Ending(id) => Some(id.as_str()),
                Terminal::Chapter => None,
            })
            .collect();
        let mut verdict = Verdict {
            ending: ids
                .iter()
                .find(|id| self.accepts(id))
                .map(|id| (*id).to_owned()),
            ..Verdict::default()
        };
        if verdict.ending.is_none() && !self.finale {
            verdict.notes = ids.iter().map(|id| self.refusal(id)).collect();
        }
        if self.finale && verdict.ending.is_none() {
            verdict.ending = Some(DEFAULT_ENDING.to_owned());
            verdict.append_default = true;
        }
        if verdict.ending.is_none() && !self.finale && marks.contains(&Terminal::Chapter) {
            match self.chapter < self.max {
                true => verdict.keep_chapter = true,
                false => verdict.finale = true,
            }
        }
        verdict
    }

    fn refusal(&self, id: &str) -> String {
        match self.endings.iter().find(|ending| ending.id == id) {
            None => format!("結局「{id}」不在結局表內，標記已剝掉，故事繼續。"),
            Some(_) if id == DEFAULT_ENDING => {
                "結局「default」只在導演指示要求收尾時使用，標記已剝掉，故事繼續。".to_owned()
            }
            Some(ending) => {
                let when: Vec<String> = ending
                    .when
                    .iter()
                    .map(crate::transport::vn_bible_prompts::condition_text)
                    .collect();
                format!(
                    "結局「{id}」的條件還不成立（{}），標記已剝掉，故事繼續。",
                    when.join("，且 ")
                )
            }
        }
    }
}

impl Verdict {
    /// 不驗結局的桌（zod 引擎）：`[chapter]` 照留、`[ending]` 一律剝。
    pub(crate) fn unjudged() -> Self {
        Self {
            keep_chapter: true,
            ..Self::default()
        }
    }

    /// 照判定改寫文字：不收的 `[chapter]`／`[ending]` 整行剝掉，強制收尾補一行。`text` 與 `raw` 都用它。
    pub(crate) fn rewrite(&self, text: &str) -> String {
        let keep = |line: &&str| match terminal_of(line) {
            Some(Terminal::Chapter) => self.keep_chapter,
            Some(Terminal::Ending(id)) => self.ending.as_deref() == Some(id.as_str()),
            None => true,
        };
        if !self.append_default && text.lines().all(|line| keep(&line)) {
            return text.to_owned();
        }
        let mut out = text.lines().filter(keep).collect::<Vec<_>>().join("\n");
        if self.append_default {
            out = format!("{}\n[ending: {DEFAULT_ENDING}]", out.trim_end());
        }
        out
    }

    /// 寫進狀態樹：`ended` 接受的結局、`finale` 轉收尾；回饋句接在既有拒收句後面。
    pub(crate) fn apply(&self, state: &mut crate::data::TableState) {
        let path = |key: &str| vec![key.to_owned()];
        if let Some(id) = &self.ending {
            crate::data::set_tree_value(&mut state.tree, &path("ended"), id);
            crate::data::set_tree_value(&mut state.tree, &path("finale"), "false");
        } else if self.finale {
            crate::data::set_tree_value(&mut state.tree, &path("finale"), "true");
        }
        state.notes.extend(self.notes.iter().cloned());
    }

    /// 這一拍因第 M 章的 `[chapter]` 轉收尾：沒有其他終止標記時告訴前端停下（顯示「繼續」）。
    pub(crate) fn stop(&self) -> Option<VnStop> {
        (self.finale && self.ending.is_none()).then_some(VnStop::Finale)
    }
}

#[cfg(test)]
#[path = "vn_ending_tests.rs"]
mod tests;
