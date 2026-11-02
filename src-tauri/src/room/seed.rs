//! 房間的公開投影（協定 §4.4、§6.2、§10）：名冊、公開歷史、capabilities。只送公開欄位——私設、世界書、
//! GM 專屬事件、原文與變數表一律不出這台電腦。
use crate::data::{self, AppConfig, EventMarker, TranscriptKind};
use crate::transport;
use base64::Engine as _;
use serde_json::{json, Value};
use std::path::Path;

const MAX_CARDS: usize = 64;
/// 每片 ≤ 512 KiB、整份 ≤ 1 MiB（協定 §6.2、§14-3），各留一點餘裕
const PART_BYTES: usize = 480 * 1024;
const SEED_BYTES: usize = 900 * 1024;
const AVATAR_BYTES: usize = 16 * 1024;
const AVATAR_PX: u32 = 96;

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// 頭像壓成 96px JPEG 的 data URL；沒有頭像圖、解不開或壓完仍太大就不送
fn avatar(root: &Path, world: &str, card: &str) -> Option<String> {
    let path = data::character_path(root, world, card)
        .ok()?
        .with_extension("avatar.png");
    let image = image::load_from_memory(&std::fs::read(path).ok()?).ok()?;
    let mut jpeg = Vec::new();
    image
        .thumbnail(AVATAR_PX, AVATAR_PX)
        .to_rgb8()
        .write_to(
            &mut std::io::Cursor::new(&mut jpeg),
            image::ImageFormat::Jpeg,
        )
        .ok()?;
    let url = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    );
    (url.len() <= AVATAR_BYTES).then_some(url)
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 公開名冊：沒封存的卡全列（說話者必須在名冊裡）；舞台卡不可認領；本幕自動隱藏、還沒回歸的卡標 `hidden`，
/// 伺服器擋認領（選角時看不到 GM 設定的卡）、回歸時由 `gen.done.cardsArrived` 翻開。整份壓在一片之內：
/// 太大就從後面的卡開始拿掉頭像
pub(super) fn roster(root: &Path, world: &str) -> Result<Vec<Value>, String> {
    let state = data::read_state(root, world).map_err(|error| error.to_string())?;
    let events = data::read_transcript(root, world, state.current_scene)
        .map_err(|error| error.to_string())?;
    let appeared = data::appeared_card_names(&events);
    let metas = data::list_characters(root, world).map_err(|error| error.to_string())?;
    let mut cards: Vec<Value> = metas
        .into_iter()
        .filter(|meta| !meta.archived && valid_id(&meta.id))
        .filter_map(|meta| {
            let card = data::read_character(root, world, &meta.id).ok()?;
            let name = clip(card.name.trim(), 64);
            let hidden = meta.auto_hidden
                && !appeared
                    .iter()
                    .any(|seen| data::name_matches(seen, &meta.name));
            let color = (meta.color.len() == 7
                && meta.color.starts_with('#')
                && meta.color[1..].chars().all(|c| c.is_ascii_hexdigit()))
            .then_some(meta.color.clone());
            (!name.is_empty()).then(|| {
                json!({
                    "cardId": meta.id, "name": name, "color": color,
                    "avatar": avatar(root, world, &meta.id), "blurb": clip(card.public_md.trim(), 1000),
                    "playable": !meta.stage, "hidden": hidden,
                })
            })
        })
        .take(MAX_CARDS)
        .collect();
    for index in (0..cards.len()).rev() {
        if Value::Array(cards.clone()).to_string().len() <= PART_BYTES {
            break;
        }
        cards[index]["avatar"] = Value::Null;
    }
    Ok(cards)
}

pub(super) fn card_ids(cards: &[Value]) -> Vec<String> {
    cards
        .iter()
        .filter_map(|card| card["cardId"].as_str().map(str::to_owned))
        .collect()
}

/// 公開歷史投影：本幕的旁白、角色對白、玩家句與前情提要，從最新往回裝到 `budget` 位元組為止
fn history(root: &Path, world: &str, cards: &[Value], budget: usize) -> Vec<Value> {
    let Ok(state) = data::read_state(root, world) else {
        return Vec::new();
    };
    let events = data::read_transcript(root, world, state.current_scene).unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    let card_named = |name: &str| cards.iter().find(|card| card["name"] == name);
    let mut out = Vec::new();
    let mut size = 0;
    for event in events.iter().rev() {
        if event.gm_only || event.text.trim().is_empty() {
            continue;
        }
        let speaker = match (&event.kind, &event.marker) {
            (TranscriptKind::Narration, _) => json!({ "kind": "gm" }),
            (TranscriptKind::Dialogue, _) => match card_named(&event.speaker_name) {
                Some(card) => {
                    json!({ "kind": "npc", "cardId": card["cardId"], "name": card["name"] })
                }
                None => continue,
            },
            // 單人時期的玩家句：還沒有房間成員，memberId 留空
            (TranscriptKind::Player, _) => match card_named(&event.speaker_name) {
                Some(card) => {
                    json!({ "kind": "player", "memberId": "", "cardId": card["cardId"], "name": card["name"] })
                }
                None => continue,
            },
            (TranscriptKind::System, Some(EventMarker::SceneSummary)) => {
                json!({ "kind": "system", "marker": "scene_summary" })
            }
            _ => continue,
        };
        let item = json!({
            "speaker": speaker, "text": clip(&event.text, 64000), "interrupted": event.truncated, "at": now,
        });
        size += item.to_string().len();
        if size > budget {
            break;
        }
        out.push(item);
    }
    out.reverse();
    out
}

