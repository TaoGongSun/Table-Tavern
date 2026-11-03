//! VN 桌匯出成作品包（plans/vn-mode-pkg8.md §2.1、§2.2）：只帶演出所需。逐字稿與狀態在一次短提交鎖內讀齊，
//! 圖在鎖外逐張「嚴驗＋chunk 白名單」後串流寫進 zip；先寫 `<path>.part` 再改名，失敗不留半個檔。
//! 寫完前用匯入端同一支結構驗證自驗，保證自家包自家收得下。
use super::format::*;
use super::validate::{self, Notice, NoticeKind};
use crate::data::vn::{self, CardOrigin};
use crate::data::{self, TranscriptEvent, TranscriptKind, WorldState};
use crate::import::png_clean::sanitize_png;
use crate::ui_msg::UiMsg;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, serde::Serialize)]
pub struct WorkExported {
    pub notices: Vec<Notice>,
}

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// 鎖內讀齊的一份：狀態、已發布且非空的幕（原幕號）、聖經與卡原件、是否變數模式
struct Snapshot {
    state: WorldState,
    scenes: BTreeMap<u64, Vec<TranscriptEvent>>,
    vn: vn::VnFile,
    vars_mode: bool,
}

fn snapshot(root: &Path, world_id: &str) -> Result<Snapshot, String> {
    data::state_commit::with_commit(root, world_id, |tx| {
        data::message_vars::refuse_during_turn(tx)?;
        let state = data::read_state(root, world_id)?;
        vn::refuse_replay(&state)?;
        if state.table_kind != data::TableKind::Vn {
            return Err(data::invalid_data("export needs a VN table"));
        }
        let mut scenes = BTreeMap::new();
        for scene in 0..=data::scene_high(&state) {
            let events = data::read_transcript(root, world_id, scene)?;
            if !events.is_empty() {
                scenes.insert(scene, events);
            }
        }
        let vars_mode = data::message_vars::read_control(root, world_id)?
            .active(state.current_scene)
            .is_some();
        Ok(Snapshot {
            vn: vn::read_vn(root, world_id)?,
            state,
            scenes,
            vars_mode,
        })
    })
    .map_err(text)
}

