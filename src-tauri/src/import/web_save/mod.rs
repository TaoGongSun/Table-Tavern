//! 匯入網頁存檔（桌檔契約 v1，src/shared/contracts/web-save/web-save.md；計畫 web-version 二之二「落地規則」）：
//! 一律開新桌。整份先驗完才動資料；桌內全部寫好後才補跨桌層（D18 只補缺），任何一步失敗就退回已補的
//! 跨桌鍵、刪掉新桌，不留半桌。成功後新桌留著未確認記錄（pending.rs），前端寫好卡片 storage 才確認；
//! 放棄就照記錄撤回跨桌層再刪桌。
mod parse;
mod pending;

pub use pending::{confirm_web_save_import, discard_web_save_import};

use super::card::{import_character_placing, BookWrite};
use crate::data::card_vars::{self, Filled, Layer, LayerWrite};
use crate::data::message_vars::{self, Json};
use crate::data::state_commit::with_commit;
use crate::data::{
    self, CharacterCard, DataResult, ImportedEvent, ImportedScene, Tier, TranscriptEvent,
    TranscriptKind,
};
use crate::ui_msg::UiMsg;
use parse::{Role, Route, WebSave};
use serde_json::Value;
use std::path::Path;

/// 匯入結果：新桌、角色（世界書路沒有）、要交回前端寫進卡片 storage 的內容，與給玩家的提示素材。
#[derive(Debug, serde::Serialize)]
pub struct WebSaveImported {
    pub world_id: String,
    pub character_id: Option<String>,
    pub card_storage: serde_json::Map<String, Value>,
    /// 新桌世界書裡來自這張卡的條目數（>0 就提示桌面版觸發規則與網頁版不同）
    pub worldbook_entries: usize,
    /// 跨桌層因為桌面版已有同名鍵而沒寫進去的鍵數（D18，存檔裡的值仍在）
    pub shared_kept: usize,
}

const PLAYER_COLOR: &str = "#3d84a8";
const CHARACTER_COLOR: &str = "#e07a5f";

pub fn import_web_save(root: &Path, bytes: &[u8], lang: &str) -> DataResult<WebSaveImported> {
    import_with(root, bytes, lang, &mut |_, _| Ok(()))
}

/// 測試掛點：每補完一個跨桌層就呼叫一次（第幾個、補了什麼），回錯＝模擬之後的步驟失敗。
pub(super) fn import_with(
    root: &Path,
    bytes: &[u8],
    lang: &str,
    after_shared: &mut dyn FnMut(usize, &Filled) -> DataResult<()>,
) -> DataResult<WebSaveImported> {
    let save = parse::parse(bytes)?;
    let card_bytes = card_bytes(&save)?;
    // 照存檔記的身分驗卡：那條路桌面版會拒收就整份拒收，什麼都還沒建
    match save.route {
        Route::Character => super::check_character_bytes(&card_bytes)?,
        Route::Worldbook => {
            super::worldbook_json(&card_bytes)?;
        }
    }
    let name = card_name(&save.card);
    let table_name = if name.is_empty() {
        "Web"
    } else {
        name.as_str()
    };
    let (world_id, held) = data::create_world_exclusive(root, table_name).map_err(|failed| {
        cleanup_error(
            failed.error,
            failed
                .leftover
                .map(|id| format!("world:{id}"))
                .into_iter()
                .collect(),
        )
    })?;
    let mut journal = Vec::new();
    let result = pending::write(root, &world_id, &[]).and_then(|()| {
        build(
            root,
            &world_id,
            &save,
            &card_bytes,
            &name,
            lang,
            &mut journal,
            after_shared,
        )
    });
    match result {
        Ok(imported) => Ok(imported),
        Err(error) => {
            // 先退跨桌層（新桌刪不掉也要退），再收掉新桌；退不掉、刪不掉的都列進回報，不只報原錯
            let mut leftovers = pending::retract_all(root, &world_id, &journal);
            if data::discard_new_world(root, &world_id, &held).is_err() {
                leftovers.push(format!("world:{world_id}"));
            }
            Err(cleanup_error(error, leftovers))
        }
    }
}

