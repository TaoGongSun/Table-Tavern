use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub type DataResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Best,
    Balanced,
    Fast,
    Default,
}

impl Tier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Balanced => "balanced",
            Self::Fast => "fast",
            Self::Default => "default",
        }
    }

    pub(crate) fn parse(value: &str) -> DataResult<Self> {
        match value {
            "best" => Ok(Self::Best),
            "balanced" => Ok(Self::Balanced),
            "fast" => Ok(Self::Fast),
            "default" => Ok(Self::Default),
            _ => Err(invalid_data(format!("invalid tier: {value}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterMeta {
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterCard {
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub tier: Tier,
    pub public_md: String,
    pub private_md: String,
}

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
    pub speaker: String,
    pub kind: TranscriptKind,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    #[serde(default)]
    pub model_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub current_scene: u64,
    #[serde(default)]
    pub catchup_summaries: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api_keys: BTreeMap<String, String>,
    #[serde(default)]
    pub tier_models: BTreeMap<String, String>,
    #[serde(default)]
    pub preferences: serde_json::Map<String, serde_json::Value>,
}

fn invalid_data(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into()
}

fn validate_name(name: &str) -> DataResult<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err(invalid_data(format!("invalid name: {name:?}")));
    }
    Ok(())
}

fn worlds_dir(root: &Path) -> PathBuf {
    root.join("worlds")
}

fn world_dir(root: &Path, world: &str) -> DataResult<PathBuf> {
    validate_name(world)?;
    Ok(worlds_dir(root).join(world))
}

fn character_path(root: &Path, world: &str, name: &str) -> DataResult<PathBuf> {
    validate_name(name)?;
    Ok(world_dir(root, world)?
        .join("characters")
        .join(format!("{name}.md")))
}

/// 最後活動時間＝transcript 內最新檔案 mtime，退而求其次用世界目錄 mtime
fn last_active(world_directory: &Path) -> std::time::SystemTime {
    let mut latest = fs::metadata(world_directory)
        .and_then(|meta| meta.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = fs::read_dir(world_directory.join("transcript")) {
        for entry in entries.flatten() {
            if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
                latest = latest.max(modified);
            }
        }
    }
    latest
}

/// 依最後活動排序（新的在前），供側欄桌列表用（NewPlan §9.3）
pub fn list_worlds(root: &Path) -> DataResult<Vec<String>> {
    let directory = worlds_dir(root);
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid_data("world directory name is not valid UTF-8"))?;
            worlds.push((last_active(&entry.path()), name));
        }
    }
    worlds.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(worlds.into_iter().map(|(_, name)| name).collect())
}

pub fn create_world(root: &Path, name: &str) -> DataResult<()> {
    validate_name(name)?;
    let directory = worlds_dir(root).join(name);
    fs::create_dir_all(worlds_dir(root))?;
    fs::create_dir(&directory)?;
    fs::create_dir(directory.join("characters"))?;
    fs::create_dir(directory.join("transcript"))?;
    fs::write(directory.join("world.md"), "")?;
    fs::write(
        directory.join("state.json"),
        serde_json::to_string_pretty(&WorldState::default())?,
    )?;
    Ok(())
}

/// 空桌回收（NewPlan §9.3）：只回收完全未動過的桌——零訊息、零角色、world.md 空白；
/// 任一項有內容即保留，防資料遺失。回傳是否真的刪了。
pub fn reclaim_world_if_empty(root: &Path, world: &str) -> DataResult<bool> {
    let directory = world_dir(root, world)?;
    if !directory.exists() {
        return Ok(false);
    }
    let has_messages = fs::read_dir(directory.join("transcript"))
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.metadata().map(|meta| meta.len() > 0).unwrap_or(true))
        })
        .unwrap_or(false);
    let has_characters = fs::read_dir(directory.join("characters"))
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    let world_md = fs::read_to_string(directory.join("world.md")).unwrap_or_default();
    if has_messages || has_characters || !world_md.trim().is_empty() {
        return Ok(false);
    }
    fs::remove_dir_all(directory)?;
    Ok(true)
}