pub fn export_work(
    root: &Path,
    world_id: &str,
    path: &Path,
    cover: Option<&Path>,
    lang: &str,
    settings: bool,
) -> Result<WorkExported, String> {
    data::refuse_if_updating().map_err(text)?;
    let snap = snapshot(root, world_id)?;
    if settings && snap.vars_mode {
        return Err(UiMsg::VnWorkVarsMode.to_string());
    }
    let ended = snap.scenes.values().flatten().any(|event| {
        event
            .state
            .as_ref()
            .is_some_and(|state| vn::ended_in(&state.tree).is_some())
    });
    if !ended {
        return Err(UiMsg::VnWorkNoEnding.to_string());
    }
    let mut script = script_of(&snap);
    let mut cast = characters_of(root, world_id, &snap)?;
    let mut manifest = Manifest {
        format: FORMAT.to_owned(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        title: snap.state.name.clone(),
        lang: snap
            .vn
            .card
            .as_ref()
            .map_or_else(|| lang.to_owned(), |card| card.lang.clone()),
        chapters: script.chapters.len() as u32,
        endings: script.endings.len() as u32,
        settings,
        vars_mode: snap.vars_mode,
        rating: snap.vn.card.as_ref().map(|card| card.rating.clone()),
        assets: Vec::new(),
    };
    let mut notices = Vec::new();
    // 自驗結構（資產還沒寫，先全收）；超限或對不上就不出包
    validate::check(
        &mut manifest,
        &mut script,
        &mut cast,
        LIMITS,
        |_| true,
        &mut Vec::new(),
    )?;
    let extra = match settings {
        true => super::settings::export_files(
            root,
            world_id,
            &snap.vn,
            snap.scenes.values().map(Vec::as_slice),
            &cast,
        )?,
        false => Vec::new(),
    };
    let part = PathBuf::from(format!("{}.part", path.display()));
    let written = write_pack(
        root,
        world_id,
        &part,
        cover,
        &mut manifest,
        &script,
        &cast,
        &extra,
        &mut notices,
    )
    .and_then(|()| std::fs::rename(&part, path).map_err(text));
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written.map(|()| WorkExported { notices })
}

/// 收幕：原幕號遞增緊排成 0..n−1；分岔幕來源前段未改寫（`origin` 同一條判斷）就只放自己的後段
fn script_of(snap: &Snapshot) -> Script {
    let index: BTreeMap<u64, u64> = snap
        .scenes
        .keys()
        .enumerate()
        .map(|(new, old)| (*old, new as u64))
        .collect();
    let mut max_chapter = 1;
    let mut reached = BTreeSet::new();
    let mut scenes = Vec::new();
    for (&old, events) in &snap.scenes {
        let at = index[&old];
        let mut label = None;
        let mut own = events.as_slice();
        if at > 0 {
            let found = data::scene_label(&snap.state, old);
            let map = |scene: Option<u64>| {
                scene
                    .and_then(|scene| index.get(&scene).copied())
                    .filter(|&n| n < at)
            };
            let source = map(found.source).filter(|_| found.forked);
            let keep = match (source, found.source, found.keep, found.cut.as_deref()) {
                (Some(_), Some(from), Some(keep), Some(cut))
                    if keep > 0
                        && (keep as usize) <= events.len()
                        && snap
                            .scenes
                            .get(&from)
                            .and_then(|from| from.get(keep as usize - 1))
                            .and_then(|event| event.id.as_deref())
                            == Some(cut) =>
                {
                    own = &events[keep as usize..];
                    Some(keep)
                }
                _ => None,
            };
            label = Some(Label {
                base: found.base,
                version: found.version,
                parent: map(found.parent),
                forked: source.is_some(),
                source,
                keep,
            });
        }
        let events: Vec<Event> = own.iter().map(event_of).collect();
        for event in &events {
            max_chapter = max_chapter.max(event.stage.chapter.unwrap_or(1));
            reached.extend(event.stage.ended.clone());
        }
        scenes.push(Scene {
            scene: at,
            title: snap.state.scene_titles.get(&old.to_string()).cloned(),
            label,
            events,
        });
    }
    let bible = snap.vn.bible.as_ref();
    Script {
        current: index
            .get(&snap.state.current_scene)
            .copied()
            .unwrap_or_default(),
        chapters: bible.map_or_else(Vec::new, |bible| {
            bible
                .chapters
                .iter()
                .take(max_chapter as usize)
                .map(|chapter| chapter.title.clone())
                .collect()
        }),
        endings: reached
            .into_iter()
            .map(|id| EndingName {
                title: bible
                    .and_then(|bible| bible.endings.iter().find(|ending| ending.id == id))
                    .map(|ending| ending.title.clone())
                    .unwrap_or_default(),
                id,
            })
            .collect(),
        locations: snap.vn.card.as_ref().map_or_else(Vec::new, |card| {
            card.locations
                .iter()
                .map(|place| Location {
                    key: place.key.clone(),
                    name: place.name.clone(),
                    times: place.times.clone(),
                })
                .collect()
        }),
        scenes,
    }
}

/// 一則事件只留演出所需：system 與 GM 限定事件保留位置、清空內容；`raw`、完整快照、id、時間、回合鍵、變數表都不帶
fn event_of(event: &TranscriptEvent) -> Event {
    let hidden = event.kind == TranscriptKind::System || event.gm_only;
    let state = event.state.as_ref();
    Event {
        kind: match event.kind {
            _ if hidden => Kind::System,
            TranscriptKind::Narration => Kind::Narration,
            TranscriptKind::Dialogue => Kind::Dialogue,
            TranscriptKind::Player => Kind::Player,
            TranscriptKind::System => Kind::System,
        },
        speaker: (!hidden
            && event.kind == TranscriptKind::Dialogue
            && !event.speaker_id.is_empty())
        .then(|| event.speaker_id.clone()),
        speaker_name: if hidden {
            String::new()
        } else {
            event.speaker_name.clone()
        },
        text: if hidden {
            String::new()
        } else {
            event.text.clone()
        },
        recap: !hidden && event.marker == Some(data::EventMarker::SceneSummary),
        opening: event.opening,
        stage: Stage {
            place: state.and_then(|state| state.table.get("place").cloned()),
            time: state.and_then(|state| state.table.get("time").cloned()),
            chapter: state
                .filter(|state| state.tree.contains_key("chapter"))
                .map(|state| vn::chapter_in(&state.tree)),
            ended: state.and_then(|state| vn::ended_in(&state.tree).map(str::to_owned)),
            finale: state.is_some_and(|state| vn::finale_in(&state.tree)),
        },
    }
}

/// 角色表：全部角色（含封存）加玩家卡／卡定主角（名單不列它，但解析器要它的名字對到主角）；
/// 事件引用了已刪角色就補一筆封存的名牌
fn characters_of(root: &Path, world_id: &str, snap: &Snapshot) -> Result<Characters, String> {
    let card = snap.vn.card.as_ref();
    let member = |id: &str, name: &str, color: &str, emoji: &str, archived: bool| {
        let art = card
            .and_then(|card: &CardOrigin| card.cast.get(id))
            .map(|cast| &cast.art);
        Character {
            id: id.to_owned(),
            name: name.to_owned(),
            color: color.to_owned(),
            emoji: emoji.to_owned(),
            archived,
            height_cm: art.and_then(|art| art.height_cm),
            expressions_extra: art.map_or_else(Vec::new, |art| art.expression_keys()),
            variants: art.map_or_else(Vec::new, |art| {
                art.variants.iter().map(|entry| entry.key.clone()).collect()
            }),
        }
    };
    let mut characters: Vec<Character> = data::list_characters(root, world_id)
        .map_err(text)?
        .iter()
        .map(|meta| {
            member(
                &meta.id,
                &meta.name,
                &meta.color,
                &meta.avatar,
                meta.archived,
            )
        })
        .collect();
    let player = data::read_player_card(root, world_id).map_err(text)?;
    if let Some(player) = &player {
        if !characters.iter().any(|known| known.id == player.id) {
            characters.push(member(
                &player.id,
                &player.name,
                &player.color,
                &player.avatar,
                player.archived,
            ));
        }
    }
    let known: BTreeSet<String> = characters.iter().map(|member| member.id.clone()).collect();
    let mut stubs = BTreeMap::new();
    for event in snap.scenes.values().flatten() {
        if event.kind == TranscriptKind::Dialogue
            && !event.speaker_id.is_empty()
            && !known.contains(&event.speaker_id)
        {
            stubs
                .entry(event.speaker_id.clone())
                .or_insert_with(|| event.speaker_name.clone());
        }
    }
    characters.extend(
        stubs
            .into_iter()
            .filter(|(id, _)| data::validate_id(id).is_ok())
            .map(|(id, name)| Character {
                id,
                name,
                color: "#888888".to_owned(),
                archived: true,
                ..Default::default()
            }),
    );
    let player = snap
        .state
        .player_card_id
        .clone()
        .filter(|id| known.contains(id));
    Ok(Characters { player, characters })
}

/// 要出包的圖（磁碟來源路徑、包內資產）：立繪定稿先過驗，過不了整份立繪（含 sprite.json）都不帶
struct Writer<'a> {
    zip: ZipWriter<BufWriter<File>>,
    manifest: &'a mut Manifest,
    notices: &'a mut Vec<Notice>,
    bytes: u64,
}