/// `room.seed` 分片（協定 §6.2）：第 0 片帶名冊，歷史依序往後裝；id＝`seed-<片>-<總片數>`。另回名冊卡 id
pub(super) fn seed_frames(root: &Path, world: &str) -> Result<(Vec<Value>, Vec<String>), String> {
    let cards = roster(root, world)?;
    let ids = card_ids(&cards);
    let cards_size = Value::Array(cards.clone()).to_string().len();
    let mut parts: Vec<Vec<Value>> = vec![Vec::new()];
    let mut size = cards_size;
    for item in history(root, world, &cards, SEED_BYTES.saturating_sub(cards_size)) {
        let n = item.to_string().len();
        if size + n > PART_BYTES {
            parts.push(Vec::new());
            size = 0;
        }
        size += n;
        parts.last_mut().expect("至少一片").push(item);
    }
    let total = parts.len();
    let mut cards = Some(cards);
    let frames = parts
        .into_iter()
        .enumerate()
        .map(|(part, items)| {
            let mut p = json!({ "part": part, "total": total });
            if let Some(cards) = cards.take() {
                p["cards"] = json!(cards);
            }
            if !items.is_empty() {
                p["history"] = json!(items);
            }
            json!({ "t": "room.seed", "id": format!("seed-{part}-{total}"), "p": p })
        })
        .collect();
    Ok((frames, ids))
}

/// `capabilities`（協定 §6.2、§7.3）：目前路線，加上有金鑰的 API 路線（OpenRouter `api`、矽基流動
/// `siliconflow`）；模型＝各檔位設定的模型（去重），矽基流動再補上整份免費清單；
/// 沒有可選模型的路線給 `default`（照該路線自己的預設）。
pub(super) fn capabilities(config: &AppConfig) -> Value {
    let siliconflow = transport::siliconflow::active(config);
    let raw = transport::dispatch::chat_transport(config);
    let current = if raw == "api" && siliconflow {
        "siliconflow".to_owned()
    } else {
        raw.clone()
    };
    let has_key = |slot: &str| {
        config
            .api_keys
            .get(slot)
            .is_some_and(|key| !key.trim().is_empty())
    };
    let mut routes = vec![current.clone()];
    for (route, slot) in [("api", "openrouter"), ("siliconflow", "siliconflow")] {
        if current != route && has_key(slot) {
            routes.push(route.to_owned());
        }
    }
    // OpenRouter 路線的檔位模型要在「沒選矽基流動」的設定上解析
    let mut openrouter = config.clone();
    if siliconflow {
        openrouter.preferences.remove("base_url");
    }
    let models = |route: &str| {
        let mut list: Vec<String> = Vec::new();
        let mut push = |model: &str| {
            if !model.trim().is_empty() && !list.contains(&clip(model, 200)) {
                list.push(clip(model, 200));
            }
        };
        for tier in [data::Tier::Best, data::Tier::Balanced, data::Tier::Fast] {
            let model = match route {
                "siliconflow" => transport::siliconflow::tier_model(config, tier),
                "api" => transport::tier_model(&openrouter, route, tier)
                    .model
                    .unwrap_or_default(),
                cli => transport::tier_model(config, cli, tier)
                    .model
                    .unwrap_or_default(),
            };
            push(&model);
        }
        if route == "siliconflow" {
            transport::siliconflow::FREE_MODELS
                .iter()
                .for_each(|model| push(model));
        }
        if list.is_empty() {
            list.push("default".to_owned());
        }
        list
    };
    let gm = transport::tier_model(config, &raw, transport::gm_tier(config))
        .model
        .filter(|model| !model.trim().is_empty())
        .map_or_else(|| "default".to_owned(), |model| clip(&model, 200));
    let label = |route: &str| match route {
        "api" => "API".to_owned(),
        "siliconflow" => "SiliconFlow".to_owned(),
        cli => clip(&format!("{cli} CLI"), 64),
    };
    let max = config
        .preferences
        .get("max_round_speakers")
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
        .filter(|max| *max > 0)
        .unwrap_or(3);
    json!({
        "routes": routes.iter().map(|route| json!({
            "route": route, "label": label(route),
            "models": models(route).iter().map(|id| json!({ "id": id, "label": id, "laxity": null })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "current": { "route": current, "model": gm, "modelLabel": gm },
        "maxRoundSpeakers": max,
    })
}
