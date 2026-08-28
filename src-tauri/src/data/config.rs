use crate::cli::ModelOption;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use super::{DataResult, invalid_data};

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
    fs::create_dir_all(root)?;
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
    fs::create_dir_all(root)?;
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
