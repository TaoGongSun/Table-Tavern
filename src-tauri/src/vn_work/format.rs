//! `.ttvn` 的型別與上限（plans/vn-mode-pkg8.md §2.1、§2.5）。未知欄位一律忽略；上限正式呼叫用 `LIMITS`，測試換小的。
use crate::data::vn::Rating;
use serde::{Deserialize, Serialize};

pub(crate) const FORMAT: &str = "tt-vn-work";
pub(crate) const VERSION: u32 = 1;
pub(crate) const MANIFEST: &str = "manifest.json";
pub(crate) const SCRIPT: &str = "script.json";
pub(crate) const CHARACTERS: &str = "characters.json";
pub(crate) const COVER: &str = "cover.png";

/// 匯入成哪種桌：重播（只讀演出）或可續寫（需包內附設定，§2.7）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    Replay,
    Continue,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub pack_bytes: u64,
    pub entries: usize,
    pub png_bytes: u64,
    pub script_bytes: u64,
    pub json_bytes: u64,
    pub total_bytes: u64,
    pub images: usize,
    pub total_pixels: u64,
    pub scenes: usize,
    pub scene_events: usize,
    pub total_events: usize,
    pub total_text_bytes: u64,
    pub characters: usize,
    /// 附設定：`settings/history/<幕>.json` 每檔、`settings/world.md`
    pub history_bytes: u64,
    pub world_bytes: u64,
    /// 作品包裡單首音訊（bgm 或 sfx），以及兩者合計檔數
    pub audio_bytes: u64,
    pub audio_files: usize,
}

pub(crate) const LIMITS: Limits = Limits {
    pack_bytes: 768 << 20,
    entries: 4_096,
    png_bytes: 24 << 20,
    script_bytes: 48 << 20,
    json_bytes: 4 << 20,
    total_bytes: 1536 << 20,
    images: 2_000,
    total_pixels: 3_000_000_000,
    scenes: 1_000,
    scene_events: 4_000,
    total_events: 60_000,
    total_text_bytes: 256 << 20,
    characters: 32,
    history_bytes: 16 << 20,
    world_bytes: 256 << 10,
    audio_bytes: 32 << 20,
    audio_files: 1_000,
};