/// 清理做完就回原錯；有留下東西就換成 `WebSaveCleanupIncomplete`，帶原錯與殘留位置。
pub(super) fn cleanup_error(
    error: Box<dyn std::error::Error + Send + Sync>,
    leftovers: Vec<String>,
) -> Box<dyn std::error::Error + Send + Sync> {
    if leftovers.is_empty() {
        return error;
    }
    UiMsg::WebSaveCleanupIncomplete {
        error: error.to_string(),
        leftovers: leftovers.join(", "),
    }
    .into_error()
}

fn card_name(card: &Value) -> String {
    card.get("data")
        .filter(|data| data.is_object())
        .unwrap_or(card)
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// 用哪份檔匯卡：PNG 讀出的卡與 `card` 完全相同才用 PNG（帶得到卡圖），否則以 `card` 為準。
fn card_bytes(save: &WebSave) -> DataResult<Vec<u8>> {
    if let Some(png) = &save.card_png {
        // `Value` 相等不看物件鍵順序，而物件形條目的順序決定 ST 載入先後，所以順序也要一致
        let same = super::card_io::decode_png_character(png)
            .ok()
            .is_some_and(|json| {
                serde_json::from_slice::<Value>(&json).is_ok_and(|decoded| decoded == save.card)
                    && super::card::book_object_key_order(&json)
                        == super::card::book_object_key_order(save.card_text.as_bytes())
            });
        if same {
            return Ok(png.clone());
        }
    }
    Ok(save.card_text.clone().into_bytes())
}

#[allow(clippy::too_many_arguments)]
fn build(
    root: &Path,
    world_id: &str,
    save: &WebSave,
    card_bytes: &[u8],
    name: &str,
    lang: &str,
    journal: &mut Vec<Filled>,
    after_shared: &mut dyn FnMut(usize, &Filled) -> DataResult<()>,
) -> DataResult<WebSaveImported> {
    if !save.regex_allowed {
        data::update_state(root, world_id, |state| {
            state.regex_allowed = false;
            Ok(Some(()))
        })?;
    }
    if !save.user_name.trim().is_empty() {
        let player = CharacterCard {
            id: data::new_id(),
            name: save.user_name.trim().to_owned(),
            color: PLAYER_COLOR.to_owned(),
            avatar: "🙂".to_owned(),
            tier: Tier::Balanced,
            show_image: true,
            archived: false,
            gen_prompt: String::new(),
            public_md: String::new(),
            private_md: String::new(),
        };
        data::write_character(root, world_id, &player)?;
        data::set_player_card(root, world_id, Some(player.id))?;
    }

    // 卡：角色卡路的隨身條目沒指定可見度就只給這個角色看（D16，與桌面版同一條規則）；世界書路由 GM 演，給 GM
    let (character_id, placed) = match save.route {
        Route::Character => {
            let imported = import_character_placing(
                root,
                world_id,
                card_bytes,
                CHARACTER_COLOR,
                lang,
                BookWrite::Strict,
            )?;
            (
                Some(imported.meta.id),
                imported.book.map(|book| book.placed).unwrap_or_default(),
            )
        }
        Route::Worldbook => {
            let json_text = super::worldbook_json(card_bytes)?;
            let book = data::import_worldbook_as(root, world_id, &json_text, &data::BookOwner::Gm)?;
            super::interface::save_world_card_strict(root, world_id, card_bytes)?;
            super::save_gm_image(root, world_id, card_bytes)?;
            if let Ok(book_value) = serde_json::from_str(&json_text) {
                super::mechanism::import_mechanism_strict(root, world_id, &book_value)?;
            }
            super::mechanism::import_card_extension_strict(root, world_id, name, card_bytes)?;
            (None, book.placed)
        }
    };

    // 逐字稿：整幕一次寫入，控制檔同一把鎖發布
    let scene = data::read_state(root, world_id)?.current_scene;
    let events = save
        .messages
        .iter()
        .map(|message| ImportedEvent {
            event: event_of(message, save, character_id.as_deref(), name),
            table: message.table.clone(),
        })
        .collect();
    let event_ids = data::write_imported_scene(
        root,
        world_id,
        scene,
        ImportedScene {
            macros: save.mvu.as_ref().and_then(|mvu| mvu.macros.clone()),
            seed: save.mvu.as_ref().and_then(|mvu| mvu.seed.clone()),
            events,
        },
    )?;

    // 桌內三層
    if let Some(mvu) = &save.mvu {
        write_world_layer(root, world_id, Layer::Chat, None, &mvu.layers.chat)?;
        if let Some(id) = &character_id {
            write_world_layer(
                root,
                world_id,
                Layer::Character,
                Some(id),
                &mvu.layers.character,
            )?;
        }
        for (id, table) in &mvu.layers.script {
            write_world_layer(root, world_id, Layer::Script, Some(id), table)?;
        }
    }

    write_sidecar(root, world_id, save, &placed, &event_ids)?;
    write_world_info_timed(
        root,
        world_id,
        scene,
        save,
        &placed,
        character_id.as_deref(),
    )?;

    // 跨桌層最後補：只補缺，補了什麼記進 journal，之後任何一步失敗由呼叫端退回。每層真的要寫之前先把
    // 「已補＋這一層」落進未確認記錄，崩潰或放棄時記錄一定涵蓋已落地的補缺
    let mut shared_kept = 0;
    if let Some(mvu) = &save.mvu {
        let mut shared: Vec<(Layer, Option<&str>, &Json)> = vec![
            (Layer::Global, None, &mvu.layers.global),
            (Layer::Preset, None, &mvu.layers.preset),
        ];
        shared.extend(
            mvu.layers
                .extension
                .iter()
                .map(|(id, table)| (Layer::Extension, Some(id.as_str()), table)),
        );
        for (layer, id, table) in shared {
            let incoming = table.as_object().map_or(0, Vec::len);
            if incoming == 0 {
                continue;
            }
            let filled = card_vars::fill_missing(root, world_id, layer, id, table, &mut |next| {
                let mut recorded = journal.clone();
                recorded.push(next.clone());
                pending::write(root, world_id, &recorded)
            })?;
            shared_kept += incoming - filled.added.len();
            journal.push(filled);
            after_shared(journal.len(), journal.last().expect("剛放進去"))?;
        }
    }

    Ok(WebSaveImported {
        world_id: world_id.to_owned(),
        character_id,
        card_storage: save.card_storage.clone(),
        // 去重併掉或 `id` 被蓋掉的條目映到留下那條的 uid，不重複算
        worldbook_entries: placed
            .iter()
            .filter_map(|(_, uid)| *uid)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        shared_kept,
    })
}

fn event_of(
    message: &parse::Message,
    save: &WebSave,
    character_id: Option<&str>,
    name: &str,
) -> TranscriptEvent {
    let (kind, speaker_id, speaker_name) = match (message.role, character_id) {
        (Role::User, _) => (
            TranscriptKind::Player,
            String::new(),
            save.user_name.trim().to_owned(),
        ),
        // 開場白照桌面版貼開場的寫法：GM 旁白
        (Role::Char, _) if message.opening => {
            (TranscriptKind::Narration, String::new(), "GM".to_owned())
        }
        (Role::Char, Some(id)) => (TranscriptKind::Dialogue, id.to_owned(), name.to_owned()),
        (Role::Char, None) => (TranscriptKind::Narration, String::new(), "GM".to_owned()),
    };
    // 網頁訊息照 ST 存原文（`<UpdateVariable>`、MVU 佔位都在 text 裡）；桌面版畫面與送模讀 text，
    // 所以模型回的樓照桌面版的收尾剝乾淨，原文留在 raw 給卡片介面（中斷的樓照中止規則切尾巴）
    let (text, raw) = match message.role {
        Role::Char => {
            let text = crate::transport::final_reply_text(&message.text, message.interrupted);
            let raw = message.raw.clone().unwrap_or_else(|| message.text.clone());
            let raw = (raw != text).then_some(raw);
            (text, raw)
        }
        Role::User => (
            message.text.clone(),
            message.raw.clone().filter(|raw| raw != &message.text),
        ),
    };
    TranscriptEvent {
        ts: message.ts.clone(),
        speaker_id,
        speaker_name,
        kind,
        text,
        raw,
        state: None,
        truncated: message.interrupted,
        gm_only: false,
        marker: None,
        opening: message.opening,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    }
}

/// 桌內層整張寫入（新桌，沒有既有版本）。空表不建檔。
fn write_world_layer(
    root: &Path,
    world_id: &str,
    layer: Layer,
    id: Option<&str>,
    table: &Json,
) -> DataResult<()> {
    if table.as_object().is_some_and(Vec::is_empty) {
        return Ok(());
    }
    let generation = with_commit(root, world_id, message_vars::generation);
    match card_vars::write_layer(
        root,
        world_id,
        layer,
        id,
        generation,
        None,
        &table.to_text(),
    )? {
        LayerWrite::LayerOk { .. } => Ok(()),
        other => Err(data::invalid_data(format!(
            "web-save: 卡片變數層寫入被拒：{other:?}"
        ))),
    }
}

/// 世界書觸發狀態轉成這一幕的計時檔（worldbook-st-trigger-parity 三之 4）：交給演這張卡的人（P1）——
/// 角色卡路給該角色、世界書路給 GM；穩定 ID 經條目 key 換成桌面 uid，對不到的丟掉。
fn write_world_info_timed(
    root: &Path,
    world_id: &str,
    scene: u64,
    save: &WebSave,
    placed: &[(String, Option<u64>)],
    character_id: Option<&str>,
) -> DataResult<()> {
    use crate::data::world_info_store::{import_web_timed, Perspective, WebTimed};
    let uid_of = |stable: &str| {
        let key = save
            .world_info
            .entries
            .iter()
            .find(|(id, _)| id == stable)
            .map(|(_, key)| key)?;
        placed
            .iter()
            .find(|(placed_key, _)| placed_key == key)
            .and_then(|(_, uid)| *uid)
    };
    let effects: Vec<WebTimed> = save
        .world_info
        .timed
        .iter()
        .filter_map(|(cooldown, stable, value)| {
            Some(WebTimed {
                cooldown: *cooldown,
                uid: uid_of(stable)?,
                start: value.start,
                end: value.end,
                protected: value.protected,
            })
        })
        .collect();
    let perspective = match character_id {
        Some(id) => Perspective::Character(id.to_owned()),
        None => Perspective::Gm,
    };
    import_web_timed(root, world_id, scene, &perspective, &effects)
}

/// 旁檔內容；`world_info` 是存檔裡那一段的原文，照字寫回。
#[derive(serde::Serialize)]
struct Sidecar<'a> {
    contract_version: u64,
    regex_allowed: bool,
    opening_index: Option<u64>,
    entry_uids: serde_json::Map<String, Value>,
    message_ids: serde_json::Map<String, Value>,
    world_info: &'a serde_json::value::RawValue,
    /// 世界書路沒有角色：存檔的 character 層不落地，原樣留在這裡（契約：以存檔／旁檔仍在為準）
    #[serde(skip_serializing_if = "Option::is_none")]
    character_layer: Option<&'a Json>,
}

