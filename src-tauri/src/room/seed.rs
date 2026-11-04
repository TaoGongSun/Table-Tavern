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
    // (卡 id, 自動隱藏)：名單本身不含玩家卡（單人時期的玩家卡、入座後的真人玩家卡），那幾張另外補上——
    // 不補的話名冊一重送，已入座的卡就從名冊消失（伺服器照「離座」處理），房主的玩家卡也沒法認領
    let mut entries: Vec<(String, bool)> = data::list_characters(root, world)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|meta| !meta.archived)
        .map(|meta| (meta.id, meta.auto_hidden))
        .collect();
    let players = state
        .player_card_id
        .iter()
        .chain(&state.player_card_ids)
        .chain(&state.solo_player_card_id);
    for id in players {
        if !entries.iter().any(|(known, _)| known == id) {
            entries.push((id.clone(), false));
        }
    }
    let mut cards: Vec<Value> = entries
        .into_iter()
        .filter(|(id, _)| valid_id(id))
        .filter_map(|(id, auto_hidden)| {
            let card = data::read_character(root, world, &id).ok()?;
            let name = clip(card.name.trim(), 64);
            let hidden = auto_hidden
                && !appeared
                    .iter()
                    .any(|seen| data::name_matches(seen, &card.name));
            let color = (card.color.len() == 7
                && card.color.starts_with('#')
                && card.color[1..].chars().all(|c| c.is_ascii_hexdigit()))
            .then_some(card.color.clone());
            (!name.is_empty() && !card.archived).then(|| {
                json!({
                    "cardId": id, "name": name, "color": color,
                    "avatar": avatar(root, world, &id), "blurb": clip(card.public_md.trim(), 1000),
                    "playable": !card.stage, "hidden": hidden,
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

/// 電腦自己的玩家卡（進房前那張；房間期間存在 `solo_player_card_id`）：保留給電腦那台裝置（協定外加 `seat.reserve`）；
/// 沒有這張卡就不送
pub(super) fn reserve_frame(root: &Path, world: &str, device_id: &str) -> Option<Value> {
    let state = data::read_state(root, world).ok()?;
    let card = state
        .solo_player_card_id
        .or(if state.player_card_ids.is_empty() {
            state.player_card_id
        } else {
            None
        });
    let card = card.filter(|id| valid_id(id))?;
    Some(json!({ "t": "seat.reserve", "p": { "cardId": card, "deviceId": device_id } }))
}

pub(super) fn card_ids(cards: &[Value]) -> Vec<String> {
    cards
        .iter()
        .filter_map(|card| card["cardId"].as_str().map(str::to_owned))
        .collect()
}

/// 某一幕的公開投影（正序）：旁白、角色對白、玩家句；GM 專屬與空白略過。開頭的幕摘要（上一幕的）另外回，
/// 不當一般段落。說話者對不到名冊就改成 GM 段、正文前加「名字：」——劇情完整比說話者精確重要。
/// 玩家句的 `memberId` 照座位（卡 → 成員）填，查不到留空。
pub(super) struct Scene {
    pub(super) summary: Option<String>,
    pub(super) lines: Vec<Value>,
    /// 第一則不是開場白的段落在 `lines` 裡的位置；有上一幕摘要就是 0（整幕都是玩出來的）。沒有＝這桌沒玩過
    pub(super) played_from: Option<usize>,
}

pub(super) fn scene(
    root: &Path,
    world: &str,
    scene: u64,
    cards: &[Value],
    seats: &std::collections::HashMap<String, String>,
) -> Scene {
    let events = data::read_transcript(root, world, scene).unwrap_or_default();
    let card_of = |id: &str, name: &str| {
        cards
            .iter()
            .find(|card| !id.is_empty() && card["cardId"] == id)
            .or_else(|| cards.iter().find(|card| card["name"] == name))
    };
    let mut out = Scene {
        summary: None,
        lines: Vec::new(),
        played_from: None,
    };
    for event in &events {
        if event.gm_only || event.text.trim().is_empty() {
            continue;
        }
        let card = card_of(&event.speaker_id, &event.speaker_name);
        let (speaker, text) = match (&event.kind, &event.marker, card) {
            (TranscriptKind::Narration, _, _) => (json!({ "kind": "gm" }), event.text.clone()),
            (TranscriptKind::Dialogue, _, Some(card)) => (
                json!({ "kind": "npc", "cardId": card["cardId"], "name": card["name"] }),
                event.text.clone(),
            ),
            (TranscriptKind::Player, _, Some(card)) => {
                let member = seats
                    .get(card["cardId"].as_str().unwrap_or_default())
                    .cloned()
                    .unwrap_or_default();
                (
                    json!({ "kind": "player", "memberId": member, "cardId": card["cardId"], "name": card["name"] }),
                    event.text.clone(),
                )
            }
            (TranscriptKind::Dialogue | TranscriptKind::Player, _, None) => (
                json!({ "kind": "gm" }),
                format!("{}：{}", event.speaker_name.trim(), event.text),
            ),
            (TranscriptKind::System, Some(EventMarker::SceneSummary), _) => {
                out.played_from = Some(0);
                if out.lines.is_empty() && out.summary.is_none() {
                    out.summary = Some(clip(&event.text, 64000));
                }
                continue;
            }
            _ => continue,
        };
        if !event.opening && out.played_from.is_none() {
            out.played_from = Some(out.lines.len());
        }
        out.lines.push(json!({
            "speaker": speaker, "text": clip(&text, 64000), "interrupted": event.truncated, "at": 0,
        }));
    }
    out
}

/// 系統分隔段（房主單人玩的段落前後）
pub(super) fn marker(name: &str) -> Value {
    json!({ "speaker": { "kind": "system", "marker": name }, "text": "", "interrupted": false, "at": 0 })
}

/// 種子的公開歷史：本幕開頭的前情提要＋段落，從最新往回裝到 `budget` 位元組為止；這桌玩過就在第一個玩出來的
/// 段落前放「房主之前自己玩的」分隔（開場白在分隔前；有前情提要或開頭被預算切掉就放最前面）
fn history(root: &Path, world: &str, cards: &[Value], budget: usize) -> Vec<Value> {
    let Ok(state) = data::read_state(root, world) else {
        return Vec::new();
    };
    let found = scene(root, world, state.current_scene, cards, &Default::default());
    let lead = usize::from(found.summary.is_some());
    let mut items: Vec<Value> = found
        .summary
        .map(|text| {
            json!({ "speaker": { "kind": "system", "marker": "scene_summary" }, "text": text, "interrupted": false, "at": 0 })
        })
        .into_iter()
        .chain(found.lines)
        .collect();
    let mut size = 0;
    let cut = items
        .iter()
        .rposition(|item| {
            size += item.to_string().len();
            size > budget
        })
        .map_or(0, |i| i + 1);
    let mut out = items.split_off(cut);
    if let Some(from) = found.played_from.filter(|_| !out.is_empty()) {
        let at = if lead > 0 {
            0
        } else {
            from.saturating_sub(cut)
        };
        out.insert(at.min(out.len()), marker("solo_before"));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    for item in &mut out {
        item["at"] = json!(now);
    }
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
