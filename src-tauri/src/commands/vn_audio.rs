//! VN 音訊的 Tauri 邊界：把缺的曲子排進下載、串流中提早排、讀已落地的檔。
use crate::{chat_turn, vn_audio};

#[tauri::command]
pub(crate) fn vn_audio_fill(
    app: tauri::AppHandle,
    world_id: String,
    scene: Option<u64>,
) -> Result<(), String> {
    vn_audio::fill(&app, &world_id, scene)
}

/// 串流中寫完的 `[bgm]`／`[sfx]` 行一出現就排（只看換行結尾的行）
#[tauri::command]
pub(crate) fn vn_audio_prefetch(
    app: tauri::AppHandle,
    world_id: String,
    source: chat_turn::VnSource,
    text: String,
) -> Result<(), String> {
    vn_audio::prefetch(&app, &world_id, &source, &text)
}

#[tauri::command]
pub(crate) fn vn_audio_read(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<vn_audio::AudioView, String> {
    vn_audio::read(&app, &world_id)
}
