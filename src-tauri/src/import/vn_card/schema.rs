//! VN 卡 `data.extensions.table_tavern.vn` 的型別、驗證與夾斷（plans/vn-mode-pkg9.md §2.1、§2.3 第 1 步）。
//! 不信任輸入：逐欄取值、型別不對當沒填、未知欄位忽略；超量的夾斷或丟棄並記一條 notice，不拒收整張。
use crate::data::vn::{self, Bible, CardOrigin, CastOrigin, CastRole, Pov, Rating};
use crate::data::vn_art::{CardArt, CardLocation, CastArt};
use crate::data::{self, DataResult};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

pub(crate) const FORMAT: &str = "tt-vn-card";
pub(crate) const VERSION: u64 = 1;
const CAST_MAX: usize = 8;
const PUBLIC_CAP: usize = 1_500;
const WORLD_CAP: usize = 1_500;
const OPENING_CAP: usize = 2_000;
const APPEARS_CAP: usize = 200;
/// 整個 `vn` 序列化後的字元上限：超過先丟 9b 欄位，還超過才拒收
const TOTAL_CAP: usize = 100_000;
/// 美術欄位（卡太大時先丟）
const CARD_EXTRAS: [&str; 2] = ["art", "locations"];
const CAST_EXTRAS: [&str; 4] = ["appearance", "expressions_extra", "variants", "height_cm"];
const COLORS: [&str; 6] = [
    "#e07a5f", "#3d84a8", "#f2a541", "#7b9e89", "#8e7cc3", "#c76b8e",
];
pub(crate) const TOO_LARGE: &str = "VN_CARD_TOO_LARGE";
pub(crate) const NO_CAST: &str = "VN_CARD_NO_CAST";
pub(crate) const UNSUPPORTED: &str = "VN_CARD_UNSUPPORTED";

/// 匯入時給玩家看的一條提示；`target` 是欄位路徑或卡內 id。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub kind: NoticeKind,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// 文字超過上限，已截短
    Clipped,
    /// 角色超過 8 人，後面的沒建（target＝丟掉的人數）
    CastOver,
    /// 角色 id 不合格、重複或名字不合格，整筆沒建
    CastDropped,
    /// 色碼不合格，換成預設色
    ColorReset,
    /// 卡定主角對不到角色，改由玩家當主角
    ProtagonistMissing,
    /// 秘密的持有者對不到角色，改成世界層秘密
    HolderDropped,
    /// 世界書條目的可見角色對不到，已剔除（剔空了改 GM 可見）
    VisibilityDropped,
    /// 整張卡超過大小上限，美術欄位沒保存
    ExtrasDropped,
    /// 美術欄位的一筆（地點、時段、表情、變體、身高）不合格、重複或超量，沒保存
    ArtDropped,
}

pub(crate) fn notice(kind: NoticeKind, target: impl Into<String>) -> Notice {
    Notice {
        kind,
        target: target.into(),
    }
}

/// 匯入前探測：分級確認框要的摘要。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VnCardProbe {
    pub supported: bool,
    pub title: String,
    pub rating: Rating,
    pub cast: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VnCard {
    pub title: Option<String>,
    pub lang: String,
    pub rating: Rating,
    pub pov: Option<Pov>,
    /// 卡內角色 id；對不到 cast 已是 None（並記 notice）
    pub protagonist: Option<String>,
    pub world: String,
    pub opening: String,
    pub cast: Vec<CastMember>,
    /// 解析未正規化（id 重對應後才 normalize）
    pub bible: Option<Bible>,
    pub art: Option<CardArt>,
    pub locations: Vec<CardLocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CastMember {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: String,
    pub role: Option<CastRole>,
    pub appears: String,
    pub public: String,
    pub private: String,
    pub art: CastArt,
}

/// 卡上的 `vn` 擴充（`format` 相符才算）。
pub(crate) fn vn_value(card_data: &Value) -> Option<&Value> {
    card_data
        .get("extensions")?
        .get("table_tavern")?
        .get("vn")
        .filter(|vn| vn.get("format").and_then(Value::as_str) == Some(FORMAT))
}

pub(crate) fn supported(vn: &Value) -> bool {
    vn.get("version").and_then(Value::as_u64) == Some(VERSION)
}

pub(crate) fn probe(card_data: &Value, card_name: &str) -> Option<VnCardProbe> {
    let vn = vn_value(card_data)?;
    Some(VnCardProbe {
        supported: supported(vn),
        title: title(vn).unwrap_or_else(|| card_name.to_owned()),
        rating: rating(vn),
        cast: vn
            .get("cast")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|member| text(member, "name"))
            .filter(|name| !name.is_empty())
            .take(CAST_MAX)
            .collect(),
    })
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|text| text.trim().to_owned())
}

