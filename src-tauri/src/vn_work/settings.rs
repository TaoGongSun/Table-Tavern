//! 作品包附設定（plans/vn-mode-pkg8.md §2.1 `settings/`、§2.4 可續寫、§2.7）：匯出時帶聖經、卡原件、角色設定、
//! 世界書、world.md 與逐則完整快照；匯入成可續寫的桌時每項都走既有驗證並夾斷，條目名只由幕號與固定名組出。
use super::archive::{broken, Pack};
use super::format::{Characters, Limits, Manifest, Script, LIMITS};
use super::validate::{Notice, NoticeKind};
use crate::data::vn::{self, Bible, CardOrigin};
use crate::data::{self, Mechanism, TableState, TranscriptEvent};
use crate::import::vn_card;
use crate::ui_msg::UiMsg;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const VN: &str = "settings/vn.json";
const WORLD: &str = "settings/world.md";
const CHARACTERS: &str = "settings/characters.json";
const WORLDBOOK: &str = "settings/worldbook.json";
/// 角色公開／私設與 world.md 的字元上限：遠寬於 VN 卡上限，作者桌上手改過的設定往返不被截
const MD_CHARS: usize = 20_000;
const WORLD_CHARS: usize = 60_000;
/// 每則快照序列化後的上限（§2.5 #14）
const STATE_BYTES: usize = 256 << 10;

fn history_name(scene: u64) -> String {
    format!("settings/history/{scene}.json")
}

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct VnSettings {
    bible: Option<Value>,
    card: Option<Value>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Md {
    pub public_md: String,
    pub private_md: String,
}

/// 匯出：`settings/` 各檔（包內名稱, 內容）。`scenes` 依重排後幕號給每幕**完整**事件（含分岔複製來的前段）。
/// 超過匯入端上限就整包不出，免得出一個自家收不下的包。
pub(super) fn export_files<'a>(
    root: &Path,
    world_id: &str,
    file: &vn::VnFile,
    scenes: impl Iterator<Item = &'a [TranscriptEvent]>,
    cast: &Characters,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let too_large = || UiMsg::VnWorkTooLarge.to_string();
    let mut files = Vec::new();
    let mut put = |name: String, bytes: Vec<u8>, cap: u64| {
        if bytes.len() as u64 > cap {
            return Err(too_large());
        }
        files.push((name, bytes));
        Ok(())
    };
    let settings = VnSettings {
        bible: file.bible.as_ref().map(|bible| json!(bible)),
        card: file.card.as_ref().map(vn_card::origin_json),
    };
    put(
        VN.to_owned(),
        serde_json::to_vec(&settings).map_err(text)?,
        LIMITS.json_bytes,
    )?;
    let world = data::read_world_md(root, world_id).unwrap_or_default();
    put(WORLD.to_owned(), world.into_bytes(), LIMITS.world_bytes)?;
    // 已刪角色的名牌讀不到設定，略過
    let md: BTreeMap<&str, Md> = cast
        .characters
        .iter()
        .filter_map(|member| {
            let card = data::read_character(root, world_id, &member.id).ok()?;
            Some((
                member.id.as_str(),
                Md {
                    public_md: card.public_md,
                    private_md: card.private_md,
                },
            ))
        })
        .collect();
    put(
        CHARACTERS.to_owned(),
        serde_json::to_vec(&md).map_err(text)?,
        LIMITS.json_bytes,
    )?;
    let entries = vn_card::book_entries(root, world_id).map_err(text)?;
    if !entries.is_empty() {
        let book = json!({ "name": "", "entries": entries, "extensions": {} });
        put(
            WORLDBOOK.to_owned(),
            serde_json::to_vec(&book).map_err(text)?,
            LIMITS.json_bytes,
        )?;
    }
    for (scene, events) in scenes.enumerate() {
        let mut states = Vec::with_capacity(events.len());
        for event in events {
            let state = serde_json::to_string(&event.state).map_err(text)?;
            if state.len() > STATE_BYTES {
                return Err(too_large());
            }
            states.push(state);
        }
        let history = format!("[{}]", states.join(","));
        put(
            history_name(scene as u64),
            history.into_bytes(),
            LIMITS.history_bytes,
        )?;
    }
    Ok(files)
}

