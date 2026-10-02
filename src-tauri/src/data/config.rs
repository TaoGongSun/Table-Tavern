use super::{invalid_data, DataResult};
use crate::cli::ModelOption;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api_keys: BTreeMap<String, String>,
    #[serde(default)]
    pub tier_models: BTreeMap<String, String>,
    #[serde(default)]
    pub preferences: serde_json::Map<String, serde_json::Value>,
}

pub fn read_config(root: &Path) -> DataResult<AppConfig> {
    let path = root.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn config_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn read_raw_config(path: &Path) -> DataResult<Value> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(&text)?;
    if !value.is_object() {
        return Err(invalid_data("設定檔不是 JSON 物件"));
    }
    Ok(value)
}

fn parse_app_config(value: &Value) -> DataResult<AppConfig> {
    Ok(serde_json::from_value(value.clone())?)
}

/// 以磁碟上的原始 JSON 為底套補丁。`None` 表示這次不用寫。
pub fn update_config_with(
    root: &Path,
    build: impl FnOnce(&Value) -> DataResult<Option<Value>>,
) -> DataResult<AppConfig> {
    let _guard = config_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path = root.join("config.json");
    let mut base = read_raw_config(&path)?;
    let Some(patch) = build(&base)? else {
        return parse_app_config(&base);
    };
    let object = base
        .as_object_mut()
        .ok_or_else(|| invalid_data("設定檔不是 JSON 物件"))?;
    apply_config_patch(object, &patch)?;
    let parsed = parse_app_config(&base)?;
    write_raw_config(root, &path, &base)?;
    Ok(parsed)
}

pub fn update_config(root: &Path, patch: &Value) -> DataResult<AppConfig> {
    update_config_with(root, |_| Ok(Some(patch.clone())))
}

/// 舊 `smart_free` 模式改成 `stable_free`。在設定鎖裡看磁碟原文，不重寫沒碰到的欄位。
pub fn migrate_legacy_config(root: &Path) -> DataResult<AppConfig> {
    update_config_with(root, |disk| {
        let legacy = disk
            .get("preferences")
            .and_then(|value| value.get(crate::smart_free::MODE_KEY))
            .and_then(|value| value.as_str())
            == Some(crate::smart_free::MODE_SMART_LEGACY);
        if !legacy {
            return Ok(None);
        }
        let key = crate::smart_free::MODE_KEY;
        Ok(Some(serde_json::json!({
            "preferences": { key: crate::smart_free::MODE_STABLE }
        })))
    })
}

