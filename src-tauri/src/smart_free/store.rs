use super::select::FreeModel;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const CACHE_FILE: &str = "smart_free_cache.json";
const PINS_FILE: &str = "smart_free_pins.json";
const EXPIRY_NOTICES_FILE: &str = "smart_free_expiry_notices.json";
const SEEN_FILE: &str = "smart_free_seen.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub account_fingerprint: String,
    #[serde(default)]
    pub user_catalog: Option<Vec<FreeModel>>,
    #[serde(default)]
    pub user_catalog_fetched_at: u64,
    #[serde(default)]
    pub weekly_ids: Option<Vec<String>>,
    #[serde(default)]
    pub weekly_fetched_at: u64,
    /// 角色扮演排行（`?category=roleplay`）的 canonical_slug 名次，供穩定免費推薦挑選。
    #[serde(default)]
    pub roleplay_slugs: Option<Vec<String>>,
    #[serde(default)]
    pub roleplay_fetched_at: u64,
}

impl Cache {
    pub fn catalog_for(&self, fingerprint: &str) -> Option<&[FreeModel]> {
        (self.account_fingerprint == fingerprint)
            .then_some(self.user_catalog.as_deref())
            .flatten()
    }
}

pub fn read_cache(root: &Path) -> Cache {
    read_json(&root.join(CACHE_FILE))
}

pub fn write_cache(root: &Path, cache: &Cache) -> std::io::Result<()> {
    write_json_atomic(root, CACHE_FILE, cache)
}

pub fn read_pins(root: &Path) -> BTreeMap<String, String> {
    read_json(&root.join(PINS_FILE))
}

pub fn write_pins(root: &Path, pins: &BTreeMap<String, String>) -> std::io::Result<()> {
    write_json_atomic(root, PINS_FILE, pins)
}

pub fn read_expiry_notices(root: &Path) -> BTreeMap<String, String> {
    read_json(&root.join(EXPIRY_NOTICES_FILE))
}

pub fn write_expiry_notices(
    root: &Path,
    notices: &BTreeMap<String, String>,
) -> std::io::Result<()> {
    write_json_atomic(root, EXPIRY_NOTICES_FILE, notices)
}

/// §15 新限免提示的本機去重：記玩家已看過（比對基準）的限時推薦 id。
/// 綁定帳號指紋，換帳號自動重新建立基準，不把別帳號看過的當看過。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenRecommendations {
    #[serde(default)]
    pub account_fingerprint: String,
    #[serde(default)]
    pub ids: Vec<String>,
}

pub fn read_seen(root: &Path) -> SeenRecommendations {
    read_json(&root.join(SEEN_FILE))
}

pub fn write_seen(root: &Path, seen: &SeenRecommendations) -> std::io::Result<()> {
    write_json_atomic(root, SEEN_FILE, seen)
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_json_atomic<T: Serialize>(root: &Path, name: &str, value: &T) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    let tmp = root.join(format!("{name}.{}.tmp", ulid::Ulid::generate()));
    // 先寫同目錄暫存檔再 rename；序列化或寫入失敗時不會覆蓋上一份好檔。
    fs::write(&tmp, serde_json::to_vec(value)?)?;
    fs::rename(tmp, root.join(name))
}

#[cfg(test)]
mod tests {
    use super::super::test_root;
    use super::*;

    #[test]
    fn broken_or_missing_files_read_as_empty_and_round_trip() {
        let root = test_root("store");
        assert_eq!(read_cache(&root), Cache::default());
        assert!(read_pins(&root).is_empty());
        let cache = Cache {
            account_fingerprint: "account-a".to_owned(),
            user_catalog: Some(Vec::new()),
            user_catalog_fetched_at: 10,
            weekly_ids: Some(vec!["x/model:free".to_owned()]),
            weekly_fetched_at: 20,
            roleplay_slugs: Some(vec!["x/model-20260101".to_owned()]),
            roleplay_fetched_at: 30,
        };
        write_cache(&root, &cache).unwrap();
        assert_eq!(read_cache(&root), cache);
        assert!(read_cache(&root).catalog_for("account-b").is_none());
        fs::write(root.join(CACHE_FILE), "{ 壞掉").unwrap();
        assert_eq!(read_cache(&root), Cache::default());
        let _ = fs::remove_dir_all(root);
    }
}