impl Writer<'_> {
    fn put(&mut self, name: &str, bytes: &[u8], compress: bool) -> Result<(), String> {
        self.bytes += bytes.len() as u64;
        if self.bytes > LIMITS.total_bytes {
            return Err(UiMsg::VnWorkTooLarge.to_string());
        }
        let method = if compress {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        self.zip
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(method),
            )
            .map_err(text)?;
        self.zip.write_all(bytes).map_err(text)
    }

    /// 一張圖：讀不到或驗不過＝略過並記提示；回有沒有放進包
    fn image(&mut self, asset: Asset, from: &Path) -> Result<bool, String> {
        let name = asset.path();
        let Ok(raw) = std::fs::read(from) else {
            return Ok(false);
        };
        match sanitize_png(&raw) {
            Ok((clean, _)) if clean.len() as u64 <= LIMITS.png_bytes => {
                self.put(&name, &clean, false)?;
                self.manifest.assets.push(name);
                Ok(true)
            }
            _ => {
                self.notices.push(Notice {
                    kind: NoticeKind::ImageSkipped,
                    target: name,
                });
                Ok(false)
            }
        }
    }
}

/// 目錄下合格的 `<stem>.png`（stem 照 `parse` 的同一套規則驗）
fn pngs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".png")?.to_owned();
            entry
                .file_type()
                .ok()?
                .is_file()
                .then(|| (stem, entry.path()))
        })
        .collect();
    found.sort();
    found
}