fn apply_config_patch(base: &mut Map<String, Value>, patch: &Value) -> DataResult<()> {
    let patch = patch
        .as_object()
        .ok_or_else(|| invalid_data("設定補丁必須是 JSON 物件"))?;
    for (key, value) in patch {
        if matches!(key.as_str(), "api_keys" | "tier_models" | "preferences") {
            apply_map_patch(base, key, value)?;
        } else if value.is_null() {
            base.remove(key);
        } else {
            base.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

fn apply_map_patch(base: &mut Map<String, Value>, key: &str, value: &Value) -> DataResult<()> {
    if value.is_null() {
        base.remove(key);
        return Ok(());
    }
    let patch_map = value
        .as_object()
        .ok_or_else(|| invalid_data(format!("{key} 補丁必須是物件或 null")))?;
    let entry = base
        .entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    let map = entry
        .as_object_mut()
        .ok_or_else(|| invalid_data(format!("{key} 不是物件")))?;
    for (child, child_value) in patch_map {
        if child_value.is_null() {
            map.remove(child);
        } else {
            map.insert(child.clone(), child_value.clone());
        }
    }
    Ok(())
}

fn write_raw_config(root: &Path, path: &Path, value: &Value) -> DataResult<()> {
    // world-write-exempt: 建立設定目錄，不是桌目錄
    fs::create_dir_all(root)?;
    let tmp = root.join("config.json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        // world-write-exempt: 設定檔暫存 config.json.tmp，不是桌目錄
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        // 0600 僅限 unix；Windows 的 %APPDATA% 本身即使用者私有目錄，不需 chmod
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp)?;
        // world-write-exempt: 寫入設定檔暫存，不是桌目錄
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    // Windows 的 fs::rename 是 MoveFileExW＋MOVEFILE_REPLACE_EXISTING，可以直接覆蓋。
    // world-write-exempt: 設定檔暫存改名成 config.json，不是桌目錄
    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    {
        if let Ok(dir) = File::open(root) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// 模型清單快取：可重建的資料，跟 config.json 分家——設定檔含 API key（0600）且每次
/// 存設定就整份重寫，不該再馱著幾十 KB 的清單；快取壞掉也只是重抓一次，不連累設定。
/// 形狀為 `{供應商 id: [{id, label}, …]}`，內容由前端組好整份寫入。
pub fn read_model_catalog(root: &Path) -> DataResult<BTreeMap<String, Vec<ModelOption>>> {
    let path = root.join("model_catalog.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?).unwrap_or_default())
}

pub fn write_model_catalog(
    root: &Path,
    catalog: &BTreeMap<String, Vec<ModelOption>>,
) -> DataResult<()> {
    // world-write-exempt: 建立設定目錄，不是桌目錄
    fs::create_dir_all(root)?;
    // world-write-exempt: model_catalog.json 是可重建的模型快取，不是桌目錄
    fs::write(
        root.join("model_catalog.json"),
        serde_json::to_string(catalog)?,
    )?;
    Ok(())
}

pub fn validate_sponsor_pack(bytes: &[u8]) -> DataResult<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid_data(format!("贊助包不是合法 JSON：{error}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid_data("贊助包必須是 JSON 物件"))?;

    if object.get("type").and_then(serde_json::Value::as_str) != Some("table-tavern-sponsor-pack") {
        return Err(invalid_data("贊助包的 type 不正確"));
    }

    if object
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|format| format == 0)
    {
        return Err(invalid_data("贊助包的 format 必須是正整數"));
    }

    Ok(())
}

pub fn sponsor_pack_active(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "ttpack")
            && fs::read(entry.path()).is_ok_and(|bytes| validate_sponsor_pack(&bytes).is_ok())
    })
}

pub fn install_sponsor_pack(root: &Path, bytes: &[u8]) -> DataResult<()> {
    validate_sponsor_pack(bytes)?;
    // world-write-exempt: 建立設定目錄，不是桌目錄
    fs::create_dir_all(root)?;
    // world-write-exempt: sponsor-pack.ttpack 是贊助包，不是桌目錄
    fs::write(root.join("sponsor-pack.ttpack"), bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_support::*;

    #[test]
    fn validates_a_valid_sponsor_pack() {
        assert!(
            validate_sponsor_pack(br#"{"type":"table-tavern-sponsor-pack","format":1}"#).is_ok()
        );
    }

    #[test]
    fn rejects_sponsor_pack_with_wrong_type() {
        let error = validate_sponsor_pack(br#"{"type":"other-pack","format":1}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("type"));
    }

    #[test]
    fn rejects_sponsor_pack_without_format() {
        let error = validate_sponsor_pack(br#"{"type":"table-tavern-sponsor-pack"}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("format"));
    }

    #[test]
    fn install_sponsor_pack_activates_only_valid_packages() {
        let root = TestRoot::new("sponsor-pack");
        let empty_root = TestRoot::new("empty-sponsor-pack");
        let pack = br#"{"type":"table-tavern-sponsor-pack","format":1,"edition":"supporter"}"#;

        assert!(!sponsor_pack_active(empty_root.path()));
        install_sponsor_pack(root.path(), pack).unwrap();
        assert!(sponsor_pack_active(root.path()));
    }

    /// 「先秀舊的」靠這條往返：開 app 讀得回上次存的清單，玩家點進設定才即刻有東西可選。
    #[test]
    fn model_catalog_round_trips_and_missing_file_is_empty() {
        let root = TestRoot::new("catalog");
        // 還沒預熱過就是空的，不是錯誤
        assert!(read_model_catalog(root.path()).unwrap().is_empty());

        let mut catalog = BTreeMap::new();
        catalog.insert(
            "agy".to_owned(),
            vec![ModelOption {
                id: "gemini-3.6-flash-high".to_owned(),
                label: "Gemini 3.6 Flash (High)".to_owned(),
            }],
        );
        write_model_catalog(root.path(), &catalog).unwrap();
        assert_eq!(read_model_catalog(root.path()).unwrap(), catalog);

        // 快取壞掉只是重抓一次，不能讓開 app 失敗
        fs::write(root.path().join("model_catalog.json"), "{ 壞掉的 json").unwrap();
        assert!(read_model_catalog(root.path()).unwrap().is_empty());
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

        update_config(
            root.path(),
            &serde_json::json!({
                "api_keys": { "provider": "secret" },
                "tier_models": { "best": "model-name" },
                "preferences": { "language": "zh-TW" }
            }),
        )
        .unwrap();
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

    #[test]
    fn patch_deletes_null_keeps_unknown_and_keeps_empty_string() {
        let root = TestRoot::new("config-patch");
        fs::write(
            {
                fs::create_dir_all(root.path()).unwrap();
                root.path().join("config.json")
            },
            r#"{"api_keys":{"keep":"yes","drop":"gone"},"preferences":{"language":"zh-TW","theme":"dark"},"extra":{"nested":1},"note":"stay"}"#,
        )
        .unwrap();

        let saved = update_config(
            root.path(),
            &serde_json::json!({
                "api_keys": { "drop": null, "empty": "" },
                "preferences": { "theme": null },
                "note": null
            }),
        )
        .unwrap();

        assert_eq!(saved.api_keys.get("keep").map(String::as_str), Some("yes"));
        assert!(!saved.api_keys.contains_key("drop"));
        assert_eq!(saved.api_keys.get("empty").map(String::as_str), Some(""));
        assert_eq!(
            saved
                .preferences
                .get("language")
                .and_then(|value| value.as_str()),
            Some("zh-TW")
        );
        assert!(!saved.preferences.contains_key("theme"));

        let raw: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.path().join("config.json")).unwrap())
                .unwrap();
        assert_eq!(raw["extra"]["nested"], 1);
        assert!(raw.get("note").is_none());
        assert_eq!(raw["api_keys"]["empty"], "");
    }

    #[test]
    fn invalid_merged_config_is_rejected_without_touching_disk() {
        let root = TestRoot::new("config-validate");
        let original = r#"{"api_keys":{"openrouter":"sk-ok"},"note":"stay"}"#;
        fs::create_dir_all(root.path()).unwrap();
        let path = root.path().join("config.json");
        fs::write(&path, original).unwrap();

        let error = update_config(
            root.path(),
            &serde_json::json!({"api_keys":{"openrouter":123}}),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("123") || error.to_string().contains("string"),
            "{error}"
        );
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
}
