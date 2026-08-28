use crate::mechanism::{self, Outcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use super::{DataResult, invalid_data, local_timestamp};
use super::character::{list_characters, set_character_auto_hidden};
use super::paths::world_dir;
use super::state::{SceneLabel, TableState, WorldState, read_state, write_state};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptKind {
    Dialogue,
    Narration,
    Player,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub ts: String,
    /// 角色事件存角色 id；GM 旁白／系統訊息／玩家發言存空字串（kind 已足以區分）
    pub speaker_id: String,
    /// 發言當下的顯示名快照——改名後舊事件不動，這是既有拍板行為
    pub speaker_name: String,
    pub kind: TranscriptKind,
    pub text: String,
    /// 剝殼前的模型原文：狀態區塊與點名行都還在，供卡片自帶的面板重畫歷史訊息用。
    /// 與 text 相同（沒剝到東西）時不存，舊檔沒有這欄也照樣讀得起來。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<TableState>,
    /// 這則系統事件的全文只給 GM 看；chars 續聊線遇到只留第一行（AI 卡重構包 4b，
    /// 補 4a 遺留的 visibility 洩漏——非 Public 世界書人物的登場全文不該流進扮演引擎）。
    #[serde(default)]
    pub gm_only: bool,
}

/// 沒進 scene_labels 的幕＝原線（舊存檔也走這條）：顯示編號就是內部幕號，第 1 版，上一幕是前一號。
pub fn scene_label(state: &WorldState, scene: u64) -> SceneLabel {
    state
        .scene_labels
        .get(&scene.to_string())
        .copied()
        .unwrap_or(SceneLabel {
            base: scene,
            version: 1,
            parent: scene.checked_sub(1),
            forked: false,
        })
}

fn transcript_path(root: &Path, world_id: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world_id)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

pub fn append_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<()> {
    let mut event = event.clone();
    if event.state.is_none() {
        // 復原舊句子會帶回當時快照，只有新事件才借用目前檯面。
        event.state = read_state(root, world_id).ok().map(|state| state.state);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world_id, scene)?)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;
    // 目前值恆等於最後一則事件的快照，復原舊句時狀態才會跟著回到那一刻。
    // 快取寫失敗不該把「事件已經寫進去了」這件事變成錯誤，權威在 transcript。
    if let Some(snapshot) = event.state {
        if let Ok(mut world) = read_state(root, world_id) {
            if world.state != snapshot {
                world.state = snapshot;
                let _ = write_state(root, world_id, &world);
            }
        }
    }
    Ok(())
}

/// 開場白也要存成快照，收回時檯面才能回到貼上前的最後一句；狀態區塊走與 GM 回覆同一條
/// 本地權威（mechanism::apply_block），增量桌的數值一開場就是本機在算。
pub fn append_opening(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    raw: &str,
    block: &crate::transport::StateBlock,
    user_name: &str,
) -> DataResult<(TranscriptEvent, Outcome)> {
    let mut world = read_state(root, world_id)?;
    let outcome = mechanism::apply_block(&mut world, block, user_name);
    let event = TranscriptEvent {
        ts: ts.to_owned(),
        speaker_id: String::new(),
        speaker_name: "GM".to_owned(),
        kind: TranscriptKind::Narration,
        text: block.display.clone(),
        raw: (raw != block.display).then(|| raw.to_owned()),
        state: Some(world.state),
        gm_only: false,
    };
    append_transcript(root, world_id, scene, &event)?;
    Ok((event, outcome))
}

/// 整檔重寫這一幕，並把檯面退回剩下事件的最後一份快照（這一幕沒了就往前一幕找）。
/// 刪事件的兩條路（收回上一句、復原匯入收掉開場白）共用。
fn rewrite_scene(
    root: &Path,
    world_id: &str,
    scene: u64,
    events: &[TranscriptEvent],
) -> DataResult<()> {
    let mut buffer = String::new();
    for event in events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    let mut state = read_state(root, world_id)?;
    state.state = events
        .iter()
        .rev()
        .find_map(|entry| entry.state.clone())
        .or_else(|| {
            scene.checked_sub(1).and_then(|previous_scene| {
                read_transcript(root, world_id, previous_scene)
                    .ok()
                    .and_then(|previous_events| {
                        previous_events
                            .iter()
                            .rev()
                            .find_map(|entry| entry.state.clone())
                    })
            })
        })
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(())
}

/// 狀態樹被逐字稿以外的路徑換掉（重構套用重建欄位）之後，把新樹補進這一幕每一則事件的快照。
/// 收回上一句與換幕都拿事件快照當回捲基準，不補的話玩家一收回，介面就被打回重構前的舊欄位。
/// 補整幕而不是只補最後一則：連按收回會一路往前吃，任何一則留著舊欄位都會在那一下現形。
/// 只換 tree／jumps——劇情面的欄位（table、changes、notes）照舊跟著各自那一刻走。
pub fn sync_scene_state_tree(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    let scene = state.current_scene;
    let mut events = read_transcript(root, world_id, scene)?;
    let mut touched = false;
    for event in events.iter_mut() {
        let Some(snapshot) = event.state.as_mut() else {
            continue;
        };
        if snapshot.tree != state.state.tree || snapshot.jumps != state.state.jumps {
            snapshot.tree = state.state.tree.clone();
            snapshot.jumps = state.state.jumps.clone();
            touched = true;
        }
    }
    if touched {
        rewrite_scene(root, world_id, scene, &events)?;
    }
    Ok(())
}

