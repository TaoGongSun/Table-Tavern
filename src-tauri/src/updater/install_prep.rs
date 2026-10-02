//! 開閘之前的設定與上一版紀錄。正向更新寫 pending 再清略過鍵。回退只寫略過鍵，不碰 previous.json。

use std::path::Path;

use serde_json::Value;

use super::previous;
use crate::data;

pub(crate) fn forward_before_raise(
    versions: &Path,
    config_root: &Path,
    from: &str,
    to: &str,
) -> Result<(), String> {
    previous::write_pending(versions, from, to)?;
    clear_skipped_if_present(config_root)
}

/// 被退掉的是現在這一版。寫進略過鍵，之後自動檢查不會再拿它來提醒。
pub(crate) fn rollback_before_raise(config_root: &Path, running: &str) -> Result<(), String> {
    write_skipped_version(config_root, running)
}

pub(crate) fn clear_skipped_if_present(root: &Path) -> Result<(), String> {
    let config = data::read_config(root).map_err(|error| error.to_string())?;
    if !config.preferences.contains_key("update_skipped_version") {
        return Ok(());
    }
    data::update_config(
        root,
        &serde_json::json!({ "preferences": { "update_skipped_version": Value::Null } }),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn write_skipped_version(root: &Path, version: &str) -> Result<(), String> {
    data::update_config(
        root,
        &serde_json::json!({ "preferences": { "update_skipped_version": version } }),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::run_gated_local;
    use crate::updater::previous::{self, read_previous};

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("tt-prep-{label}-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn skipped(root: &Path) -> Option<String> {
        data::read_config(root)
            .unwrap()
            .preferences
            .get("update_skipped_version")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    #[tokio::test]
    async fn rollback_writes_the_skip_before_the_gate_and_does_not_touch_previous() {
        let versions = TempDir::new("ver");
        let config = TempDir::new("cfg");
        previous::write_pending(&versions.0, "0.1.0", "0.2.0").unwrap();
        let path = versions.0.join("previous.json");
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["previous"] = serde_json::json!("0.0.9");
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = run_gated_local(
            || Err::<(), String>("重驗失敗".to_owned()),
            || rollback_before_raise(&config.0, "0.2.0"),
            |_| panic!("重驗失敗不該安裝"),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "重驗失敗");
        assert!(skipped(&config.0).is_none(), "重驗失敗不該寫略過鍵");

        let error = run_gated_local(
            || Ok(()),
            || {
                rollback_before_raise(&config.0, "0.2.0")?;
                Err("停在開閘前".to_owned())
            },
            |_| panic!("開閘前失敗不該安裝"),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "停在開閘前");
        assert_eq!(skipped(&config.0).as_deref(), Some("0.2.0"));
        let state = read_previous(&versions.0);
        assert_eq!(state.previous.as_deref(), Some("0.0.9"));
        assert_eq!(state.pending_from.as_deref(), Some("0.1.0"));
        assert_eq!(state.pending_to.as_deref(), Some("0.2.0"));
    }

    #[tokio::test]
    async fn forward_update_writes_pending_then_clears_the_skip() {
        let versions = TempDir::new("fwd");
        let config = TempDir::new("fwd-cfg");
        data::update_config(
            &config.0,
            &serde_json::json!({ "preferences": { "update_skipped_version": "0.2.0" } }),
        )
        .unwrap();
        let error = run_gated_local(
            || Ok(()),
            || {
                forward_before_raise(&versions.0, &config.0, "0.2.0", "0.3.0")?;
                Err("停在開閘前".to_owned())
            },
            |_| panic!("不該安裝"),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "停在開閘前");
        assert!(skipped(&config.0).is_none());
        let state = read_previous(&versions.0);
        assert_eq!(state.pending_from.as_deref(), Some("0.2.0"));
        assert_eq!(state.pending_to.as_deref(), Some("0.3.0"));
    }
}
