//! VN 桌（vn-mode）的 Tauri 邊界：接參數、轉呼叫 `chat_turn` 的劇本解析與換一組。
use crate::chat_turn::{self, VnCell, VnRoster, VnSource, VnText};
use crate::{config_root, data, data_root, transport};
use serde::Serialize;

/// 一則事件的正式格（包 2 播放器只消費這個）。
#[derive(Serialize)]
pub(crate) struct VnEventCells {
    event_id: Option<String>,
    kind: data::TranscriptKind,
    cells: Vec<VnCell>,
}

/// 一幕事件逐則解析：GM 旁白照 GM 線、角色台詞照該角色線；玩家句原樣一格旁白；系統事件略過。
#[tauri::command]
pub(crate) fn read_vn_cells(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<Vec<VnEventCells>, String> {
    let root = data_root(&app)?;
    let roster = VnRoster::load(&root, &world_id)?;
    let events =
        data::read_transcript(&root, &world_id, scene).map_err(|error| error.to_string())?;
    Ok(events
        .into_iter()
        .filter_map(|event| {
            let cells = match event.kind {
                data::TranscriptKind::Narration => {
                    chat_turn::parse_event(&event.text, &VnSource::Gm, &roster)
                }
                data::TranscriptKind::Dialogue => chat_turn::parse_event(
                    &event.text,
                    &VnSource::Character {
                        self_id: event.speaker_id.clone(),
                    },
                    &roster,
                ),
                data::TranscriptKind::Player => vec![VnCell::Narration {
                    text: event.text.clone(),
                }],
                data::TranscriptKind::System => return None,
            };
            Some(VnEventCells {
                event_id: event.id,
                kind: event.kind,
                cells,
            })
        })
        .collect())
}

/// 串流中未落檔的文字→暫定格（同一個解析器、同一份名單；不套刪格規則）。
#[tauri::command]
pub(crate) fn parse_vn_text(
    app: tauri::AppHandle,
    world_id: String,
    source: VnSource,
    text: String,
) -> Result<VnText, String> {
    let roster = VnRoster::load(&data_root(&app)?, &world_id)?;
    Ok(chat_turn::parse_text(&text, &source, &roster))
}

/// 換一組：只重生成最後一則 GM 旁白的選項區塊（流程見 `chat_turn::reroll_choices`）。
#[tauri::command]
pub(crate) async fn vn_reroll_choices(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<String>, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let tier = transport::gm_tier(&config);
    let path = crate::scene_budget::path_limits(&config_root(&app)?, &root, &config, tier);
    let mut caller = RerollCaller {
        app: &app,
        config: &config,
        world_id: &world_id,
        lang: &lang,
    };
    chat_turn::reroll_choices(&root, &world_id, &lang, &path, &mut caller).await
}

/// 換一組的那一下：GM 檔單發（同換幕摘要的一次性路徑），截斷算失敗。
struct RerollCaller<'a> {
    app: &'a tauri::AppHandle,
    config: &'a data::AppConfig,
    world_id: &'a str,
    lang: &'a str,
}

impl crate::scene_budget::summarize::SummaryCaller for RerollCaller<'_> {
    async fn call(&mut self, messages: Vec<transport::ChatMessage>) -> Result<String, String> {
        let (reply, truncated) = transport::dispatch::stream_turn_reporting_truncation(
            self.app,
            self.config,
            None,
            false,
            transport::gm_tier(self.config),
            Some(self.world_id),
            None,
            "GM",
            transport::vn_reroll_closing(self.lang),
            &messages,
            crate::usage::log::PromptShape::Oneshot,
            false,
            |_| {},
        )
        .await?;
        match truncated {
            true => Err("AI_INCOMPLETE_RESPONSE: choices reroll truncated".to_owned()),
            false => Ok(reply),
        }
    }
}
