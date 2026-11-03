//! 立繪資料夾（plans/vn-mode-pkg7.md §3）：`sprite.json`、`body/<變體>.png`、`face/<表情>.png`、
//! `face-<變體>/<表情>.png`（獨立表情組）、`candidates/<時間戳>.png`。表情組以 `Option<&str>` 指名：
//! None＝定稿與共用組，Some(變體鍵)＝該變體的獨立組。路徑一律由驗過的鍵組成；寫入走桌目錄的共用寫入函式。
use crate::data;
use crate::transport::VN_EXPRESSIONS;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 候選最多留幾張（每張約 2 MB），新的進來就刪最舊的
const MAX_CANDIDATES: usize = 6;

/// 要生臉層的表情：neutral 不存，就是定稿本身
pub(crate) fn face_expressions() -> impl Iterator<Item = &'static str> {
    VN_EXPRESSIONS
        .into_iter()
        .filter(|expression| *expression != "neutral")
}

pub(crate) fn is_face_expression(value: &str) -> bool {
    face_expressions().any(|expression| expression == value)
}

/// 臉框（定稿像素座標）
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FaceBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl FaceBox {
    pub(crate) fn fits(&self, width: u32, height: u32) -> bool {
        self.w > 0
            && self.h > 0
            && self
                .x
                .checked_add(self.w)
                .is_some_and(|right| right <= width)
            && self
                .y
                .checked_add(self.h)
                .is_some_and(|bottom| bottom <= height)
    }

    /// 預設框：圖寬 30% 的正方形，水平置中、上緣在圖高 8% 處
    pub(crate) fn fallback(width: u32, height: u32) -> Self {
        let side = (width * 3 / 10).clamp(1, height.max(1));
        FaceBox {
            x: (width - side.min(width)) / 2,
            y: (height * 8 / 100).min(height.saturating_sub(side)),
            w: side.min(width),
            h: side,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Variant {
    pub key: String,
    pub label: String,
    /// 非 null＝獨立表情組（框以該變體身體圖座標存）；null＝共用定稿的框與臉層
    pub face: Option<FaceBox>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SpriteMeta {
    pub version: u32,
    pub face: FaceBox,
    /// 臉框是預設框（視覺模型沒給出合格框）：立繪分頁提示玩家手調
    #[serde(default)]
    pub face_fallback: bool,
    #[serde(default)]
    pub variants: Vec<Variant>,
    #[serde(default = "base_key")]
    pub default: String,
    /// 每格修訂號：`base`、`face/<表情>`、`face-<變體>/<表情>`、`body/<變體>`
    #[serde(default)]
    pub revs: BTreeMap<String, u64>,
}

fn base_key() -> String {
    "base".to_owned()
}

pub(crate) fn valid_variant_key(key: &str) -> bool {
    (1..=32).contains(&key.len())
        && key != "base"
        && key
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

impl SpriteMeta {
    /// 載入時驗：框在圖內（不合格退預設框）、變體鍵合格且唯一（不合格的丟掉）、預設變體存在
    fn validated(mut self, width: u32, height: u32) -> Self {
        if !self.face.fits(width, height) {
            self.face = FaceBox::fallback(width, height);
            self.face_fallback = true;
        }
        let mut seen = std::collections::HashSet::new();
        self.variants
            .retain(|variant| valid_variant_key(&variant.key) && seen.insert(variant.key.clone()));
        if !seen.contains(&self.default) {
            self.default = base_key();
        }
        self
    }

    pub(crate) fn rev(&self, slot: &str) -> u64 {
        self.revs.get(slot).copied().unwrap_or(0)
    }

    fn bump(&mut self, slot: &str) {
        *self.revs.entry(slot.to_owned()).or_default() += 1;
    }

    fn bump_faces(&mut self, set: Option<&str>) {
        for expression in face_expressions() {
            self.bump(&face_slot(set, expression));
        }
    }

    pub(crate) fn variant(&self, key: &str) -> Option<&Variant> {
        self.variants.iter().find(|variant| variant.key == key)
    }

    /// 新變體鍵：v1、v2…取第一個沒用過的
    pub(crate) fn next_variant_key(&self) -> String {
        (1..)
            .map(|index| format!("v{index}"))
            .find(|key| self.variant(key).is_none())
            .unwrap_or_default()
    }
}

/// 修訂號的格名：`face/<表情>` 或 `face-<變體>/<表情>`
fn face_slot(set: Option<&str>, expression: &str) -> String {
    match set {
        None => format!("face/{expression}"),
        Some(key) => format!("face-{key}/{expression}"),
    }
}

pub(crate) fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// 臉層工作開始時記下的身體圖雜湊、臉框與目標格修訂號；寫回前三者都要沒變
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    body_hash: Vec<u8>,
    face: FaceBox,
    rev: u64,
}

impl Snapshot {
    pub(crate) fn face(&self) -> FaceBox {
        self.face
    }
}

/// 一次讀進的立繪：驗過的 sprite.json、定稿尺寸與定稿位元組
pub(crate) struct Loaded {
    pub meta: SpriteMeta,
    pub size: (u32, u32),
    pub body: Vec<u8>,
}

pub(crate) struct SpriteDir(PathBuf);

impl SpriteDir {
    pub(crate) fn of(
        root: &std::path::Path,
        world_id: &str,
        character_id: &str,
    ) -> Result<Self, String> {
        data::vn_sprite_dir(root, world_id, character_id)
            .map(SpriteDir)
            .map_err(|error| error.to_string())
    }

    fn body_path(&self) -> PathBuf {
        self.0.join("body").join("base.png")
    }

    fn meta_path(&self) -> PathBuf {
        self.0.join("sprite.json")
    }

    fn face_dir(&self, set: Option<&str>) -> PathBuf {
        match set {
            None => self.0.join("face"),
            Some(key) => self.0.join(format!("face-{}", path_key(key))),
        }
    }

    pub(crate) fn face_path(&self, set: Option<&str>, expression: &str) -> PathBuf {
        self.face_dir(set).join(format!("{expression}.png"))
    }

    fn variant_path(&self, key: &str) -> PathBuf {
        self.0.join("body").join(format!("{}.png", path_key(key)))
    }

    /// 獨立表情組的操作：鍵要是 sprite.json 裡有自己臉框的變體
    fn own_set(meta: &SpriteMeta, set: Option<&str>) -> Result<(), String> {
        match set {
            Some(key) if meta.variant(key).and_then(|variant| variant.face).is_none() => {
                Err("VN_SPRITE_VARIANT".to_owned())
            }
            _ => Ok(()),
        }
    }

    /// 變體身體圖與尺寸
    pub(crate) fn variant_body(&self, key: &str) -> Option<(Vec<u8>, (u32, u32))> {
        let body = std::fs::read(self.variant_path(key)).ok()?;
        let size = png_size(&body)?;
        Some((body, size))
    }

    /// 這組的身體圖與臉框：None＝定稿；獨立組要有身體圖，框不在圖內退預設框
    fn set_body(
        &self,
        loaded: Loaded,
        set: Option<&str>,
    ) -> Option<(Vec<u8>, FaceBox, SpriteMeta)> {
        let Some(key) = set else {
            return Some((loaded.body, loaded.meta.face, loaded.meta));
        };
        let face = loaded.meta.variant(key)?.face?;
        let (body, (width, height)) = self.variant_body(key)?;
        let face = match face.fits(width, height) {
            true => face,
            false => FaceBox::fallback(width, height),
        };
        Some((body, face, loaded.meta))
    }

    fn candidates_dir(&self) -> PathBuf {
        self.0.join("candidates")
    }

    /// 有定稿才有立繪：定稿與 sprite.json 一次讀齊
    pub(crate) fn read(&self) -> Option<Loaded> {
        let body = std::fs::read(self.body_path()).ok()?;
        let (width, height) = png_size(&body)?;
        let meta: SpriteMeta =
            serde_json::from_slice(&std::fs::read(self.meta_path()).ok()?).ok()?;
        (meta.version == 1).then(|| Loaded {
            meta: meta.validated(width, height),
            size: (width, height),
            body,
        })
    }

    /// 提示詞用的變體名：只讀 sprite.json，只列有身體圖的合格變體（每輪都讀，不讀圖）
    pub(crate) fn variant_labels(&self) -> Vec<String> {
        let Some(meta) = std::fs::read(self.meta_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<SpriteMeta>(&bytes).ok())
        else {
            return Vec::new();
        };
        meta.variants
            .into_iter()
            .filter(|variant| {
                valid_variant_key(&variant.key) && self.variant_path(&variant.key).is_file()
            })
            .map(|variant| variant.label)
            .collect()
    }

    /// 候選檔名，新的在前
    pub(crate) fn candidates(&self) -> Vec<String> {
        let mut files: Vec<String> = std::fs::read_dir(self.candidates_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| valid_candidate(name))
            .collect();
        files.sort_unstable_by(|left, right| right.cmp(left));
        files
    }

    pub(crate) fn candidate_bytes(&self, file: &str) -> Result<Vec<u8>, String> {
        if !valid_candidate(file) {
            return Err(crate::ui_msg::UiMsg::InvalidFileName.into());
        }
        std::fs::read(self.candidates_dir().join(file)).map_err(|error| error.to_string())
    }

    /// 呼叫端持桌寫入許可
    pub(crate) fn add_candidate(&self, png: &[u8]) -> Result<String, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let file = format!("{stamp}.png");
        data::commit_world_write(&self.candidates_dir().join(&file), png)
            .map_err(|error| error.to_string())?;
        for old in self.candidates().into_iter().skip(MAX_CANDIDATES) {
            remove(&self.candidates_dir().join(old))?;
        }
        Ok(file)
    }

    /// 換定稿：寫身體圖、臉框，舊臉層全部作廢（換定稿＝表情全部重生）。呼叫端持桌寫入許可
    pub(crate) fn install_base(
        &self,
        png: &[u8],
        face: FaceBox,
        face_fallback: bool,
    ) -> Result<(), String> {
        let mut meta = self.read().map(|loaded| loaded.meta).unwrap_or(SpriteMeta {
            version: 1,
            face,
            face_fallback,
            variants: Vec::new(),
            default: base_key(),
            revs: BTreeMap::new(),
        });
        self.detach_shared_variants(&mut meta)?;
        meta.face = face;
        meta.face_fallback = face_fallback;
        meta.bump("base");
        meta.bump_faces(None);
        data::commit_world_write(&self.body_path(), png).map_err(|error| error.to_string())?;
        remove(&self.0.join("face"))?;
        self.write_meta(&meta)
    }

    /// 換定稿前：共用組變體的身體是照舊定稿對齊的，轉成獨立組——存舊臉框、複製現有臉層，缺的格交給佇列補
    fn detach_shared_variants(&self, meta: &mut SpriteMeta) -> Result<(), String> {
        let old_face = meta.face;
        for variant in meta
            .variants
            .iter_mut()
            .filter(|variant| variant.face.is_none())
        {
            for expression in face_expressions() {
                if let Ok(png) = std::fs::read(self.face_path(None, expression)) {
                    data::commit_world_write(&self.face_path(Some(&variant.key), expression), &png)
                        .map_err(|error| error.to_string())?;
                }
            }
            variant.face = Some(old_face);
        }
        Ok(())
    }

    /// 玩家改框：框要在該組身體圖範圍內；該組臉層全部失效重生。呼叫端持桌寫入許可
    pub(crate) fn set_face(&self, set: Option<&str>, face: FaceBox) -> Result<(), String> {
        let loaded = self.read().ok_or("VN_SPRITE_MISSING")?;
        Self::own_set(&loaded.meta, set)?;
        let size = match set {
            None => loaded.size,
            Some(key) => self.variant_body(key).ok_or("VN_SPRITE_MISSING")?.1,
        };
        let mut meta = loaded.meta;
        if !face.fits(size.0, size.1) {
            return Err("VN_SPRITE_FACE_OUT_OF_RANGE".to_owned());
        }
        match set {
            None => {
                meta.face = face;
                meta.face_fallback = false;
            }
            Some(key) => {
                let variant = meta.variants.iter_mut().find(|variant| variant.key == key);
                match variant {
                    Some(variant) if variant.face.is_some() => variant.face = Some(face),
                    _ => return Err("VN_SPRITE_MISSING".to_owned()),
                }
            }
        }
        meta.bump_faces(set);
        remove(&self.face_dir(set))?;
        self.write_meta(&meta)
    }

    /// 寫變體（新增、重骰或上傳替換）：face 為 None＝共用表情組。舊的獨立組臉層作廢。呼叫端持桌寫入許可
    pub(crate) fn install_variant(
        &self,
        key: &str,
        label: &str,
        png: &[u8],
        face: Option<FaceBox>,
    ) -> Result<(), String> {
        if !valid_variant_key(key) {
            return Err("VN_SPRITE_VARIANT".to_owned());
        }
        let mut meta = self.read().ok_or("VN_SPRITE_MISSING")?.meta;
        let entry = Variant {
            key: key.to_owned(),
            label: label.to_owned(),
            face,
        };
        match meta.variants.iter_mut().find(|variant| variant.key == key) {
            Some(variant) => *variant = entry,
            None => meta.variants.push(entry),
        }
        meta.bump(&format!("body/{key}"));
        meta.bump_faces(Some(key));
        data::commit_world_write(&self.variant_path(key), png)
            .map_err(|error| error.to_string())?;
        remove(&self.face_dir(Some(key)))?;
        self.write_meta(&meta)
    }

    /// 刪變體：身體圖、獨立組臉層與 sprite.json 那筆。呼叫端持桌寫入許可
    pub(crate) fn remove_variant(&self, key: &str) -> Result<(), String> {
        let mut meta = self.read().ok_or("VN_SPRITE_MISSING")?.meta;
        meta.variant(key).ok_or("VN_SPRITE_MISSING")?;
        meta.variants.retain(|variant| variant.key != key);
        if meta.default == key {
            meta.default = base_key();
        }
        meta.bump(&format!("body/{key}"));
        meta.bump_faces(Some(key));
        remove(&self.variant_path(key))?;
        remove(&self.face_dir(Some(key)))?;
        self.write_meta(&meta)
    }

    /// 重骰一格：刪臉層、修訂號加一（在途的舊工作寫不回來）。呼叫端持桌寫入許可
    pub(crate) fn drop_face(&self, set: Option<&str>, expression: &str) -> Result<(), String> {
        let mut meta = self.read().ok_or("VN_SPRITE_MISSING")?.meta;
        Self::own_set(&meta, set)?;
        meta.bump(&face_slot(set, expression));
        remove(&self.face_path(set, expression))?;
        self.write_meta(&meta)
    }

    /// 刪整份立繪（退回角色卡原圖）。呼叫端持桌寫入許可
    pub(crate) fn remove_all(&self) -> Result<(), String> {
        remove(&self.0)
    }

    /// 有定稿但缺的臉層：共用組，加上有身體圖的獨立組
    pub(crate) fn missing_faces(&self) -> Vec<(Option<String>, &'static str)> {
        let Some(loaded) = self.read() else {
            return Vec::new();
        };
        let own_sets = loaded
            .meta
            .variants
            .iter()
            .filter(|variant| variant.face.is_some() && self.variant_path(&variant.key).is_file())
            .map(|variant| Some(variant.key.clone()));
        std::iter::once(None)
            .chain(own_sets)
            .flat_map(|set| face_expressions().map(move |expression| (set.clone(), expression)))
            .filter(|(set, expression)| !self.face_path(set.as_deref(), expression).is_file())
            .collect()
    }

    /// 臉層工作的起點：該組身體圖（雜湊）、臉框與該格修訂號
    pub(crate) fn snapshot(
        &self,
        set: Option<&str>,
        expression: &str,
    ) -> Option<(Snapshot, Vec<u8>)> {
        let (body, face, meta) = self.set_body(self.read()?, set)?;
        let snapshot = Snapshot {
            body_hash: Sha256::digest(&body).to_vec(),
            face,
            rev: meta.rev(&face_slot(set, expression)),
        };
        Some((snapshot, body))
    }

    /// 寫臉層：身體圖、臉框、該格修訂號都跟開工時一樣才寫，否則丟掉回 false。呼叫端持桌寫入許可
    pub(crate) fn write_face_if_current(
        &self,
        set: Option<&str>,
        expression: &str,
        snapshot: &Snapshot,
        png: &[u8],
    ) -> Result<bool, String> {
        match self.snapshot(set, expression) {
            Some((current, _)) if current == *snapshot => {}
            _ => return Ok(false),
        }
        self.write_face(set, expression, png)?;
        Ok(true)
    }

    /// 寫臉層並加修訂號（玩家上傳替換也走這裡）。呼叫端持桌寫入許可
    pub(crate) fn write_face(
        &self,
        set: Option<&str>,
        expression: &str,
        png: &[u8],
    ) -> Result<(), String> {
        let mut meta = self.read().ok_or("VN_SPRITE_MISSING")?.meta;
        Self::own_set(&meta, set)?;
        meta.bump(&face_slot(set, expression));
        data::commit_world_write(&self.face_path(set, expression), png)
            .map_err(|error| error.to_string())?;
        self.write_meta(&meta)
    }

    fn write_meta(&self, meta: &SpriteMeta) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(meta).map_err(|error| error.to_string())?;
        data::commit_world_write_atomic(&self.meta_path(), &json).map_err(|error| error.to_string())
    }
}

/// 組路徑用的變體鍵：不合格的鍵換成不可能是真變體的名字（`!` 不在鍵的字元集內），路徑永遠留在立繪資料夾內
fn path_key(key: &str) -> &str {
    match valid_variant_key(key) {
        true => key,
        false => "!",
    }
}

fn valid_candidate(name: &str) -> bool {
    name.strip_suffix(".png").is_some_and(|stem| {
        (1..=20).contains(&stem.len()) && stem.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn remove(path: &std::path::Path) -> Result<(), String> {
    data::commit_world_remove(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