#[allow(clippy::too_many_arguments)]
fn write_pack(
    root: &Path,
    world_id: &str,
    part: &Path,
    cover: Option<&Path>,
    manifest: &mut Manifest,
    script: &Script,
    cast: &Characters,
    settings: &[(String, Vec<u8>)],
    notices: &mut Vec<Notice>,
) -> Result<(), String> {
    let file = File::create(part).map_err(text)?;
    let mut out = Writer {
        zip: ZipWriter::new(BufWriter::new(file)),
        manifest,
        notices,
        bytes: 0,
    };
    for member in &cast.characters {
        sprite(root, world_id, &member.id, &mut out)?;
        let card = data::character_path(root, world_id, &member.id).map_err(text)?;
        out.image(
            Asset::CharacterImage {
                character: member.id.clone(),
            },
            &card.with_extension("png"),
        )?;
        out.image(
            Asset::CharacterAvatar {
                character: member.id.clone(),
            },
            &card.with_extension("avatar.png"),
        )?;
    }
    let mut first_background = None;
    for (stem, from) in pngs(&data::vn_background_dir(root, world_id).map_err(text)?) {
        let Some(asset) = Asset::parse(&format!("assets/backgrounds/{stem}.png")) else {
            continue;
        };
        if out.image(asset, &from)? && first_background.is_none() {
            first_background = Some(from);
        }
    }
    // 封面：玩家挑的 → 桌上的 → 第一張背景
    let table_cover = vn::cover_path(root, world_id).map_err(text)?;
    for from in [
        cover.map(Path::to_path_buf),
        Some(table_cover),
        first_background,
    ]
    .into_iter()
    .flatten()
    {
        if from.is_file() && out.image(Asset::Cover, &from)? {
            break;
        }
    }
    let images = out.manifest.assets.len();
    if images > LIMITS.images {
        return Err(UiMsg::VnWorkTooLarge.to_string());
    }
    for (name, bytes) in settings {
        out.put(name, bytes, true)?;
    }
    out.put(SCRIPT, &serde_json::to_vec(script).map_err(text)?, true)?;
    out.put(
        CHARACTERS,
        &serde_json::to_vec_pretty(cast).map_err(text)?,
        true,
    )?;
    let manifest_json = serde_json::to_vec_pretty(&*out.manifest).map_err(text)?;
    out.put(MANIFEST, &manifest_json, true)?;
    let file = out
        .zip
        .finish()
        .map_err(text)?
        .into_inner()
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(text)
}

/// 一位角色的立繪：定稿驗過才帶 sprite.json 與其餘身體、臉層
fn sprite(
    root: &Path,
    world_id: &str,
    character: &str,
    out: &mut Writer<'_>,
) -> Result<(), String> {
    let dir = data::vn_sprite_dir(root, world_id, character).map_err(text)?;
    let Ok(raw) = std::fs::read(dir.join("sprite.json")) else {
        return Ok(());
    };
    let Ok(meta) = serde_json::from_slice::<crate::vn_sprites::store::SpriteMeta>(&raw) else {
        return Ok(());
    };
    let id = character.to_owned();
    if !out.image(
        Asset::SpriteBody {
            character: id.clone(),
            key: "base".to_owned(),
        },
        &dir.join("body").join("base.png"),
    )? {
        return Ok(());
    }
    let meta_name = Asset::SpriteMeta {
        character: id.clone(),
    }
    .path();
    out.put(
        &meta_name,
        &serde_json::to_vec_pretty(&meta).map_err(text)?,
        true,
    )?;
    out.manifest.assets.push(meta_name);
    for (stem, from) in pngs(&dir.join("body")) {
        if stem != "base" {
            if let Some(asset) = Asset::parse(&format!("assets/sprites/{id}/body/{stem}.png")) {
                out.image(asset, &from)?;
            }
        }
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    let mut sets: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name == "face" || name.starts_with("face-"))
        .collect();
    sets.sort();
    for set in sets {
        for (stem, from) in pngs(&dir.join(&set)) {
            if let Some(asset) = Asset::parse(&format!("assets/sprites/{id}/{set}/{stem}.png")) {
                out.image(asset, &from)?;
            }
        }
    }
    Ok(())
}