/// 收回上一句（可連按）：砍掉這一幕最後一筆事件後整檔重寫。
/// 回傳是否真的刪了——這一幕已經空了就是 false，收不會倒退咬到上一幕。
pub fn pop_transcript(root: &Path, world_id: &str, scene: u64) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    if events.pop().is_none() {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

/// 復原匯入用：從這一幕刪掉時間戳相符的那一則（貼出的開場白），其餘事件原位不動。
/// 回傳是否真的刪到——玩家自己先收回過就是 false。
pub fn remove_transcript_event(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let before = events.len();
    events.retain(|event| event.ts != ts);
    if events.len() == before {
        return Ok(false);
    }
    rewrite_scene(root, world_id, scene, &events)?;
    Ok(true)
}

pub fn set_last_transcript_state(
    root: &Path,
    world_id: &str,
    scene: u64,
    state: &TableState,
) -> DataResult<bool> {
    let mut events = read_transcript(root, world_id, scene)?;
    let Some(entry) = events.last_mut() else {
        return Ok(false);
    };
    entry.state = Some(state.clone());
    let mut buffer = String::new();
    for entry in &events {
        buffer.push_str(&serde_json::to_string(entry)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, scene)?, buffer)?;
    Ok(true)
}

pub fn read_transcript(
    root: &Path,
    world_id: &str,
    scene: u64,
) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut events = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let event = serde_json::from_str(&line).map_err(|error| {
            invalid_data(format!("invalid transcript line {line_number}: {error}"))
        })?;
        events.push(event);
    }
    Ok(events)
}

// ---------------------------------------------------------------------
// AI 卡重構包 4b：角色卡自動上下場共用的登場掃描原語。人物（transport::PERSON_ARRIVAL_PREFIX）
// 與角色卡（CARD_ARRIVAL_PREFIX）登場比對邏輯相同、鍵不同，這裡放兩邊都用得到、且
// 換幕結算（本檔 begin_next_scene）必須直接呼叫、不能反過來依賴 transport 的最小共用集合。
// ---------------------------------------------------------------------

/// 角色卡回歸事件的固定前綴，接著是〈name〉那一行——跟世界書人物的登場前綴
/// （transport::PERSON_ARRIVAL_PREFIX）分開，掃 transcript 或前端呈現時才分得出兩種來源。
pub const CARD_ARRIVAL_PREFIX: &str = "（角色回歸）";

/// 從一則事件文字剝出前綴後的〈title〉；prefix 不符或沒有〈〉包住就回 None。
pub(crate) fn bracket_title(text: &str, prefix: &str) -> Option<String> {
    let rest = text.strip_prefix(prefix)?.strip_prefix('〈')?;
    let end = rest.find('〉')?;
    Some(rest[..end].to_owned())
}

/// 本幕已登場（依指定前綴）集合：掃 System 事件取出〈title〉。
pub(crate) fn appeared_titles(events: &[TranscriptEvent], prefix: &str) -> BTreeSet<String> {
    events
        .iter()
        .filter(|event| event.kind == TranscriptKind::System)
        .filter_map(|event| bracket_title(&event.text, prefix))
        .collect()
}

/// present 欄的斷詞規則：頓號／逗號／斜線／分號，trim 後濾空。
pub(crate) fn split_present_names(raw: &str) -> Vec<String> {
    raw.split(['、', '，', ',', '／', '/', '；', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 在場名字跟標題比對：雙向包含，「亞歷山大」對得上「亞歷山大・馮・史特勞斯」。
pub(crate) fn name_matches(name: &str, title: &str) -> bool {
    title.contains(name) || name.contains(title)
}

/// 把單一事件渲染成一行（或多行）Markdown，整桌／單場匯出共用同一份格式。
fn render_transcript_entry(event: &TranscriptEvent, english: bool) -> String {
    match event.kind {
        TranscriptKind::Dialogue | TranscriptKind::Player => {
            if english {
                format!("**{}**: {}", event.speaker_name, event.text)
            } else {
                format!("**{}**：{}", event.speaker_name, event.text)
            }
        }
        TranscriptKind::Narration => {
            if event.text.is_empty() {
                "> ".to_owned()
            } else {
                event
                    .text
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        TranscriptKind::System => {
            if english {
                format!("*({})*", event.text)
            } else {
                format!("*（{}）*", event.text)
            }
        }
    }
}

/// 場景標題＋事件列表組成一段章節，整桌匯出把多段章節接起來。
fn render_scene_section(events: &[TranscriptEvent], heading: &str, english: bool) -> String {
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        heading.to_owned()
    } else {
        format!("{heading}\n\n{}", entries.join("\n\n"))
    }
}

pub fn export_transcript_markdown(root: &Path, world_id: &str, lang: &str) -> DataResult<String> {
    let world_name = read_state(root, world_id)?.name;
    let transcript_dir = world_dir(root, world_id)?.join("transcript");
    if !transcript_dir.is_dir() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let mut scenes = Vec::new();
    for entry in fs::read_dir(transcript_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if let Ok(scene) = stem.parse::<u64>() {
            scenes.push(scene);
        }
    }
    scenes.sort_unstable();
    scenes.dedup();
    if scenes.is_empty() {
        return Err(invalid_data("這桌還沒有任何紀錄"));
    }

    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world_name} — Session Transcript\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 跑團紀錄\n\n匯出時間：{timestamp}")
    };
    let mut sections = Vec::new();
    for scene in scenes {
        let heading = if english {
            format!("## Scene {scene}")
        } else {
            format!("## 場景 {scene}")
        };
        let events = read_transcript(root, world_id, scene)?;
        sections.push(render_scene_section(&events, &heading, english));
    }

    Ok(format!("{title}\n\n{}\n", sections.join("\n\n")))
}

/// 匯出單一場景的紀錄，格式與整桌匯出一致，供「過去的場」單場匯出使用。
/// 場景不存在（無該檔）視為錯誤，避免誤匯出空白文件。
pub fn export_scene_markdown(
    root: &Path,
    world_id: &str,
    scene: u64,
    lang: &str,
) -> DataResult<String> {
    let path = transcript_path(root, world_id, scene)?;
    if !path.exists() {
        return Err(invalid_data(format!("場景 {scene} 不存在")));
    }

    let world_name = read_state(root, world_id)?.name;
    let english = lang == "en";
    let timestamp = local_timestamp()?;
    let title = if english {
        format!("# {world_name} — Scene {scene}\n\nExported: {timestamp}")
    } else {
        format!("# {world_name} 場景 {scene}\n\n匯出時間：{timestamp}")
    };
    let events = read_transcript(root, world_id, scene)?;
    let entries = events
        .iter()
        .map(|event| render_transcript_entry(event, english))
        .collect::<Vec<_>>();
    Ok(format!("{title}\n\n{}\n", entries.join("\n\n")))
}

/// 換幕摘要固定前綴：新幕開頭與重寫前情提要共用同一套語系文案，避免兩處各自維護。
fn format_scene_summary(summary_text: &str, lang: &str) -> String {
    if lang == "en" {
        format!("Previously:\n{summary_text}")
    } else {
        format!("【前情提要】\n{summary_text}")
    }
}

/// 算「某個 base 目前該排第幾個版本」：掃 0..=upto 每一幕的顯示 base，數出撞號的幕數再 +1。
/// begin_next_scene 與 fork_scene 都靠它算新標籤，掃描範圍在插入新標籤之前的呼叫端已經固定。
fn next_scene_version(state: &WorldState, upto: u64, base: u64) -> u32 {
    (0..=upto)
        .filter(|&scene| scene_label(state, scene).base == base)
        .count() as u32
        + 1
}

/// 分岔：把某一幕的紀錄原樣複製成新的一幕接著玩，原本歷史一個字都不動。
/// 顯示編號跟隨來源幕（從分岔幕再分岔＝跟著源頭走，不是跟著內部號走），
/// parent 記分岔當下所在的幕，退回時回到這裡而不是來源幕。
pub fn fork_scene(root: &Path, world_id: &str, from_scene: u64) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    if from_scene >= state.current_scene {
        return Err(invalid_data("只能從前面的幕分岔"));
    }
    let events = read_transcript(root, world_id, from_scene)?;
    if events.is_empty() {
        return Err(invalid_data("這一幕沒有紀錄可以接續"));
    }

    let current_scene = state.current_scene;
    let new_scene = current_scene + 1;
    let mut buffer = String::new();
    for event in &events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    fs::write(transcript_path(root, world_id, new_scene)?, buffer)?;

    let base = scene_label(&state, from_scene).base;
    let version = next_scene_version(&state, current_scene, base);
    state.scene_labels.insert(
        new_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(current_scene),
            forked: true,
        },
    );
    state.current_scene = new_scene;
    state.state = events
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(new_scene)
}