/// 懶命名的後半：桌名隨時可改（NewPlan §9.3）
pub fn rename_world(root: &Path, world: &str, new_name: &str) -> DataResult<()> {
    let from = world_dir(root, world)?;
    let to = world_dir(root, new_name)?;
    if world == new_name {
        return Ok(());
    }
    if to.exists() {
        return Err(invalid_data(format!("world already exists: {new_name}")));
    }
    fs::rename(from, to)?;
    Ok(())
}

pub fn read_world_md(root: &Path, world: &str) -> DataResult<String> {
    Ok(fs::read_to_string(
        world_dir(root, world)?.join("world.md"),
    )?)
}

pub fn write_world_md(root: &Path, world: &str, content: &str) -> DataResult<()> {
    fs::write(world_dir(root, world)?.join("world.md"), content)?;
    Ok(())
}

fn parse_frontmatter(contents: &str) -> DataResult<(CharacterMeta, &str)> {
    let rest = contents
        .strip_prefix("---\n")
        .ok_or_else(|| invalid_data("character card must start with frontmatter"))?;
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| invalid_data("character card frontmatter is not closed"))?;
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];

    let mut name = None;
    let mut color = None;
    let mut avatar = None;
    let mut tier = None;
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            if line.trim().is_empty() {
                continue;
            }
            return Err(invalid_data(format!("invalid frontmatter line: {line}")));
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "name" => name = Some(value.to_owned()),
            "color" => color = Some(value.to_owned()),
            "avatar" => avatar = Some(value.to_owned()),
            "tier" => tier = Some(Tier::parse(value)?),
            _ => {}
        }
    }

    let name = name.ok_or_else(|| invalid_data("frontmatter is missing name"))?;
    validate_name(&name)?;
    Ok((
        CharacterMeta {
            name,
            color: color.ok_or_else(|| invalid_data("frontmatter is missing color"))?,
            avatar: avatar.ok_or_else(|| invalid_data("frontmatter is missing avatar"))?,
            tier: tier.ok_or_else(|| invalid_data("frontmatter is missing tier"))?,
        },
        body,
    ))
}

fn parse_sections(body: &str) -> (String, String) {
    #[derive(Clone, Copy)]
    enum Section {
        Public,
        Private,
    }

    let mut markers = Vec::new();
    let mut offset = 0;
    for segment in body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let section = match line {
            "## 公開" => Some(Section::Public),
            "## 私有" => Some(Section::Private),
            _ => None,
        };
        if let Some(section) = section {
            markers.push((offset, offset + segment.len(), section));
        }
        offset += segment.len();
    }

    let mut public_md = String::new();
    let mut private_md = String::new();
    for (index, (_, content_start, section)) in markers.iter().copied().enumerate() {
        let content_end = markers
            .get(index + 1)
            .map(|(heading_start, _, _)| *heading_start)
            .unwrap_or(body.len());
        let mut content = &body[content_start..content_end];
        if index + 1 < markers.len() {
            content = content.strip_suffix('\n').unwrap_or(content);
        }
        match section {
            Section::Public => public_md = content.to_owned(),
            Section::Private => private_md = content.to_owned(),
        }
    }
    (public_md, private_md)
}

fn serialize_character(card: &CharacterCard) -> String {
    format!(
        "---\nname: {}\ncolor: {}\navatar: {}\ntier: {}\n---\n## 公開\n{}\n## 私有\n{}",
        card.name,
        card.color,
        card.avatar,
        card.tier.as_str(),
        card.public_md,
        card.private_md
    )
}

pub fn list_characters(root: &Path, world: &str) -> DataResult<Vec<CharacterMeta>> {
    let directory = world_dir(root, world)?.join("characters");
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut characters = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        {
            let contents = fs::read_to_string(entry.path())?;
            characters.push(parse_frontmatter(&contents)?.0);
        }
    }
    characters.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(characters)
}

