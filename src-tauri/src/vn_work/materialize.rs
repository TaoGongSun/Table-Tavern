//! 作品包匯入建重播桌或可續寫的桌（plans/vn-mode-pkg8.md §2.4、§2.7）：結構層在記憶體驗完才建桌；建桌後圖逐張「讀→嚴驗→剝 chunk
//! →寫」，幕依序整幕落地（分岔前段照已寫好的來源幕補回、`cut` 用新 id）；任何一步失敗刪整桌，不留半桌。
use super::archive::{broken, Pack};
use super::format::*;
use super::settings::Settings;
use super::validate::{self, Notice};
use crate::data::vn::{self, CardOrigin, CastOrigin};
use crate::data::vn_art::{ArtEntry, CardLocation, CastArt};
use crate::data::{
    self, CharacterCard, SceneLabel, StateNode, TableState, Tier, TranscriptEvent, TranscriptKind,
};
use crate::import::png_clean::sanitize_png;
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Serialize)]
pub struct WorkImported {
    pub world_id: String,
    pub notices: Vec<Notice>,
}

/// 只讀 manifest 給前端確認（標題、章數、結局數、分級）
#[derive(Debug, serde::Serialize)]
pub struct WorkProbe {
    pub title: String,
    pub lang: String,
    pub chapters: u32,
    pub endings: u32,
    pub settings: bool,
    /// 卡片變數模式的桌匯出的包：只能當作品玩
    pub vars_mode: bool,
    pub rating: Option<vn::Rating>,
}

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

fn open_manifest(path: &Path, limits: Limits) -> Result<(Pack, Manifest), String> {
    let mut pack = Pack::open(path, limits)?;
    if !pack.contains(MANIFEST) {
        return Err(UiMsg::VnWorkNotPack.to_string());
    }
    let manifest: Manifest = pack.json(MANIFEST, limits.json_bytes)?;
    validate::header(&manifest)?;
    Ok((pack, manifest))
}

pub fn probe_work(path: &Path) -> Result<WorkProbe, String> {
    let (_, mut manifest) = open_manifest(path, LIMITS)?;
    let mut title = std::mem::take(&mut manifest.title);
    validate::clip_title(&mut title);
    Ok(WorkProbe {
        title,
        lang: manifest.lang,
        chapters: manifest.chapters.min(MAX_CHAPTER),
        endings: manifest.endings.min(MAX_ENDINGS as u32),
        settings: manifest.settings,
        vars_mode: manifest.vars_mode,
        rating: manifest.rating,
    })
}

pub fn import_work(
    root: &Path,
    path: &Path,
    mode: ImportMode,
    spoiler_lock: bool,
) -> Result<WorkImported, String> {
    import_with(root, path, mode, spoiler_lock, LIMITS, &mut |_| Ok(()))
}

/// 測試掛點：每做完一步呼叫一次（步驟名），回錯＝模擬之後的步驟失敗。
pub(super) fn import_with(
    root: &Path,
    path: &Path,
    mode: ImportMode,
    spoiler_lock: bool,
    limits: Limits,
    hook: &mut dyn FnMut(&str) -> Result<(), String>,
) -> Result<WorkImported, String> {
    let (mut pack, mut manifest) = open_manifest(path, limits)?;
    let mut script: Script = pack.json(SCRIPT, limits.script_bytes)?;
    let mut cast: Characters = pack.json(CHARACTERS, limits.json_bytes)?;
    let mut notices = Vec::new();
    let assets = validate::check(
        &mut manifest,
        &mut script,
        &mut cast,
        limits,
        |name| pack.contains(name),
        &mut notices,
    )?;
    let settings = match mode {
        ImportMode::Continue => Some(super::settings::read(
            &mut pack,
            &manifest,
            &script,
            &cast,
            limits,
            &mut notices,
        )?),
        ImportMode::Replay => None,
    };
    let (world_id, held) = data::create_world_exclusive(root, &manifest.title, data::TableKind::Vn)
        .map_err(|failed| failed.error.to_string())?;
    let built = Build {
        root,
        world_id: &world_id,
        pack: &mut pack,
        limits,
        settings: settings.as_ref(),
        spoiler_lock,
    }
    .run(&manifest, &script, &cast, &assets, hook);
    match built {
        Ok(()) => Ok(WorkImported { world_id, notices }),
        Err(error) => {
            if let Err(cleanup) = data::discard_new_world(root, &world_id, &held) {
                log::warn!("VN 作品匯入失敗後收不掉新桌 {world_id}：{cleanup}");
            }
            Err(error)
        }
    }
}