/// 換場：把摘要包成一則 GM 旁白 append 到下一場景開頭，再把 current_scene +1 並存檔。
/// 回傳新場景號。摘要文字本身由呼叫端（單發 LLM）產生，這裡只負責落地與推進場次。
/// title 有值就存進「舊場景」（bump 前的 current_scene）的 scene_titles，與場次 +1 同一次 write_state。
pub fn begin_next_scene(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    lang: &str,
    title: Option<&str>,
) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    let old_scene = state.current_scene;
    let next_scene = old_scene + 1;
    append_transcript(
        root,
        world_id,
        next_scene,
        &TranscriptEvent {
            raw: None,
            ts: local_timestamp()?,
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: format_scene_summary(summary_text, lang),
            state: None,
            gm_only: false,
        },
    )?;
    if let Some(name) = title.map(str::trim).filter(|name| !name.is_empty()) {
        state
            .scene_titles
            .insert(old_scene.to_string(), name.to_owned());
    }
    let base = scene_label(&state, old_scene).base + 1;
    let version = next_scene_version(&state, old_scene, base);
    state.scene_labels.insert(
        next_scene.to_string(),
        SceneLabel {
            base,
            version,
            parent: Some(old_scene),
            forked: false,
        },
    );
    state.current_scene = next_scene;
    write_state(root, world_id, &state)?;
    settle_card_visibility(
        root,
        world_id,
        old_scene,
        state.state.table.get("present").map(String::as_str),
    );
    Ok(next_scene)
}

/// 換幕結算角色卡自動隱藏（AI 卡重構包 4b；鐵律：auto_hidden 這個持久欄位只在換幕動，
/// 幕中回合的登場偵測只 append 事件不改欄位，見 lib.rs record_card_arrivals）。
///
/// 出現過＝(a) 剛結束那幕的角色卡回歸事件集合 ∪ (b) 換幕當下 present 名單比對命中。
/// 出現過→auto_hidden=false（拉回主區）；沒出現過→auto_hidden=true（收進隱藏區）；
/// archived（手動封存）的卡完全不動，自動判斷不能覆蓋玩家的手動決定。
///
/// 已知限制：幕開始就在主區、全程活躍，但最後一輪 GM 忘記把它列進 present、
/// 這幕本文也沒有登場事件（因為它本來就沒被隱藏過）的卡，會在這裡被判定「沒出現」
/// 而轉為隱藏——(a)(b) 都掃不到這種情況；真正掃「正文有沒有提到名字」(c) 成本較高
/// （要跑完整幕全部旁白文字），先不做，之後真的常誤判再考慮補。
///
/// 結算失敗一律吞掉：換幕本身已經成功，auto_hidden 記帳不該反過來讓換幕報錯。
fn settle_card_visibility(root: &Path, world_id: &str, ended_scene: u64, present: Option<&str>) {
    let Ok(characters) = list_characters(root, world_id) else {
        return;
    };
    let events = read_transcript(root, world_id, ended_scene).unwrap_or_default();
    let arrived = appeared_titles(&events, CARD_ARRIVAL_PREFIX);
    let present_names = present.map(split_present_names);
    for meta in characters {
        if meta.archived {
            continue;
        }
        let appeared = arrived.iter().any(|name| name_matches(name, &meta.name))
            || present_names
                .as_ref()
                .is_some_and(|names| names.iter().any(|name| name_matches(name, &meta.name)));
        let _ = set_character_auto_hidden(root, world_id, &meta.id, !appeared);
    }
}