pub fn read_character(root: &Path, world: &str, name: &str) -> DataResult<CharacterCard> {
    let contents = fs::read_to_string(character_path(root, world, name)?)?;
    let (meta, body) = parse_frontmatter(&contents)?;
    let (public_md, private_md) = parse_sections(body);
    Ok(CharacterCard {
        name: meta.name,
        color: meta.color,
        avatar: meta.avatar,
        tier: meta.tier,
        public_md,
        private_md,
    })
}

fn validate_single_line(field: &str, value: &str) -> DataResult<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(invalid_data(format!("{field} must be a single line")));
    }
    Ok(())
}

pub fn write_character(root: &Path, world: &str, card: &CharacterCard) -> DataResult<()> {
    let path = character_path(root, world, &card.name)?;
    validate_single_line("color", &card.color)?;
    validate_single_line("avatar", &card.avatar)?;
    fs::write(path, serialize_character(card))?;
    Ok(())
}

fn transcript_path(root: &Path, world: &str, scene: u64) -> DataResult<PathBuf> {
    Ok(world_dir(root, world)?
        .join("transcript")
        .join(format!("{scene}.jsonl")))
}

pub fn append_transcript(
    root: &Path,
    world: &str,
    scene: u64,
    event: &TranscriptEvent,
) -> DataResult<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(root, world, scene)?)?;
    serde_json::to_writer(&mut file, event)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn read_transcript(root: &Path, world: &str, scene: u64) -> DataResult<Vec<TranscriptEvent>> {
    let path = transcript_path(root, world, scene)?;
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

pub fn read_state(root: &Path, world: &str) -> DataResult<WorldState> {
    let path = world_dir(root, world)?.join("state.json");
    if !path.exists() {
        return Ok(WorldState::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_state(root: &Path, world: &str, state: &WorldState) -> DataResult<()> {
    fs::write(
        world_dir(root, world)?.join("state.json"),
        serde_json::to_string_pretty(state)?,
    )?;
    Ok(())
}

pub fn read_config(root: &Path) -> DataResult<AppConfig> {
    let path = root.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_config(root: &Path, config: &AppConfig) -> DataResult<()> {
    fs::create_dir_all(root)?;
    let path = root.join("config.json");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    // 0600 僅限 unix；Windows 的 %APPDATA% 本身即使用者私有目錄，不需 chmod
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path)?;
    file.write_all(serde_json::to_string_pretty(config)?.as_bytes())?;
    // mode() 只在建檔時生效；補 set_permissions 修復既存檔的過寬權限
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("table-tavern-{label}-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_lists_worlds_and_rejects_duplicates() {
        let root = TestRoot::new("worlds");
        assert!(list_worlds(root.path()).unwrap().is_empty());

        create_world(root.path(), "群島").unwrap();
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["群島"]);
        assert!(create_world(root.path(), "群島").is_err());
        assert!(root.path().join("worlds/群島/characters").is_dir());
        assert!(root.path().join("worlds/群島/transcript").is_dir());
        assert!(root.path().join("worlds/群島/world.md").is_file());
        assert!(root.path().join("worlds/群島/state.json").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn lists_worlds_by_last_activity_descending() {
        let root = TestRoot::new("activity");
        create_world(root.path(), "甲桌").unwrap();
        create_world(root.path(), "乙桌").unwrap();

        // 兩桌目錄 mtime 撥回一小時前：同時間時按名稱升冪（乙 U+4E59 < 甲 U+7532）
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for name in ["甲桌", "乙桌"] {
            let directory = fs::File::open(root.path().join("worlds").join(name)).unwrap();
            directory.set_modified(hour_ago).unwrap();
        }
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["乙桌", "甲桌"]);

        // 對名稱排序居後的甲桌寫一筆訊息，活動排序應把它推到最前
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "你好".to_owned(),
        };
        append_transcript(root.path(), "甲桌", 0, &event).unwrap();
        assert_eq!(list_worlds(root.path()).unwrap(), vec!["甲桌", "乙桌"]);
    }

    #[test]
    fn reclaims_only_untouched_worlds() {
        let root = TestRoot::new("reclaim");
        create_world(root.path(), "空桌").unwrap();
        assert!(reclaim_world_if_empty(root.path(), "空桌").unwrap());
        assert!(list_worlds(root.path()).unwrap().is_empty());
        // 已刪的桌再回收一次應為 no-op
        assert!(!reclaim_world_if_empty(root.path(), "空桌").unwrap());

        create_world(root.path(), "有訊息").unwrap();
        let event = TranscriptEvent {
            ts: "2026-07-19T00:00:00Z".to_owned(),
            speaker: "玩家".to_owned(),
            kind: TranscriptKind::Player,
            text: "留著".to_owned(),
        };
        append_transcript(root.path(), "有訊息", 0, &event).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有訊息").unwrap());

        create_world(root.path(), "有角色").unwrap();
        let card = CharacterCard {
            name: "旅人".to_owned(),
            color: "#e07a5f".to_owned(),
            avatar: "🎭".to_owned(),
            tier: Tier::Default,
            public_md: String::new(),
            private_md: String::new(),
        };
        write_character(root.path(), "有角色", &card).unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有角色").unwrap());

        create_world(root.path(), "有設定").unwrap();
        write_world_md(root.path(), "有設定", "海島世界").unwrap();
        assert!(!reclaim_world_if_empty(root.path(), "有設定").unwrap());
    }

    #[test]
    fn renames_world_and_rejects_collisions() {
        let root = TestRoot::new("rename");
        create_world(root.path(), "舊名").unwrap();
        create_world(root.path(), "占用").unwrap();

        rename_world(root.path(), "舊名", "新名").unwrap();
        assert!(root.path().join("worlds/新名").is_dir());
        assert!(!root.path().join("worlds/舊名").exists());

        assert!(rename_world(root.path(), "新名", "占用").is_err());
        rename_world(root.path(), "新名", "新名").unwrap();
        assert!(rename_world(root.path(), "新名", "壞/名").is_err());
    }

    #[test]
    fn rejects_multiline_frontmatter_values() {
        let root = TestRoot::new("scalars");
        create_world(root.path(), "世界").unwrap();
        let card = CharacterCard {
            name: "角色".to_owned(),
            color: "#123456\ntier: best".to_owned(),
            avatar: "🧙".to_owned(),
            tier: Tier::Default,
            public_md: String::new(),
            private_md: String::new(),
        };
        assert!(write_character(root.path(), "世界", &card).is_err());
    }

    #[test]
    fn rejects_unsafe_world_and_character_names() {
        let root = TestRoot::new("names");
        for name in ["../evil", "a/b", ".hidden", ""] {
            assert!(
                create_world(root.path(), name).is_err(),
                "accepted {name:?}"
            );
        }

        create_world(root.path(), "安全世界").unwrap();
        for name in ["../evil", "a/b", ".hidden", ""] {
            let card = CharacterCard {
                name: name.to_owned(),
                color: "#123456".to_owned(),
                avatar: "🧙".to_owned(),
                tier: Tier::Default,
                public_md: String::new(),
                private_md: String::new(),
            };
            assert!(write_character(root.path(), "安全世界", &card).is_err());
        }
    }

    #[test]
    fn character_round_trip_preserves_fields_and_sections() {
        let root = TestRoot::new("character");
        create_world(root.path(), "港灣").unwrap();
        let card = CharacterCard {
            name: "阿藍".to_owned(),
            color: "#3366ff".to_owned(),
            avatar: "avatars/blue.png".to_owned(),
            tier: Tier::Best,
            public_md: "第一段\n\n- 公開條目\n".to_owned(),
            private_md: "秘密第一行\n\n秘密第二行".to_owned(),
        };

        write_character(root.path(), "港灣", &card).unwrap();
        assert_eq!(read_character(root.path(), "港灣", "阿藍").unwrap(), card);
        assert_eq!(
            list_characters(root.path(), "港灣").unwrap(),
            vec![CharacterMeta {
                name: "阿藍".to_owned(),
                color: "#3366ff".to_owned(),
                avatar: "avatars/blue.png".to_owned(),
                tier: Tier::Best,
            }]
        );

        let raw = fs::read_to_string(root.path().join("worlds/港灣/characters/阿藍.md")).unwrap();
        let frontmatter = raw
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0;
        let keys: Vec<_> = frontmatter
            .lines()
            .map(|line| line.split_once(':').unwrap().0)
            .collect();
        assert_eq!(keys, ["name", "color", "avatar", "tier"]);
        assert!(raw.contains("\n## 公開\n"));
        assert!(raw.contains("\n## 私有\n"));
    }

    #[test]
    fn frontmatter_accepts_spacing_and_order_but_rejects_invalid_tier() {
        let root = TestRoot::new("frontmatter");
        create_world(root.path(), "世界").unwrap();
        let path = root.path().join("worlds/世界/characters/角色.md");
        fs::write(
            &path,
            "---\ntier : fast\nunknown: ignored\navatar: 🐕\n color : #abcdef\nname : 角色\n---\n## 私有\n私密",
        )
        .unwrap();
        assert_eq!(
            read_character(root.path(), "世界", "角色").unwrap().tier,
            Tier::Fast
        );

        fs::write(
            path,
            "---\nname: 角色\ncolor: #abcdef\navatar: 🐕\ntier: impossible\n---\n",
        )
        .unwrap();
        assert!(read_character(root.path(), "世界", "角色").is_err());
    }

    #[test]
    fn transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind() {
        let root = TestRoot::new("transcript");
        create_world(root.path(), "劇場").unwrap();
        let events = vec![
            TranscriptEvent {
                ts: "2026-07-19T10:00:00+08:00".to_owned(),
                speaker: "旁白".to_owned(),
                kind: TranscriptKind::Narration,
                text: "序幕".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:01+08:00".to_owned(),
                speaker: "玩家".to_owned(),
                kind: TranscriptKind::Player,
                text: "第一行\n仍是同一事件".to_owned(),
            },
            TranscriptEvent {
                ts: "2026-07-19T10:00:02+08:00".to_owned(),
                speaker: "角色".to_owned(),
                kind: TranscriptKind::Dialogue,
                text: "你好".to_owned(),
            },
        ];
        for event in &events {
            append_transcript(root.path(), "劇場", 7, event).unwrap();
        }
        assert_eq!(read_transcript(root.path(), "劇場", 7).unwrap(), events);

        let path = root.path().join("worlds/劇場/transcript/7.jsonl");
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
            .write_all(b"{\"ts\":\"now\",\"speaker\":\"x\",\"kind\":\"bad\",\"text\":\"x\"}\n")
            .unwrap();
        let error = read_transcript(root.path(), "劇場", 7)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn state_round_trip_and_missing_file_default() {
        let root = TestRoot::new("state");
        fs::create_dir_all(root.path().join("worlds/無狀態")).unwrap();
        assert_eq!(
            read_state(root.path(), "無狀態").unwrap(),
            WorldState::default()
        );

        let mut state = WorldState {
            current_scene: 12,
            ..WorldState::default()
        };
        state
            .model_bindings
            .insert("船長".to_owned(), "balanced".to_owned());
        state
            .catchup_summaries
            .insert("水手".to_owned(), "錯過了序幕".to_owned());
        write_state(root.path(), "無狀態", &state).unwrap();
        assert_eq!(read_state(root.path(), "無狀態").unwrap(), state);
    }

    #[test]
    fn config_round_trip_and_permissions_are_private() {
        let root = TestRoot::new("config");
        assert_eq!(read_config(root.path()).unwrap(), AppConfig::default());
        let mut config = AppConfig::default();
        config
            .api_keys
            .insert("provider".to_owned(), "secret".to_owned());
        config
            .tier_models
            .insert("best".to_owned(), "model-name".to_owned());
        config.preferences.insert(
            "language".to_owned(),
            serde_json::Value::String("zh-TW".to_owned()),
        );

        write_config(root.path(), &config).unwrap();
        assert_eq!(read_config(root.path()).unwrap(), config);
        #[cfg(unix)]
        {
            let mode = fs::metadata(root.path().join("config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