fn title(vn: &Value) -> Option<String> {
    text(vn, "title")
        .filter(|title| !title.is_empty() && data::validate_single_line("title", title).is_ok())
}

fn rating(vn: &Value) -> Rating {
    vn.get("rating")
        .and_then(|rating| serde_json::from_value::<Rating>(rating.clone()).ok())
        .unwrap_or_default()
}

/// 截到 `cap` 字，超過記一條 Clipped。
fn clipped(value: Option<String>, cap: usize, target: &str, notices: &mut Vec<Notice>) -> String {
    let value = value.unwrap_or_default();
    if value.chars().count() <= cap {
        return value;
    }
    notices.push(notice(NoticeKind::Clipped, target));
    vn::cap_chars(&value, cap)
}

fn hex_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 逐欄解析與夾斷。整份超過大小上限先丟美術欄位再判；還超過、或沒有一個合格角色就拒收。
pub(crate) fn parse(vn: &Value, notices: &mut Vec<Notice>) -> DataResult<VnCard> {
    if !supported(vn) {
        return Err(data::invalid_data(UNSUPPORTED));
    }
    let mut vn = vn.clone();
    if chars_of(&vn)? > TOTAL_CAP {
        if strip_extras(&mut vn) {
            notices.push(notice(NoticeKind::ExtrasDropped, ""));
        }
        if chars_of(&vn)? > TOTAL_CAP {
            return Err(data::invalid_data(TOO_LARGE));
        }
    }
    let cast = parse_cast(&vn, notices);
    if cast.is_empty() {
        return Err(data::invalid_data(NO_CAST));
    }
    let protagonist = match vn.get("protagonist") {
        Some(protagonist) if protagonist.get("mode").and_then(Value::as_str) == Some("fixed") => {
            let id = text(protagonist, "id").unwrap_or_default();
            match cast.iter().any(|member| member.id == id) {
                true => Some(id),
                false => {
                    notices.push(notice(NoticeKind::ProtagonistMissing, id));
                    None
                }
            }
        }
        _ => None,
    };
    let bible = vn.get("bible").and_then(bible_from);
    let (art, locations) = super::art::card_art(&vn, notices);
    Ok(VnCard {
        title: title(&vn),
        lang: text(&vn, "lang")
            .filter(|lang| lang.len() <= 35)
            .unwrap_or_default(),
        rating: rating(&vn),
        pov: vn
            .get("pov")
            .and_then(|pov| serde_json::from_value(pov.clone()).ok()),
        protagonist,
        world: clipped(text(&vn, "world"), WORLD_CAP, "world", notices),
        opening: clipped(text(&vn, "opening"), OPENING_CAP, "opening", notices),
        cast,
        bible,
        art,
        locations,
    })
}

/// 聖經寬鬆讀進來（含章數鎖）；讀完還要 `normalize`
pub(crate) fn bible_from(value: &Value) -> Option<Bible> {
    let mut bible = vn::parse_bible(value)?;
    bible.chapter_lock = value.get("chapter_lock").and_then(Value::as_bool) == Some(true);
    Some(bible)
}