struct Build<'a> {
    root: &'a Path,
    world_id: &'a str,
    pack: &'a mut Pack,
    limits: Limits,
    /// 有＝可續寫的桌（§2.7）
    settings: Option<&'a Settings>,
    spoiler_lock: bool,
}

impl Build<'_> {
    fn run(
        &mut self,
        manifest: &Manifest,
        script: &Script,
        cast: &Characters,
        assets: &[Asset],
        hook: &mut dyn FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        let (root, world_id, settings) = (self.root, self.world_id, self.settings);
        for member in &cast.characters {
            let mut card = card_of(member);
            if let Some(md) = settings.and_then(|settings| settings.md.get(&member.id)) {
                card.public_md = md.public_md.clone();
                card.private_md = md.private_md.clone();
            }
            data::write_character(root, world_id, &card).map_err(text)?;
        }
        hook("characters")?;
        if let Some(settings) = settings {
            data::write_world_md(root, world_id, &settings.world).map_err(text)?;
            // 世界書在角色之後：可見角色要對得到本桌角色，對不到的剔除
            if let Some(book) = &settings.worldbook {
                data::import_worldbook_as(root, world_id, book, &data::BookOwner::Gm)
                    .map_err(text)?;
            }
            hook("worldbook")?;
        }
        let mut pixels = 0u64;
        let mut audio_keys = BTreeSet::new();
        let mut saw_index = false;
        for asset in assets {
            match asset {
                Asset::AudioIndex => saw_index = true,
                Asset::Audio { kind, key } => {
                    audio_keys.insert((kind.prefix().to_owned(), key.clone()));
                    self.write_asset(asset, &mut pixels)?;
                }
                _ => self.write_asset(asset, &mut pixels)?,
            }
        }
        // 先落 mp3 再濾索引，對不到檔的條目丟掉。
        if saw_index {
            self.write_audio_index(&audio_keys)?;
        }
        hook("assets")?;
        let labels = self.write_scenes(script, hook)?;
        // 重播桌一律從幕 0；可續寫落在作者匯出時所在的幕
        let current = settings.map_or(0, |_| script.current);
        let last = data::read_transcript(root, world_id, current).map_err(text)?;
        data::update_state(root, world_id, |state| {
            state.replay = settings.is_none();
            state.current_scene = current;
            state.scene_labels = labels;
            state.scene_titles = script
                .scenes
                .iter()
                .filter_map(|scene| Some((scene.scene.to_string(), scene.title.clone()?)))
                .collect();
            state.player_card_id = cast.player.clone();
            state.state = last
                .iter()
                .rev()
                .find_map(|event| event.state.clone())
                .unwrap_or_default();
            if let Some(settings) = settings {
                super::settings::derive_mechanism(&mut state.mechanism, settings.bible.as_ref());
            }
            Ok(Some(()))
        })
        .map_err(text)?;
        hook("state")?;
        let (spoiler_lock, bible, card) = match settings {
            Some(settings) => (
                self.spoiler_lock,
                settings.bible.clone(),
                settings.card.clone(),
            ),
            None => (
                false,
                Some(min_bible(manifest, script)),
                Some(min_card(manifest, script, cast)),
            ),
        };
        data::state_commit::with_commit(root, world_id, |tx| {
            vn::edit_vn(tx, |file| {
                file.spoiler_lock = spoiler_lock;
                file.bible = bible;
                file.card = card;
                if !manifest.tagline.is_empty() {
                    file.preferences
                        .get_or_insert_with(Default::default)
                        .tagline = manifest.tagline.clone();
                }
            })
        })
        .map_err(text)?;
        hook("vn")
    }

    fn target(&self, asset: &Asset) -> Result<PathBuf, String> {
        let (root, world) = (self.root, self.world_id);
        let sprite = |character: &str| data::vn_sprite_dir(root, world, character).map_err(text);
        Ok(match asset {
            Asset::SpriteMeta { character } => sprite(character)?.join("sprite.json"),
            Asset::SpriteBody { character, key } => {
                sprite(character)?.join("body").join(format!("{key}.png"))
            }
            Asset::SpriteFace {
                character,
                set: None,
                expression,
            } => sprite(character)?
                .join("face")
                .join(format!("{expression}.png")),
            Asset::SpriteFace {
                character,
                set: Some(set),
                expression,
            } => sprite(character)?
                .join(format!("face-{set}"))
                .join(format!("{expression}.png")),
            Asset::Background { slug, period } => data::vn_background_dir(root, world)
                .map_err(text)?
                .join(format!("{slug}-{period}.png")),
            Asset::CharacterImage { character } => data::character_path(root, world, character)
                .map_err(text)?
                .with_extension("png"),
            Asset::CharacterAvatar { character } => data::character_path(root, world, character)
                .map_err(text)?
                .with_extension("avatar.png"),
            Asset::Cover => vn::cover_path(root, world).map_err(text)?,
            Asset::Audio { kind, key } => data::vn_audio_dir(root, world)
                .map_err(text)?
                .join(format!("{}-{key}.mp3", kind.prefix())),
            Asset::AudioIndex => data::vn_audio_dir(root, world)
                .map_err(text)?
                .join("audio.json"),
        })
    }

    fn write_audio_index(&mut self, keys: &BTreeSet<(String, String)>) -> Result<(), String> {
        let name = Asset::AudioIndex.path();
        let raw = self
            .pack
            .read(&name, self.limits.json_bytes)?
            .unwrap_or_default();
        let entries = crate::vn_audio::import_entries(&raw, keys, self.limits.audio_files)
            .map_err(|error| {
                if error == "AUDIO_INDEX_TOO_MANY" {
                    UiMsg::VnWorkTooLarge.to_string()
                } else {
                    broken(format!("{name}: {error}"))
                }
            })?;
        let bytes = serde_json::to_vec(&entries).map_err(text)?;
        data::commit_world_write_atomic(&self.target(&Asset::AudioIndex)?, &bytes).map_err(text)
    }

    /// 落檔路徑由驗過的鍵重組（`target`），條目名只用來讀
    fn write_asset(&mut self, asset: &Asset, pixels: &mut u64) -> Result<(), String> {
        let name = asset.path();
        let bytes = match asset {
            Asset::Audio { .. } => {
                let raw = self
                    .pack
                    .read(&name, self.limits.audio_bytes)?
                    .unwrap_or_default();
                if !crate::vn_audio::mp3_ok(&raw) {
                    return Err(broken(format!("{name}: bad mp3")));
                }
                raw
            }
            Asset::AudioIndex => Vec::new(),
            Asset::SpriteMeta { .. } => {
                let raw = self
                    .pack
                    .read(&name, self.limits.json_bytes)?
                    .unwrap_or_default();
                let meta: crate::vn_sprites::store::SpriteMeta = serde_json::from_slice(&raw)
                    .map_err(|error| broken(format!("{name}: {error}")))?;
                serde_json::to_vec_pretty(&meta).map_err(text)?
            }
            _ => {
                let raw = self
                    .pack
                    .read(&name, self.limits.png_bytes)?
                    .unwrap_or_default();
                let (clean, (width, height)) =
                    sanitize_png(&raw).map_err(|error| broken(format!("{name}: {error}")))?;
                *pixels += u64::from(width) * u64::from(height);
                if *pixels > self.limits.total_pixels {
                    return Err(UiMsg::VnWorkTooLarge.to_string());
                }
                clean
            }
        };
        data::commit_world_write_atomic(&self.target(asset)?, &bytes).map_err(text)
    }

    /// 依幕號逐幕整幕落地，回標籤表。分岔前段複製已寫好的來源幕（新 id），`cut`＝來源幕剛寫回的第 keep−1 則。
    fn write_scenes(
        &mut self,
        script: &Script,
        hook: &mut dyn FnMut(&str) -> Result<(), String>,
    ) -> Result<BTreeMap<String, SceneLabel>, String> {
        let mut written: Vec<Vec<TranscriptEvent>> = Vec::with_capacity(script.scenes.len());
        let mut labels = BTreeMap::new();
        for scene in &script.scenes {
            let mut events = Vec::new();
            let mut cut = None;
            if let Some(label) = &scene.label {
                if let (Some(source), Some(keep)) = (label.source, label.keep) {
                    let from = &written[source as usize];
                    let keep = keep as usize;
                    events.extend(from[..keep].iter().cloned().map(|mut event| {
                        event.id = None;
                        event
                    }));
                    cut = from[keep - 1].id.clone();
                }
                labels.insert(
                    scene.scene.to_string(),
                    SceneLabel {
                        base: label.base,
                        version: label.version,
                        parent: label.parent,
                        forked: label.forked,
                        source: label.source,
                        keep: label.keep,
                        cut,
                    },
                );
            }
            events.extend(scene.events.iter().map(transcript_event));
            if self.settings.is_some() {
                let states =
                    super::settings::history(self.pack, scene.scene, events.len(), self.limits)?;
                for (event, state) in events.iter_mut().zip(states) {
                    event.state = state;
                }
            }
            let mut landed = data::write_work_scene(self.root, self.world_id, scene.scene, events)
                .map_err(text)?;
            if self.settings.is_some() {
                // 分岔前段的快照由該幕自己的快照檔給，這裡不必留在記憶體
                for event in &mut landed {
                    event.state = None;
                }
            }
            written.push(landed);
            hook(&format!("scene {}", scene.scene))?;
        }
        Ok(labels)
    }
}