/// 退回前幕：換幕的精確反向操作，純本地檔案處理不必呼叫模型。
/// 前一幕看 scene_labels 的 parent（原線／分岔都適用），不再假設一定是「幕號 -1」。
/// 只認「這一幕剛好一則事件」——begin_next_scene 保證新幕開頭就是那則摘要，
/// 多於一則代表玩家已經在這一幕行動過，退回會悄悄吃掉那些內容，所以直接擋，
/// 且擋下時故意先不動任何檔案／狀態（讀完才判斷），錯誤路徑不留副作用。
pub fn revert_scene(root: &Path, world_id: &str) -> DataResult<u64> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let Some(previous_scene) = scene_label(&state, scene).parent else {
        return Err(invalid_data("已經是第一幕，沒有前幕可以退回"));
    };
    let events = read_transcript(root, world_id, scene)?;
    if events.len() != 1 {
        return Err(invalid_data("這一幕已經有新內容，不能退回前幕"));
    }

    fs::remove_file(transcript_path(root, world_id, scene)?)?;
    state.current_scene = previous_scene;
    state.scene_titles.remove(&previous_scene.to_string());
    // 自己這筆標籤跟著檔案一起消失，不留退回後查不到來源、卻還佔著 key 的殭屍紀錄。
    state.scene_labels.remove(&scene.to_string());
    // current_scene 落回前幕，前幕本來就對齊過了，aligned_scene 不用跟著動。
    state.state = read_transcript(root, world_id, previous_scene)?
        .iter()
        .rev()
        .find_map(|event| event.state.clone())
        .unwrap_or_default();
    write_state(root, world_id, &state)?;
    Ok(previous_scene)
}