/// 字串夾斷上限（字元）
pub(crate) const TITLE_CHARS: usize = 120;
pub(crate) const NAME_CHARS: usize = 64;
pub(crate) const TEXT_CHARS: usize = 12_000;
pub(crate) const PLACE_CHARS: usize = 200;
pub(crate) const EMOJI_CHARS: usize = 16;
/// 演出包每則最小快照的落檔量估算（展開後文字量上限一併計）
pub(crate) const STAGE_BYTES: u64 = 256;
pub(crate) const MAX_CHAPTER: u32 = 30;
pub(crate) const MAX_ENDINGS: usize = 30;
pub(crate) const MAX_LOCATIONS: usize = 200;
pub(crate) const MAX_EXTRA_EXPRESSIONS: usize = 6;
pub(crate) const MAX_VARIANTS: usize = 16;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Manifest {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    pub title: String,
    /// 標題畫面的主角一句話（原桌 `preferences.tagline`）；舊包沒有＝空
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tagline: String,
    pub lang: String,
    pub chapters: u32,
    pub endings: u32,
    pub settings: bool,
    pub vars_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<Rating>,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Script {
    pub current: u64,
    pub chapters: Vec<String>,
    pub endings: Vec<EndingName>,
    pub locations: Vec<Location>,
    pub scenes: Vec<Scene>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EndingName {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Location {
    pub key: String,
    pub name: String,
    pub times: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Scene {
    pub scene: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Label>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Label {
    pub base: u64,
    pub version: u32,
    pub parent: Option<u64>,
    pub forked: bool,
    pub source: Option<u64>,
    /// 有值＝分岔幕只存 `keep` 之後自己的事件，前段照來源幕補回
    pub keep: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    Narration,
    Dialogue,
    Player,
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Event {
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub speaker_name: String,
    pub text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub recap: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub opening: bool,
    pub stage: Stage,
}

/// 每則的最小快照：背景起點、章號、結局、強制收尾
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Stage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub finale: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Characters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    pub characters: Vec<Character>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Character {
    pub id: String,
    pub name: String,
    pub color: String,
    pub emoji: String,
    pub archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_cm: Option<u16>,
    pub expressions_extra: Vec<String>,
    pub variants: Vec<String>,
}

/// 作品包裡的音訊種類。前綴同時是檔名與 audio.json 的 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioKind {
    Bgm,
    Sfx,
}

impl AudioKind {
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::Bgm => "bgm",
            Self::Sfx => "sfx",
        }
    }
}

/// 作品包裡的一個資產，路徑形狀只收這幾種（鍵都驗過，落檔路徑由鍵重組，條目名不進路徑）
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asset {
    SpriteMeta {
        character: String,
    },
    /// `body/<key>.png`，`base`＝定稿
    SpriteBody {
        character: String,
        key: String,
    },
    /// `face/<表情>.png`（set None）或 `face-<變體>/<表情>.png`
    SpriteFace {
        character: String,
        set: Option<String>,
        expression: String,
    },
    Background {
        slug: String,
        period: String,
    },
    CharacterImage {
        character: String,
    },
    CharacterAvatar {
        character: String,
    },
    Cover,
    /// `assets/audio/<kind>-<key>.mp3`。bgm 鍵 `^[a-z0-9+]{1,64}$`；sfx 鍵只收 16 個小寫 hex
    Audio {
        kind: AudioKind,
        key: String,
    },
    /// `assets/audio/audio.json`
    AudioIndex,
}

impl Asset {
    /// 包內路徑
    pub(crate) fn path(&self) -> String {
        match self {
            Self::SpriteMeta { character } => format!("assets/sprites/{character}/sprite.json"),
            Self::SpriteBody { character, key } => {
                format!("assets/sprites/{character}/body/{key}.png")
            }
            Self::SpriteFace {
                character,
                set: None,
                expression,
            } => {
                format!("assets/sprites/{character}/face/{expression}.png")
            }
            Self::SpriteFace {
                character,
                set: Some(set),
                expression,
            } => {
                format!("assets/sprites/{character}/face-{set}/{expression}.png")
            }
            Self::Background { slug, period } => format!("assets/backgrounds/{slug}-{period}.png"),
            Self::CharacterImage { character } => format!("assets/characters/{character}.png"),
            Self::CharacterAvatar { character } => {
                format!("assets/characters/{character}.avatar.png")
            }
            Self::Cover => COVER.to_owned(),
            Self::Audio { kind, key } => format!("assets/audio/{}-{key}.mp3", kind.prefix()),
            Self::AudioIndex => "assets/audio/audio.json".to_owned(),
        }
    }

    /// 從包內路徑認回來；形狀不對或鍵不合格回 None
    pub(crate) fn parse(path: &str) -> Option<Self> {
        use crate::data::vn_art::valid_art_key;
        use crate::vn_sprites::store::valid_variant_key;
        let id = |text: &str| {
            crate::data::validate_id(text)
                .is_ok()
                .then(|| text.to_owned())
        };
        if path == COVER {
            return Some(Self::Cover);
        }
        let parts: Vec<&str> = path.split('/').collect();
        match parts.as_slice() {
            ["assets", "sprites", character, "sprite.json"] => Some(Self::SpriteMeta {
                character: id(character)?,
            }),
            ["assets", "sprites", character, "body", file] => {
                let key = file.strip_suffix(".png")?;
                (key == "base" || valid_variant_key(key)).then_some(())?;
                Some(Self::SpriteBody {
                    character: id(character)?,
                    key: key.to_owned(),
                })
            }
            ["assets", "sprites", character, dir, file] => {
                let expression = file.strip_suffix(".png")?;
                valid_art_key(expression).then_some(())?;
                let set = match *dir {
                    "face" => None,
                    other => {
                        let set = other.strip_prefix("face-")?;
                        valid_variant_key(set).then_some(())?;
                        Some(set.to_owned())
                    }
                };
                Some(Self::SpriteFace {
                    character: id(character)?,
                    set,
                    expression: expression.to_owned(),
                })
            }
            ["assets", "backgrounds", file] => {
                let stem = file.strip_suffix(".png")?;
                let (slug, period) = stem.split_once('-')?;
                (crate::vn_backgrounds::valid_slug(slug) && valid_art_key(period)).then_some(())?;
                Some(Self::Background {
                    slug: slug.to_owned(),
                    period: period.to_owned(),
                })
            }
            ["assets", "audio", "audio.json"] => Some(Self::AudioIndex),
            ["assets", "audio", file] => {
                let (kind, rest) = file
                    .strip_prefix("bgm-")
                    .map(|rest| (AudioKind::Bgm, rest))
                    .or_else(|| file.strip_prefix("sfx-").map(|rest| (AudioKind::Sfx, rest)))?;
                let key = rest.strip_suffix(".mp3")?;
                let ok = match kind {
                    AudioKind::Bgm => crate::vn_audio::valid_key(key),
                    AudioKind::Sfx => crate::vn_audio::valid_sfx_key(key),
                };
                ok.then_some(Self::Audio {
                    kind,
                    key: key.to_owned(),
                })
            }
            ["assets", "characters", file] => match file.strip_suffix(".avatar.png") {
                Some(character) => Some(Self::CharacterAvatar {
                    character: id(character)?,
                }),
                None => Some(Self::CharacterImage {
                    character: id(file.strip_suffix(".png")?)?,
                }),
            },
            _ => None,
        }
    }

    pub(crate) fn character(&self) -> Option<&str> {
        match self {
            Self::SpriteMeta { character }
            | Self::SpriteBody { character, .. }
            | Self::SpriteFace { character, .. }
            | Self::CharacterImage { character }
            | Self::CharacterAvatar { character } => Some(character),
            Self::Background { .. } | Self::Cover | Self::Audio { .. } | Self::AudioIndex => None,
        }
    }
}