fn card_of(member: &Character) -> CharacterCard {
    CharacterCard {
        id: member.id.clone(),
        name: member.name.clone(),
        color: member.color.clone(),
        avatar: member.emoji.clone(),
        tier: Tier::Balanced,
        show_image: true,
        archived: member.archived,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
        stage: false,
    }
}

fn transcript_event(event: &Event) -> TranscriptEvent {
    TranscriptEvent {
        dice_pool: None,
        ts: data::local_timestamp().unwrap_or_default(),
        speaker_id: event.speaker.clone().unwrap_or_default(),
        speaker_name: event.speaker_name.clone(),
        kind: match event.kind {
            Kind::Narration => TranscriptKind::Narration,
            Kind::Dialogue => TranscriptKind::Dialogue,
            Kind::Player => TranscriptKind::Player,
            Kind::System => TranscriptKind::System,
        },
        text: event.text.clone(),
        raw: None,
        state: Some(snapshot(&event.stage)),
        truncated: false,
        gm_only: event.kind == Kind::System,
        marker: event.recap.then_some(data::EventMarker::SceneSummary),
        opening: event.opening,
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
    }
}

/// 最小快照轉回狀態：背景起點進平欄，章號、結局、強制收尾進樹
fn snapshot(stage: &Stage) -> TableState {
    let mut state = TableState::default();
    for (key, value) in [("place", &stage.place), ("time", &stage.time)] {
        if let Some(value) = value {
            state.table.insert(key.to_owned(), value.clone());
        }
    }
    let leaf = |text: String| StateNode::Leaf(text);
    if let Some(chapter) = stage.chapter {
        state
            .tree
            .insert("chapter".to_owned(), leaf(chapter.to_string()));
    }
    if stage.chapter.is_some() || stage.ended.is_some() {
        let ended = stage
            .ended
            .clone()
            .unwrap_or_else(|| vn::NOT_ENDED.to_owned());
        state.tree.insert("ended".to_owned(), leaf(ended));
        state
            .tree
            .insert("finale".to_owned(), leaf(stage.finale.to_string()));
    }
    state
}