/// 匯入時建桌前驗完的設定（逐幕快照另在寫幕時逐檔讀，見 `history`）
pub(super) struct Settings {
    pub bible: Option<Bible>,
    pub card: Option<CardOrigin>,
    pub world: String,
    pub md: BTreeMap<String, Md>,
    pub worldbook: Option<String>,
}

fn clip(text: &mut String, cap: usize, target: &str, notices: &mut Vec<Notice>) {
    if let Some((cut, _)) = text.char_indices().nth(cap) {
        text.truncate(cut);
        notices.push(Notice {
            kind: NoticeKind::Clipped,
            target: target.to_owned(),
        });
    }
}

/// 結構層（建桌前）：包要真的附了設定；聖經過 `parse_bible`＋`normalize`、卡原件過 VN 卡美術欄位驗證、
/// 角色設定只收角色表裡有的 id 並夾斷、每幕的快照檔都在包內。
pub(super) fn read(
    pack: &mut Pack,
    manifest: &Manifest,
    script: &Script,
    cast: &Characters,
    limits: Limits,
    notices: &mut Vec<Notice>,
) -> Result<Settings, String> {
    if !manifest.settings || manifest.vars_mode {
        return Err(broken("no settings".to_owned()));
    }
    let names: Vec<(String, String)> = cast
        .characters
        .iter()
        .map(|member| (member.id.clone(), member.name.clone()))
        .collect();
    let file: VnSettings = pack.json(VN, limits.json_bytes)?;
    let mut card_notices = Vec::new();
    let card = file
        .card
        .as_ref()
        .map(|card| vn_card::origin_from(card, &names, &mut card_notices));
    notices.extend(card_notices.into_iter().map(|notice| Notice {
        kind: match notice.kind {
            vn_card::CardNoticeKind::Clipped => NoticeKind::Clipped,
            _ => NoticeKind::Dropped,
        },
        target: notice.target,
    }));
    let bible = file
        .bible
        .as_ref()
        .and_then(vn_card::bible_from)
        .map(|bible| vn::normalize(bible, &names));
    let mut world = match pack.read(WORLD, limits.world_bytes)? {
        Some(bytes) => String::from_utf8(bytes).map_err(|_| broken(WORLD.to_owned()))?,
        None => String::new(),
    };
    clip(&mut world, WORLD_CHARS, "world.md", notices);
    let mut md: BTreeMap<String, Md> = pack.json(CHARACTERS, limits.json_bytes)?;
    md.retain(|id, _| names.iter().any(|(known, _)| known == id));
    for (id, card) in &mut md {
        clip(&mut card.public_md, MD_CHARS, id, notices);
        clip(&mut card.private_md, MD_CHARS, id, notices);
    }
    let worldbook = match pack.read(WORLDBOOK, limits.json_bytes)? {
        Some(bytes) => Some(String::from_utf8(bytes).map_err(|_| broken(WORLDBOOK.to_owned()))?),
        None => None,
    };
    for scene in &script.scenes {
        if !pack.contains(&history_name(scene.scene)) {
            return Err(broken(format!("{} missing", history_name(scene.scene))));
        }
    }
    Ok(Settings {
        bible,
        card,
        world,
        md,
        worldbook,
    })
}

/// 一幕的逐則完整快照：長度須等於該幕展開後事件數，每則序列化後不超過上限
pub(super) fn history(
    pack: &mut Pack,
    scene: u64,
    events: usize,
    limits: Limits,
) -> Result<Vec<Option<TableState>>, String> {
    let name = history_name(scene);
    let states: Vec<Option<TableState>> = pack.json(&name, limits.history_bytes)?;
    if states.len() != events {
        return Err(broken(format!("{name}: length")));
    }
    for state in states.iter().flatten() {
        if serde_json::to_vec(state).map_err(text)?.len() > STATE_BYTES {
            return Err(UiMsg::VnWorkTooLarge.to_string());
        }
    }
    Ok(states)
}

/// 機制規則照聖經推導（不收包內規則）：有聖經＝聖經規則＋增量回報＋受保護根（同 `vn_bible::install`）；
/// 引擎三鍵一律補上。
pub(super) fn derive_mechanism(mechanism: &mut Mechanism, bible: Option<&Bible>) {
    if let Some(bible) = bible {
        mechanism.rules.extend(bible.rules());
        mechanism.incremental = true;
        mechanism.locked_roots = vn::LOCKED_ROOTS.map(str::to_owned).to_vec();
    }
    vn::ensure_engine_rules(mechanism);
}