/// 作品包附設定的卡原件（`export::origin_json` 的形狀）：卡層級與逐角色美術欄位走同一套驗證夾斷，
/// 角色只收 `cast`（桌內 id, 名字）裡有的，原版聖經照樣正規化。
pub(crate) fn origin_from(
    value: &Value,
    cast: &[(String, String)],
    notices: &mut Vec<Notice>,
) -> CardOrigin {
    let (art, locations) = super::art::card_art(value, notices);
    let members = value.get("cast").and_then(Value::as_object);
    CardOrigin {
        lang: text(value, "lang")
            .filter(|lang| lang.len() <= 35)
            .unwrap_or_default(),
        rating: rating(value),
        pov: value
            .get("pov")
            .and_then(|pov| serde_json::from_value(pov.clone()).ok()),
        protagonist_fixed: value.get("protagonist_fixed").and_then(Value::as_bool) == Some(true),
        original_bible: value
            .get("original_bible")
            .and_then(bible_from)
            .map(|bible| vn::normalize(bible, cast)),
        cast: cast
            .iter()
            .filter_map(|(id, _)| Some((id, members?.get(id)?)))
            .map(|(id, member)| {
                let origin = CastOrigin {
                    role: member
                        .get("role")
                        .and_then(|role| serde_json::from_value(role.clone()).ok()),
                    appears: clipped(
                        text(member, "appears"),
                        APPEARS_CAP,
                        &format!("{id}.appears"),
                        notices,
                    ),
                    art: super::art::cast_art(member, id, notices),
                };
                (id.clone(), origin)
            })
            .collect(),
        art,
        locations,
    }
}

fn chars_of(vn: &Value) -> DataResult<usize> {
    Ok(serde_json::to_string(vn)?.chars().count())
}

/// 拿掉卡層級與逐角色的 9b 欄位；回傳有沒有拿掉東西。
fn strip_extras(vn: &mut Value) -> bool {
    let mut stripped = false;
    if let Some(object) = vn.as_object_mut() {
        for key in CARD_EXTRAS {
            stripped |= object.remove(key).is_some();
        }
    }
    for member in vn
        .get_mut("cast")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(object) = member.as_object_mut() {
            for key in CAST_EXTRAS {
                stripped |= object.remove(key).is_some();
            }
        }
    }
    stripped
}

fn parse_cast(vn: &Value, notices: &mut Vec<Notice>) -> Vec<CastMember> {
    let mut seen = HashSet::new();
    let mut cast = Vec::new();
    for (index, member) in vn
        .get("cast")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let id = text(member, "id").unwrap_or_default();
        let name = text(member, "name").unwrap_or_default();
        let target = match id.is_empty() {
            true => format!("cast[{index}]"),
            false => id.clone(),
        };
        // 重複 id 留第一個：重對應表不得被後者覆蓋
        if !vn::valid_id(&id)
            || name.is_empty()
            || data::validate_single_line("name", &name).is_err()
            || !seen.insert(id.clone())
        {
            notices.push(notice(NoticeKind::CastDropped, target));
            continue;
        }
        let color = match text(member, "color") {
            Some(color) if hex_color(&color) => color,
            given => {
                if given.is_some() {
                    notices.push(notice(NoticeKind::ColorReset, &id));
                }
                COLORS[cast.len() % COLORS.len()].to_owned()
            }
        };
        let emoji = text(member, "emoji")
            .filter(|emoji| {
                (1..=8).contains(&emoji.chars().count())
                    && data::validate_single_line("emoji", emoji).is_ok()
            })
            .unwrap_or_else(|| "🎭".to_owned());
        cast.push(CastMember {
            role: member
                .get("role")
                .and_then(|role| serde_json::from_value(role.clone()).ok()),
            appears: clipped(
                text(member, "appears"),
                APPEARS_CAP,
                &format!("{id}.appears"),
                notices,
            ),
            public: clipped(
                text(member, "public"),
                PUBLIC_CAP,
                &format!("{id}.public"),
                notices,
            ),
            private: clipped(
                text(member, "private"),
                vn::PRIVATE_CAP,
                &format!("{id}.private"),
                notices,
            ),
            art: super::art::cast_art(member, &id, notices),
            id,
            name,
            color,
            emoji,
        });
    }
    if cast.len() > CAST_MAX {
        notices.push(notice(
            NoticeKind::CastOver,
            (cast.len() - CAST_MAX).to_string(),
        ));
        cast.truncate(CAST_MAX);
    }
    cast
}