/// 演出用最小聖經：只有章名與包內達成的結局名，讓章字卡、結局一覽、完結畫面原樣運作
fn min_bible(manifest: &Manifest, script: &Script) -> vn::Bible {
    vn::Bible {
        chapter_max: manifest
            .chapters
            .clamp(1, MAX_CHAPTER)
            .max(script.chapters.len() as u32),
        chapters: script
            .chapters
            .iter()
            .map(|title| vn::Chapter {
                title: title.clone(),
                ..Default::default()
            })
            .collect(),
        endings: script
            .endings
            .iter()
            .map(|ending| vn::Ending {
                id: ending.id.clone(),
                title: ending.title.clone(),
                when: Vec::new(),
            })
            .collect(),
        ..Default::default()
    }
}

/// 演出用最小卡原件：只有解析名單要的表情／變體鍵、身高與卡內地點；生圖用的描述一律空
fn min_card(manifest: &Manifest, script: &Script, cast: &Characters) -> CardOrigin {
    let keys = |keys: &[String]| -> Vec<ArtEntry> {
        keys.iter()
            .map(|key| ArtEntry {
                key: key.clone(),
                ..Default::default()
            })
            .collect()
    };
    CardOrigin {
        lang: manifest.lang.chars().take(16).collect(),
        rating: manifest.rating.clone().unwrap_or_default(),
        cast: cast
            .characters
            .iter()
            .map(|member| {
                let art = CastArt {
                    appearance: None,
                    expressions: keys(&member.expressions_extra),
                    variants: keys(&member.variants),
                    height_cm: member.height_cm,
                };
                (
                    member.id.clone(),
                    CastOrigin {
                        art,
                        ..Default::default()
                    },
                )
            })
            .collect(),
        locations: script
            .locations
            .iter()
            .map(|place| CardLocation {
                key: place.key.clone(),
                name: place.name.clone(),
                times: place.times.clone(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
