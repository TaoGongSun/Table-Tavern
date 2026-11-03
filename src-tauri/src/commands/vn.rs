//! VN 桌（vn-mode）的 Tauri 邊界：接參數、轉呼叫 `chat_turn` 的劇本解析與換一組。
use crate::chat_turn::{self, VnEventCells, VnRoster, VnSource, VnText};
use crate::{config_root, data, data_root, transport};

/// 一幕的正式格（背景已補好沿用時段，見 `chat_turn::read_scene`）。
#[tauri::command]
pub(crate) fn read_vn_cells(
    app: tauri::AppHandle,
    world_id: String,
    scene: u64,
) -> Result<Vec<VnEventCells>, String> {
    Ok(chat_turn::read_scene(&data_root(&app)?, &world_id, scene)?.0)
}

/// 結局一覽與完結畫面的已達成 x／y（包 5 §2.7）。
#[tauri::command]
pub(crate) fn vn_endings(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<crate::vn_bible::EndingsOverview, String> {
    crate::vn_bible::endings_overview(&data_root(&app)?, &world_id)
}

/// 從某一章重玩（包 5 §2.9）：章頭幕截幕首分岔成新的一幕，回新幕號。
#[tauri::command]
pub(crate) fn vn_replay_chapter(
    app: tauri::AppHandle,
    world_id: String,
    chapter: u32,
) -> Result<u64, String> {
    let _permit = data::world_write_permit(&world_id)?;
    let root = data_root(&app)?;
    let scene =
        data::replay_chapter(&root, &world_id, chapter).map_err(|error| error.to_string())?;
    crate::world_scan::macros::forget_outlets(&world_id);
    Ok(scene)
}

/// 開新周目（包 5 §2.10）：原桌握寫入許可只讀，建好回新桌 id。
#[tauri::command]
pub(crate) async fn vn_new_playthrough(
    app: tauri::AppHandle,
    world_id: String,
    name_template: String,
) -> Result<String, String> {
    let _permit = data::world_write_permit_async(&world_id).await?;
    data::vn_playthrough::new_playthrough(&data_root(&app)?, &world_id, &name_template)
        .map_err(|error| error.to_string())
}

/// 串流中未落檔的文字→暫定格（同一個解析器、同一份名單；不套刪格規則）。
#[tauri::command]
pub(crate) fn parse_vn_text(
    app: tauri::AppHandle,
    world_id: String,
    source: VnSource,
    text: String,
) -> Result<VnText, String> {
    let roster = {
        let root = data_root(&app)?;
        VnRoster::load(
            &root,
            &world_id,
            data::vn::read_card(&root, &world_id).as_ref(),
        )?
    };
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
    data::vn::refuse_if_ended(&root, &world_id)?;
    let tier = transport::gm_tier(&config);
    let path = crate::scene_budget::path_limits(&config_root(&app)?, &root, &config, tier);
    let mut caller = GmOneshot {
        app: &app,
        config: &config,
        world_id: &world_id,
        system: transport::vn_reroll_closing(&lang),
        what: "choices reroll",
    };
    chat_turn::reroll_choices(&root, &world_id, &lang, &path, &mut caller).await
}

/// GM 檔單發（同換幕摘要的一次性路徑；換一組、聖經生成與秘密重骰共用），截斷算失敗。
pub(crate) struct GmOneshot<'a> {
    pub app: &'a tauri::AppHandle,
    pub config: &'a data::AppConfig,
    pub world_id: &'a str,
    pub system: &'a str,
    /// 截斷錯誤裡標明是哪一種呼叫
    pub what: &'static str,
}

impl crate::scene_budget::summarize::SummaryCaller for GmOneshot<'_> {
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
            self.system,
            &messages,
            crate::usage::log::PromptShape::Oneshot,
            false,
            |_| {},
        )
        .await?;
        match truncated {
            true => Err(format!("AI_INCOMPLETE_RESPONSE: {} truncated", self.what)),
            false => Ok(reply),
        }
    }
}
