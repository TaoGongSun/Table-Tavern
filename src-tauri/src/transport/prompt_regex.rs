//! 卡片 promptOnly 正則（card-prompt-only-regex）：ST 生成時對歷史每則套的「只改送模型那份」腳本。
//! 只支援替換成空字串的剝除寫法（TestCards 全部形狀）；其餘寫法略過不套。只動送出副本，
//! 事件、世界書掃描、lane 水位與指紋都不經過這裡。

use crate::data::{TranscriptEvent, TranscriptKind};
use crate::world_info::regex_key::regex_from_string;
use fancy_regex::Regex;
use serde_json::Value;

const USER_INPUT: i64 = 1;
const AI_OUTPUT: i64 = 2;

#[derive(Debug, Clone)]
struct PromptScript {
    regex: Regex,
    global: bool,
    user: bool,
    ai: bool,
    min_depth: Option<i64>,
    max_depth: Option<i64>,
}

/// 一桌要套的 promptOnly 腳本（照卡內順序，前一條的結果交給下一條）。預設空＝原文照送。
#[derive(Debug, Clone, Default)]
pub struct PromptRegex(Vec<PromptScript>);

impl PromptRegex {
    /// 從卡內 `regex_scripts` 條目建：啟用、promptOnly、placement 有使用者輸入或 AI 輸出，
    /// 且替換字串空、無 trimStrings、無巨集替換；正則解不開的略過。
    pub fn from_scripts<'a>(scripts: impl IntoIterator<Item = &'a Value>) -> Self {
        let flag = |script: &Value, key| script.get(key).and_then(Value::as_bool) == Some(true);
        let text = |script: &'a Value, key| script.get(key).and_then(Value::as_str).unwrap_or("");
        let depth = |script: &Value, key| script.get(key).and_then(Value::as_i64);
        PromptRegex(
            scripts
                .into_iter()
                .filter(|script| {
                    flag(script, "promptOnly")
                        && !flag(script, "disabled")
                        && !text(script, "findRegex").is_empty()
                        && text(script, "replaceString").is_empty()
                        && script
                            .get("trimStrings")
                            .and_then(Value::as_array)
                            .map_or(true, Vec::is_empty)
                        && script
                            .get("substituteRegex")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            == 0
                })
                .filter_map(|script| {
                    let placement: Vec<i64> = script
                        .get("placement")?
                        .as_array()?
                        .iter()
                        .filter_map(Value::as_i64)
                        .collect();
                    let (regex, global) = regex_from_string(text(script, "findRegex"))?;
                    Some(PromptScript {
                        regex,
                        global,
                        user: placement.contains(&USER_INPUT),
                        ai: placement.contains(&AI_OUTPUT),
                        min_depth: depth(script, "minDepth"),
                        max_depth: depth(script, "maxDepth"),
                    })
                })
                .filter(|script| script.user || script.ai)
                .collect(),
        )
    }

    /// 套在一則事件送出的本文（名字前綴之前）。`depth`＝這則之後還有幾則非系統事件（最後一則 0）。
    /// 系統事件不套；玩家發言算使用者輸入，旁白與台詞算 AI 輸出。
    pub fn apply(&self, kind: &TranscriptKind, text: String, depth: usize) -> String {
        let user = match kind {
            TranscriptKind::System => return text,
            TranscriptKind::Player => true,
            TranscriptKind::Narration | TranscriptKind::Dialogue => false,
        };
        let depth = depth as i64;
        self.0
            .iter()
            .filter(|script| if user { script.user } else { script.ai })
            // ST：minDepth ≥ -1 且深度不到就跳過；maxDepth ≥ 0 且超過就跳過
            .filter(|script| {
                script
                    .min_depth
                    .map_or(true, |min| min < -1 || depth >= min)
            })
            .filter(|script| script.max_depth.map_or(true, |max| max < 0 || depth <= max))
            .fold(text, |text, script| {
                strip(&script.regex, script.global, &text).unwrap_or(text)
            })
    }

    /// 當最舊一則 AI 輸出套（換幕後 GM system 裡的開場格式範例）。
    pub fn apply_oldest_ai(&self, text: String) -> String {
        self.apply(&TranscriptKind::Narration, text, usize::MAX >> 1)
    }
}

/// 每則事件之後還有幾則非系統事件（ST 的訊息深度；系統事件不是聊天訊息、不佔深度）。
pub fn depths(events: &[TranscriptEvent]) -> Vec<usize> {
    let mut after = 0;
    let mut result: Vec<usize> = events
        .iter()
        .rev()
        .map(|event| {
            let depth = after;
            if event.kind != TranscriptKind::System {
                after += 1;
            }
            depth
        })
        .collect();
    result.reverse();
    result
}

/// 剝掉命中段（替換成空字串）；沒有 g 只剝第一個。回溯超限回 `None`（原文照送）。
fn strip(regex: &Regex, global: bool, text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for found in regex.find_iter(text) {
        let found = found.ok()?;
        out.push_str(&text[last..found.start()]);
        last = found.end();
        if !global {
            break;
        }
    }
    out.push_str(&text[last..]);
    Some(out)
}

#[cfg(test)]
#[path = "prompt_regex_tests.rs"]
mod tests;