/// 旁檔：世界書觸發狀態原樣（計時另轉成第 0 幕的計時檔）＋兩張映射表＋regex 允許與開場白序號。
fn write_sidecar(
    root: &Path,
    world_id: &str,
    save: &WebSave,
    placed: &[(String, Option<u64>)],
    event_ids: &[String],
) -> DataResult<()> {
    let entry_uids: serde_json::Map<String, Value> = save
        .world_info
        .entries
        .iter()
        .map(|(stable, key)| {
            let uid = placed
                .iter()
                .find(|(placed_key, _)| placed_key == key)
                .and_then(|(_, uid)| *uid);
            (stable.clone(), uid.map_or(Value::Null, Value::from))
        })
        .collect();
    let message_ids: serde_json::Map<String, Value> = save
        .messages
        .iter()
        .zip(event_ids)
        .map(|(message, event_id)| (message.id.clone(), Value::from(event_id.clone())))
        .collect();
    let sidecar = Sidecar {
        contract_version: parse::VERSION,
        regex_allowed: save.regex_allowed,
        opening_index: save.opening_index,
        entry_uids,
        message_ids,
        world_info: &save.world_info.raw,
        character_layer: match (save.route, &save.mvu) {
            (Route::Worldbook, Some(mvu))
                if mvu
                    .layers
                    .character
                    .as_object()
                    .is_some_and(|keys| !keys.is_empty()) =>
            {
                Some(&mvu.layers.character)
            }
            _ => None,
        },
    };
    data::commit_world_write(
        &data::web_save_sidecar_path(root, world_id)?,
        serde_json::to_string_pretty(&sidecar)?.as_bytes(),
    )
}

#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod next_turn_tests;
#[cfg(test)]
mod pending_tests;
#[cfg(test)]
mod strip_tests;
#[cfg(test)]
mod tests;