/// 重寫目前這幕唯一那則摘要：摘要不滿意可以直接原地覆寫，不必先退回再重新換幕一次。
pub fn replace_scene_summary(
    root: &Path,
    world_id: &str,
    summary_text: &str,
    lang: &str,
    title: Option<&str>,
) -> DataResult<()> {
    let mut state = read_state(root, world_id)?;
    let scene = state.current_scene;
    let label = scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(invalid_data("第一幕沒有前情提要可以重寫"));
    };
    // 分岔幕開頭那則是複製來的真實對話，不是摘要。源頭幕剛好只有一則時
    // 「只有一則」這道守門會放行，覆寫下去就把玩家的對話換成摘要了。
    if label.forked {
        return Err(invalid_data("這一幕是從前幕接續來的，開頭不是前情提要"));
    }
    let mut events = read_transcript(root, world_id, scene)?;
    if events.len() != 1 {
        return Err(invalid_data("這一幕已經有新內容，不能重寫前情提要"));
    }

    // 重寫的只有文字，其餘欄位原樣留著——尤其 state 那份快照：
    // 摘要是這一幕唯一一則，快照掉了之後退回這一幕會把狀態欄清成空的。
    let event = &mut events[0];
    event.text = format_scene_summary(summary_text, lang);
    event.ts = local_timestamp()?;
    fs::write(
        transcript_path(root, world_id, scene)?,
        format!("{}\n", serde_json::to_string(event)?),
    )?;

    match title.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            state
                .scene_titles
                .insert(previous_scene.to_string(), name.to_owned());
        }
        None => {
            state.scene_titles.remove(&previous_scene.to_string());
        }
    }
    write_state(root, world_id, &state)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;
    use std::collections::BTreeMap;

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        let world_id = create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
                state: None,
                gm_only: false,
            },
            TranscriptEvent {
                raw: None,
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker_id: "角色代碼".to_owned(),
                speaker_name: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
                state: None,
                gm_only: false,
            },
        ];
        for event in &events {
            append_transcript(root.path(), &world_id, 7, event).unwrap();
        }
        let expected: Vec<_> = events
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 7).unwrap(),
            expected
        );

        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/7.jsonl"));
        let raw = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = raw.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value.is_object());
            assert!(["dialogue", "narration", "player", "system"]
                .contains(&value["kind"].as_str().unwrap()));
        }

        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"ts\":\"now\",\"speaker_id\":\"\",\"speaker_name\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), &world_id, 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn pop_transcript_removes_last_event_until_scene_is_empty() {
        let root = TestRoot::new("transcript-pop");
        let world_id = create_world(root.path(), "收回桌").unwrap();
        let events: Vec<TranscriptEvent> = ["序幕", "我推開門", "誰在那裡？"]
            .iter()
            .enumerate()
            .map(|(index, text)| TranscriptEvent {
                raw: None,
                ts: format!("2026-08-01T10:00:0{index}+08:00"),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: (*text).to_owned(),
                state: None,
                gm_only: false,
            })
            .collect();
        for event in &events {
            append_transcript(root.path(), &world_id, 0, event).unwrap();
        }

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        let expected: Vec<_> = events[..2]
            .iter()
            .cloned()
            .map(|mut event| {
                event.state = Some(TableState::default());
                event
            })
            .collect();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            expected
        );
        // 重寫後仍是合法 JSONL：行數對齊事件數，沒有殘留的半行
        let path = root
            .path()
            .join(format!("worlds/{world_id}/transcript/0.jsonl"));
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);

        // 連按到底：收乾淨後再按回 false，不會倒退咬到別的幕
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(!pop_transcript(root.path(), &world_id, 0).unwrap());
        assert!(read_transcript(root.path(), &world_id, 0)
            .unwrap()
            .is_empty());

        // 沒開始過的幕：不建檔也不報錯
        assert!(!pop_transcript(root.path(), &world_id, 9).unwrap());
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/9.jsonl"))
            .exists());
    }

    #[test]
    fn append_transcript_uses_current_snapshot_without_overwriting_supplied_state() {
        let root = TestRoot::new("transcript-state-snapshot");
        let world_id = create_world(root.path(), "狀態桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("time".to_owned(), "清晨".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: "第一句".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[0]
                .state
                .as_ref()
                .unwrap()
                .table
                .get("time"),
            Some(&"清晨".to_owned())
        );

        let supplied = TableState {
            table: BTreeMap::from([("time".to_owned(), "午夜".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "later".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "第二句".to_owned(),
                state: Some(supplied.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1].state,
            Some(supplied)
        );
    }

    #[test]
    fn append_opening_skips_raw_when_nothing_was_stripped() {
        let root = TestRoot::new("opening-raw");
        let world_id = create_world(root.path(), "純正文桌").unwrap();
        let raw = "只有旁白，沒有狀態欄。";
        let (event, _) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert_eq!(event.text, raw);
        assert_eq!(event.raw, None);
        // 舊檔沒有 raw 欄位也讀得起來，序列化時同樣不憑空多一欄
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains("\"raw\""));
    }

    #[test]
    fn append_opening_merges_state_and_pop_restores_previous_snapshot() {
        let root = TestRoot::new("opening-state");
        let world_id = create_world(root.path(), "開場狀態桌").unwrap();
        let previous = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "before".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "前一則".to_owned(),
                state: Some(previous.clone()),
                gm_only: false,
            },
        )
        .unwrap();

        let raw = "開場旁白<status>place: 碼頭\ntime: 午夜</status>";
        let (event, outcome) = append_opening(
            root.path(),
            &world_id,
            0,
            "opening",
            raw,
            &crate::transport::extract_state_block(raw),
            "阿濤",
        )
        .unwrap();
        assert!(outcome.records.is_empty());
        // 畫面只留正文，模型原文整段另存一份（面板要靠它重畫歷史訊息）
        assert_eq!(event.text, "開場旁白");
        assert_eq!(event.raw.as_deref(), Some(raw));
        let expected = TableState {
            table: BTreeMap::from([
                ("place".to_owned(), "碼頭".to_owned()),
                ("time".to_owned(), "午夜".to_owned()),
            ]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        assert_eq!(event.state, Some(expected.clone()));
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, expected);
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap()[1],
            event
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, previous);
    }

    #[test]
    fn pop_transcript_restores_the_previous_event_snapshot() {
        let root = TestRoot::new("transcript-state-pop");
        let world_id = create_world(root.path(), "回收狀態桌").unwrap();
        let first = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::from([("place".to_owned(), "碼頭".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for (text, snapshot) in [("第一句", first.clone()), ("第二句", second.clone())] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: text.to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = second;
        write_state(root.path(), &world_id, &state).unwrap();

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

    /// 復原＝把帶著自身快照的舊事件原樣寫回，目前值要跟著回到那一刻
    /// （否則狀態欄會停在收回後的舊值，跟桌上最後一句對不起來）
    #[test]
    fn restoring_an_undone_event_puts_its_snapshot_back() {
        let root = TestRoot::new("transcript-state-restore");
        let world_id = create_world(root.path(), "復原狀態桌").unwrap();
        let snapshots = ["清晨", "午夜"].map(|time| TableState {
            table: BTreeMap::from([("time".to_owned(), time.to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        });
        let event = |text: &str, snapshot: &TableState| TranscriptEvent {
            raw: None,
            ts: "now".to_owned(),
            speaker_id: String::new(),
            speaker_name: "GM".to_owned(),
            kind: TranscriptKind::Narration,
            text: text.to_owned(),
            state: Some(snapshot.clone()),
            gm_only: false,
        };
        for (text, snapshot) in [("第一句", &snapshots[0]), ("第二句", &snapshots[1])] {
            append_transcript(root.path(), &world_id, 0, &event(text, snapshot)).unwrap();
        }
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );

        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[0]
        );

        append_transcript(root.path(), &world_id, 0, &event("第二句", &snapshots[1])).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id).unwrap().state,
            snapshots[1]
        );
    }

    #[test]
    fn exports_all_transcript_scenes_as_localized_markdown() {
        let root = TestRoot::new("transcript-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。\n港口安靜。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "玩家".to_owned(),
                    kind: TranscriptKind::Player,
                    text: "我登上甲板。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::System,
                    text: "第一幕開始".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_transcript_markdown(root.path(), &world_id, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 跑團紀錄\n\n匯出時間："));
        assert!(zh.find("## 場景 0").unwrap() < zh.find("## 場景 1").unwrap());
        assert!(zh.contains("> 霧氣升起。\n> 港口安靜。"));
        assert!(zh.contains("*（第一幕開始）*"));
        assert!(zh.contains("**玩家**：我登上甲板。"));
        assert!(zh.contains("**船長**：我們啟航。"));

        let en = export_transcript_markdown(root.path(), &world_id, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Session Transcript\n\nExported: "));
        assert!(en.contains("## Scene 0"));
        assert!(en.contains("## Scene 1"));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(en.contains("*(第一幕開始)*"));
    }

    #[test]
    fn transcript_export_rejects_a_world_without_scenes() {
        let root = TestRoot::new("empty-transcript-export");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_transcript_markdown(root.path(), &world_id, "zh-TW").is_err());
    }

    #[test]
    fn scene_export_contains_only_that_scenes_events() {
        let root = TestRoot::new("scene-export");
        let world_id = create_world(root.path(), "海風桌").unwrap();
        for (scene, event) in [
            (
                0,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "霧氣升起。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
            (
                1,
                TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: "船長代碼".to_owned(),
                    speaker_name: "船長".to_owned(),
                    kind: TranscriptKind::Dialogue,
                    text: "我們啟航。".to_owned(),
                    state: None,
                    gm_only: false,
                },
            ),
        ] {
            append_transcript(root.path(), &world_id, scene, &event).unwrap();
        }

        let zh = export_scene_markdown(root.path(), &world_id, 0, "zh-TW").unwrap();
        assert!(zh.starts_with("# 海風桌 場景 0\n\n匯出時間："));
        assert!(zh.contains("> 霧氣升起。"));
        assert!(!zh.contains("船長"));

        let en = export_scene_markdown(root.path(), &world_id, 1, "en").unwrap();
        assert!(en.starts_with("# 海風桌 — Scene 1\n\nExported: "));
        assert!(en.contains("**船長**: 我們啟航。"));
        assert!(!en.contains("霧氣升起"));
    }

    #[test]
    fn scene_export_rejects_a_missing_scene() {
        let root = TestRoot::new("scene-export-missing");
        let world_id = create_world(root.path(), "空桌").unwrap();
        assert!(export_scene_markdown(root.path(), &world_id, 0, "zh-TW").is_err());
    }

    #[test]
    fn begin_next_scene_appends_summary_and_advances_scene() {
        let root = TestRoot::new("begin-next-scene");
        let world_id = create_world(root.path(), "換場桌").unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一場的對話".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        let next = begin_next_scene(root.path(), &world_id, "壓縮後的摘要", "zh-TW", None).unwrap();
        assert_eq!(next, 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        // 摘要落在新場景檔開頭，舊場景不受影響
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
        let new_scene = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(new_scene.len(), 1);
        assert_eq!(new_scene[0].speaker_name, "GM");
        assert_eq!(new_scene[0].speaker_id, "");
        assert_eq!(new_scene[0].kind, TranscriptKind::Narration);
        assert_eq!(new_scene[0].text, "【前情提要】\n壓縮後的摘要");

        // en 語系用英文前綴
        let next_en = begin_next_scene(root.path(), &world_id, "recap text", "en", None).unwrap();
        assert_eq!(next_en, 2);
        let scene_two = read_transcript(root.path(), &world_id, 2).unwrap();
        assert_eq!(scene_two[0].text, "Previously:\nrecap text");
    }

    #[test]
    fn begin_next_scene_stores_title_on_old_scene_when_given() {
        let root = TestRoot::new("begin-next-scene-title");
        let world_id = create_world(root.path(), "取名桌").unwrap();
        let event = TranscriptEvent {
            raw: None,
            ts: "2026-07-24T00:00:00Z".to_owned(),
            speaker_id: String::new(),
            speaker_name: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "第一幕的對話".to_owned(),
            state: None,
            gm_only: false,
        };
        append_transcript(root.path(), &world_id, 0, &event).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", Some("酒館夜話")).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 1);
        assert_eq!(
            state.scene_titles.get("0").map(String::as_str),
            Some("酒館夜話")
        );
        assert!(!state.scene_titles.contains_key("1"));

        // 空字串／None 都不進表
        begin_next_scene(root.path(), &world_id, "摘要二", "zh-TW", Some("   ")).unwrap();
        begin_next_scene(root.path(), &world_id, "摘要三", "zh-TW", None).unwrap();
        let state = read_state(root.path(), &world_id).unwrap();
        assert!(!state.scene_titles.contains_key("1"));
        assert!(!state.scene_titles.contains_key("2"));
    }

    #[test]
    fn revert_scene_returns_to_previous_scene_and_drops_title() {
        let root = TestRoot::new("revert-scene");
        let world_id = create_world(root.path(), "退幕桌").unwrap();
        let snapshot = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一幕的對話".to_owned(),
                state: Some(snapshot.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", Some("酒館夜話")).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 1);

        let previous = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(previous, 0);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 0);
        // 前幕最後一則帶快照事件的 state 要跟著回來，不是砍完就放著預設值
        assert_eq!(state.state, snapshot);
        assert!(!state.scene_titles.contains_key("0"));
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/1.jsonl"))
            .exists());
        // 舊幕本身完全沒被動過
        assert_eq!(read_transcript(root.path(), &world_id, 0).unwrap().len(), 1);
    }

    #[test]
    fn revert_scene_rejects_extra_events_without_touching_anything() {
        let root = TestRoot::new("revert-scene-blocked");
        let world_id = create_world(root.path(), "退幕擋桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", None).unwrap();
        // 這一幕除了摘要之外，玩家已經多說了一句——不是「剛好一則」了
        append_transcript(
            root.path(),
            &world_id,
            1,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "新的一句".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let before_state = read_state(root.path(), &world_id).unwrap();
        let before_events = read_transcript(root.path(), &world_id, 1).unwrap();

        let error = revert_scene(root.path(), &world_id).unwrap_err().to_string();
        assert!(error.contains("不能退回前幕"));

        // 擋下時檔案與 state 都沒被動過
        assert_eq!(read_state(root.path(), &world_id).unwrap(), before_state);
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            before_events
        );
    }

    #[test]
    fn revert_scene_rejects_at_first_scene() {
        let root = TestRoot::new("revert-scene-first");
        let world_id = create_world(root.path(), "第一幕桌").unwrap();
        let error = revert_scene(root.path(), &world_id).unwrap_err().to_string();
        assert!(error.contains("沒有前幕可以退回"));
    }

    #[test]
    fn replace_scene_summary_overwrites_text_and_drops_title_when_none() {
        let root = TestRoot::new("replace-scene-summary");
        let world_id = create_world(root.path(), "重寫摘要桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "舊摘要", "zh-TW", Some("舊標題")).unwrap();
        assert_eq!(
            read_state(root.path(), &world_id)
                .unwrap()
                .scene_titles
                .get("0")
                .map(String::as_str),
            Some("舊標題")
        );

        replace_scene_summary(root.path(), &world_id, "新摘要", "zh-TW", None).unwrap();

        let events = read_transcript(root.path(), &world_id, 1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "【前情提要】\n新摘要");
        assert_eq!(events[0].speaker_name, "GM");
        assert_eq!(events[0].kind, TranscriptKind::Narration);

        // title 傳 None：舊幕名被移除，不留上一次的殘留
        assert!(!read_state(root.path(), &world_id)
            .unwrap()
            .scene_titles
            .contains_key("0"));
    }

    /// 分岔幕開頭那則是複製來的真實對話，不是前情提要。源頭幕剛好只有一則時，
    /// 「這幕只有一則」那道守門會放行——沒有 forked 這一格擋著，重寫就把玩家的對話覆寫掉了。
    #[test]
    fn replace_scene_summary_refuses_a_forked_scene() {
        let root = TestRoot::new("replace-summary-forked");
        let world_id = create_world(root.path(), "分岔守門桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "玩家的第一句".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", "zh-TW", None).unwrap();

        // 幕 0 只有一則，分岔出來的這一幕同樣只有那一則——正是守門會誤放的形狀
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(
            read_transcript(root.path(), &world_id, forked).unwrap().len(),
            1
        );

        assert!(
            replace_scene_summary(root.path(), &world_id, "不該蓋掉", "zh-TW", None).is_err()
        );
        let events = read_transcript(root.path(), &world_id, forked).unwrap();
        assert_eq!(events[0].text, "玩家的第一句");
    }

    /// 重寫摘要只換文字：那則的狀態快照要留著。摘要是這一幕唯一一則，
    /// 快照掉了的話，之後退回這一幕就只能把狀態欄清成空的。
    #[test]
    fn replace_scene_summary_keeps_snapshot_for_later_revert() {
        let root = TestRoot::new("replace-scene-summary-snapshot");
        let world_id = create_world(root.path(), "快照保留桌").unwrap();
        let snapshot = TableState {
            table: BTreeMap::from([("place".to_owned(), "酒館".to_owned())]),
            tree: BTreeMap::new(),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
                state: Some(snapshot.clone()),
                gm_only: false,
            },
        )
        .unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.state = snapshot.clone();
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "舊摘要", "zh-TW", None).unwrap();
        replace_scene_summary(root.path(), &world_id, "新摘要", "zh-TW", None).unwrap();
        // 再換一幕：這時第 1 幕那則摘要成了回推狀態的唯一來源
        begin_next_scene(root.path(), &world_id, "第二幕摘要", "zh-TW", None).unwrap();

        assert_eq!(revert_scene(root.path(), &world_id).unwrap(), 1);
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, snapshot);
    }

    /// 驗收劇本：原線三幕分岔、續玩換幕、退回吃 parent、再分岔看 version 疊加。
    #[test]
    fn fork_scene_copies_history_and_relabels_through_continue_and_revert() {
        let root = TestRoot::new("fork-scene-scenario");
        let world_id = create_world(root.path(), "分岔桌").unwrap();

        // 原線三幕（內部 0、1、2），人在幕 2
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "啟航前的最後一夜。".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        begin_next_scene(root.path(), &world_id, "第一幕摘要", "zh-TW", None).unwrap();
        begin_next_scene(root.path(), &world_id, "第二幕摘要", "zh-TW", None).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap().current_scene, 2);

        let scene0_before = read_transcript(root.path(), &world_id, 0).unwrap();
        let scene1_before = read_transcript(root.path(), &world_id, 1).unwrap();
        let scene2_before = read_transcript(root.path(), &world_id, 2).unwrap();

        // 從幕 0 分岔
        let forked = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked, 3);
        assert_eq!(
            read_transcript(root.path(), &world_id, 3).unwrap(),
            scene0_before
        );
        // 舊幕一個字都沒被動過
        assert_eq!(
            read_transcript(root.path(), &world_id, 0).unwrap(),
            scene0_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 1).unwrap(),
            scene1_before
        );
        assert_eq!(
            read_transcript(root.path(), &world_id, 2).unwrap(),
            scene2_before
        );

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert_eq!(
            state.scene_labels.get("3").copied(),
            Some(SceneLabel {
                base: 0,
                version: 2,
                parent: Some(2),
                forked: true
            })
        );

        // 在幕 3 續玩一句，再換幕
        append_transcript(
            root.path(),
            &world_id,
            3,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:01:00Z".to_owned(),
                speaker_id: "船長代碼".to_owned(),
                speaker_name: "船長".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "這次我們往南走。".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();
        let advanced =
            begin_next_scene(root.path(), &world_id, "分岔後摘要", "zh-TW", Some("南航夜話"))
                .unwrap();
        assert_eq!(advanced, 4);

        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 1,
                version: 2,
                parent: Some(3),
                forked: false
            })
        );
        assert_eq!(
            state.scene_titles.get("3").map(String::as_str),
            Some("南航夜話")
        );

        // 退回幕 4：回到 parent（3），不是算術上的 4-1=3 巧合——這裡故意驗證的是「回到分岔前所在幕」
        let reverted = revert_scene(root.path(), &world_id).unwrap();
        assert_eq!(reverted, 3);
        assert!(!root
            .path()
            .join(format!("worlds/{world_id}/transcript/4.jsonl"))
            .exists());
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(state.current_scene, 3);
        assert!(!state.scene_titles.contains_key("3"));
        assert!(!state.scene_labels.contains_key("4"));

        // 再從幕 0 分岔一次：幕 0 與幕 3 都是 base 0，這次該排第 3 個版本
        let forked_again = fork_scene(root.path(), &world_id, 0).unwrap();
        assert_eq!(forked_again, 4);
        let state = read_state(root.path(), &world_id).unwrap();
        assert_eq!(
            state.scene_labels.get("4").copied(),
            Some(SceneLabel {
                base: 0,
                version: 3,
                parent: Some(3),
                forked: true
            })
        );
    }

    #[test]
    fn fork_scene_rejects_current_or_future_scene() {
        let root = TestRoot::new("fork-scene-rejects-current");
        let world_id = create_world(root.path(), "分岔擋桌").unwrap();
        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                raw: None,
                ts: "2026-08-06T00:00:00Z".to_owned(),
                speaker_id: String::new(),
                speaker_name: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "序幕".to_owned(),
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        // from_scene == current_scene：還沒換幕，不能從自己這幕分岔
        let error = fork_scene(root.path(), &world_id, 0).unwrap_err().to_string();
        assert!(error.contains("只能從前面的幕分岔"));

        // from_scene > current_scene：幕號還沒出現過
        let error = fork_scene(root.path(), &world_id, 5).unwrap_err().to_string();
        assert!(error.contains("只能從前面的幕分岔"));
    }

    #[test]
    fn fork_scene_rejects_a_scene_with_no_events() {
        let root = TestRoot::new("fork-scene-rejects-empty");
        let world_id = create_world(root.path(), "分岔空幕桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.current_scene = 1; // 幕 0 從沒寫過任何事件，模擬空幕
        write_state(root.path(), &world_id, &state).unwrap();

        let error = fork_scene(root.path(), &world_id, 0).unwrap_err().to_string();
        assert!(error.contains("這一幕沒有紀錄可以接續"));
    }

    #[test]
    fn scene_label_falls_back_to_original_line_for_unlabeled_scene() {
        let root = TestRoot::new("scene-label-fallback");
        let world_id = create_world(root.path(), "原線桌").unwrap();
        let state = read_state(root.path(), &world_id).unwrap();

        assert_eq!(
            scene_label(&state, 5),
            SceneLabel {
                base: 5,
                version: 1,
                parent: Some(4),
                forked: false
            }
        );
        // 幕 0 沒有前幕：fallback 的 parent 也要是 None，跟 revert_scene 的邊界檢查對得起來
        assert_eq!(
            scene_label(&state, 0),
            SceneLabel {
                base: 0,
                version: 1,
                parent: None,
                forked: false
            }
        );
    }

    #[test]
    fn pop_transcript_restores_entire_nested_tree_snapshot() {
        let root = TestRoot::new("nested-state-pop");
        let world_id = create_world(root.path(), "巢狀桌").unwrap();
        let first = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("10".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        let second = TableState {
            table: BTreeMap::new(),
            tree: BTreeMap::from([(
                "World".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "城市".to_owned(),
                    StateNode::Branch(BTreeMap::from([(
                        "聲望".to_owned(),
                        StateNode::Leaf("20".to_owned()),
                    )])),
                )])),
            )]),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        };
        for snapshot in [first.clone(), second.clone()] {
            append_transcript(
                root.path(),
                &world_id,
                0,
                &TranscriptEvent {
                    raw: None,
                    ts: "now".to_owned(),
                    speaker_id: String::new(),
                    speaker_name: "GM".to_owned(),
                    kind: TranscriptKind::Narration,
                    text: "旁白".to_owned(),
                    state: Some(snapshot),
                    gm_only: false,
                },
            )
            .unwrap();
        }
        assert!(pop_transcript(root.path(), &world_id, 0).unwrap());
        assert_eq!(read_state(root.path(), &world_id).unwrap().state, first);
    }

    /// AI 卡重構包 4b：換幕結算角色卡自動隱藏。出現過＝本幕有回歸事件 (a) 或換幕當下
    /// present 名單命中 (b)；兩者都沒有（就算幕開始時本來在主區）結算成隱藏；
    /// archived 的卡完全不受結算影響。
    #[test]
    fn begin_next_scene_settles_card_auto_hidden() {
        let root = TestRoot::new("card-settlement");
        let world_id = create_world(root.path(), "測試桌").unwrap();

        let fox = character_card(&new_id(), "狐狸"); // (a) 本幕有回歸事件
        let bear = character_card(&new_id(), "熊"); // (b) present 命中
        let badger = character_card(&new_id(), "獾"); // 兩者都沒有 → 結算成隱藏
        let ghost = character_card(&new_id(), "亡靈"); // archived → 完全不動
        for card in [&fox, &bear, &badger, &ghost] {
            write_character(root.path(), &world_id, card).unwrap();
        }
        set_character_auto_hidden(root.path(), &world_id, &fox.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &bear.id, true).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &badger.id, false).unwrap();
        set_character_auto_hidden(root.path(), &world_id, &ghost.id, false).unwrap();
        set_character_archived(root.path(), &world_id, &ghost.id, true).unwrap();

        append_transcript(
            root.path(),
            &world_id,
            0,
            &TranscriptEvent {
                ts: "now".to_owned(),
                speaker_id: String::new(),
                speaker_name: "GM".to_owned(),
                kind: TranscriptKind::System,
                text: "（角色回歸）〈狐狸〉\n尾巴很大。".to_owned(),
                raw: None,
                state: None,
                gm_only: false,
            },
        )
        .unwrap();

        let mut state = read_state(root.path(), &world_id).unwrap();
        state
            .state
            .table
            .insert("present".to_owned(), "狐狸、熊".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();

        begin_next_scene(root.path(), &world_id, "摘要", "zh-TW", None).unwrap();

        let metas = list_characters(root.path(), &world_id).unwrap();
        let auto_hidden_of = |id: &str| metas.iter().find(|meta| meta.id == id).unwrap().auto_hidden;
        assert!(!auto_hidden_of(&fox.id), "本幕有回歸事件的卡應該結算成主區");
        assert!(!auto_hidden_of(&bear.id), "present 命中的卡應該結算成主區");
        assert!(
            auto_hidden_of(&badger.id),
            "沒出現過的卡（就算原本在主區）應該結算成隱藏"
        );
        assert!(!auto_hidden_of(&ghost.id), "archived 的卡完全不受結算影響");
    }

}
