//! route：送出前看各用途會打哪個模型。重用實際派送的解析（API 檔位 `resolve_model` 不退檔、
//! 只有開場翻譯／重構展開照它們自己的退檔規則；CLI 走 `tier_model` 的 CLI 分支），不送出任何請求。

use serde_json::{json, Value};

use crate::data::{self, AppConfig, Tier};
use crate::transport::dispatch::chat_transport;
use crate::{smart_free, transport};

const TIERS: [Tier; 3] = [Tier::Best, Tier::Balanced, Tier::Fast];
const API_BLOCKED: &str = "（API 未設定此檔模型：送出會被擋下，不會打出去）";
const CLI_DEFAULT: &str = "（未指定：由 provider／CLI 預設決定）";

/// 智慧免費預覽結果：Some(Ok(Some(primary))) 等；None＝智慧免費沒啟用。
type SmartPreview = Option<Result<Option<String>, String>>;

/// 某個檔位在目前傳輸下實際會送的模型——與派送端同一套解析，不套 GM 退檔。
fn tier_route(config: &AppConfig, kind: &str, tier: Tier, smart: &SmartPreview) -> Value {
    if kind == "api" {
        if let Some(preview) = smart {
            return json!({
                "tier": tier.as_str(),
                "smartFree": true,
                "previewPrimary": match preview {
                    Ok(Some(model)) => Value::from(model.clone()),
                    Ok(None) => Value::from("（沒有穩定首選：送出會被擋下）"),
                    Err(error) => Value::from(format!("（{error}）")),
                },
                "note": "只能預覽：實際送出依本次訊息長度挑選（這裡以最短長度估），送出前另查免費日額度、快取過期會先刷新",
            });
        }
        return json!({
            "tier": tier.as_str(),
            "model": transport::resolve_model(tier, config).map(Value::from)
                .unwrap_or_else(|_| Value::from(API_BLOCKED)),
        });
    }
    let resolved = transport::tier_model(config, kind, tier);
    json!({
        "tier": tier.as_str(),
        "model": resolved.model.map(Value::from).unwrap_or_else(|| Value::from(CLI_DEFAULT)),
        "effort": resolved.effort,
    })
}

/// 開場翻譯（commands/scene.rs）：API 模式該檔沒設模型時退 GM 檔，其餘照請求檔位。
fn translate_tier(config: &AppConfig, kind: &str, requested: Tier) -> Tier {
    if kind == "api" && transport::resolve_model(requested, config).is_err() {
        transport::gm_tier(config)
    } else {
        requested
    }
}

/// 純函式：設定＋智慧免費預覽＋（可選）一桌的角色 → 各用途的路由。
pub(super) fn describe(
    config: &AppConfig,
    smart: SmartPreview,
    characters: &[data::CharacterMeta],
) -> Value {
    let kind = chat_transport(config);
    let smart = if kind == "api" { smart } else { None };
    let route = |tier: Tier| tier_route(config, &kind, tier, &smart);
    let gm = transport::gm_tier(config);
    let image_source = config
        .preferences
        .get("image_source")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&kind)
        .to_owned();
    // API 生圖直接打 Images API（不經智慧免費與檔位）；CLI 生圖走 GM 檔
    let image = if image_source == "api" {
        json!({
            "transport": "api",
            "model": config.preferences.get("image_model").and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty()).unwrap_or(transport::DEFAULT_IMAGE_MODEL),
        })
    } else {
        json!({ "transport": image_source, "route": tier_route(config, &image_source, gm, &None) })
    };
    let translate = translate_tier(config, &kind, Tier::Fast);
    json!({
        "transport": kind,
        "smartFree": smart.is_some(),
        "tiers": TIERS.iter().map(|t| route(*t)).collect::<Vec<_>>(),
        "purposes": {
            "gm（旁白／推進／開桌生成／重構盤點）": route(gm),
            "refactorExpand（重構展開；API 缺 balanced 時退 GM 檔）": route(transport::refactor_expand_tier(config, &kind)),
            "translateOpening（開場翻譯，預設 fast；API 缺該檔時退 GM 檔）": route(translate),
            "characters（角色發言，依角色卡檔位，不退檔）": characters.iter().map(|c| json!({
                "id": c.id, "name": c.name, "route": route(c.tier),
            })).collect::<Vec<_>>(),
            "characterImage（生圖，依 image_source）": image,
        },
    })
}

pub(super) fn route(world: Option<&str>) -> Result<Value, String> {
    let config_root = super::config_root();
    let config = data::read_config(&config_root).map_err(|e| e.to_string())?;
    let smart =
        smart_free::is_active(&config).then(|| smart_free::preview_primary(&config_root, &config));
    let characters = match world {
        Some(id) => data::list_characters(&super::data_root(), id).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    Ok(describe(&config, smart, &characters))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pairs: &[(&str, &str)], tiers: &[(&str, &str)]) -> AppConfig {
        let mut config = AppConfig::default();
        for (k, v) in pairs {
            config.preferences.insert((*k).to_owned(), Value::from(*v));
        }
        for (k, v) in tiers {
            config.tier_models.insert((*k).to_owned(), (*v).to_owned());
        }
        config
    }

    fn character(name: &str, tier: Tier) -> data::CharacterMeta {
        serde_json::from_value(json!({
            "id": name, "name": name, "color": "#000", "avatar": "x", "tier": tier.as_str(),
        }))
        .unwrap()
    }

    #[test]
    fn api_tiers_and_characters_do_not_borrow_gm_fallback() {
        // 只設 best：balanced 角色派送時 resolve_model 會直接拒絕，route 也要說會被擋下
        let c = config(&[("transport", "api")], &[("best", "x/best-model")]);
        let v = describe(&c, None, &[character("騎士", Tier::Balanced)]);
        assert_eq!(v["tiers"][0]["model"], "x/best-model");
        assert_eq!(v["tiers"][1]["model"], API_BLOCKED);
        let chars = &v["purposes"]["characters（角色發言，依角色卡檔位，不退檔）"];
        assert_eq!(chars[0]["route"]["model"], API_BLOCKED);
        assert_eq!(
            v["purposes"]["gm（旁白／推進／開桌生成／重構盤點）"]["model"],
            "x/best-model"
        );
    }

    #[test]
    fn translate_and_refactor_expand_keep_their_own_fallback() {
        let c = config(&[("transport", "api")], &[("best", "x/best-model")]);
        let v = describe(&c, None, &[]);
        let p = &v["purposes"];
        assert_eq!(
            p["translateOpening（開場翻譯，預設 fast；API 缺該檔時退 GM 檔）"]["model"],
            "x/best-model"
        );
        assert_eq!(
            p["refactorExpand（重構展開；API 缺 balanced 時退 GM 檔）"]["model"],
            "x/best-model"
        );
    }

    #[test]
    fn api_without_any_model_is_blocked_everywhere() {
        let v = describe(&config(&[], &[]), None, &[]);
        for tier in v["tiers"].as_array().unwrap() {
            assert_eq!(tier["model"], API_BLOCKED);
        }
    }

    #[test]
    fn cli_route_without_override_says_provider_decides() {
        let v = describe(&config(&[("transport", "grok")], &[]), None, &[]);
        assert_eq!(v["tiers"][0]["model"], CLI_DEFAULT);
        let v = describe(&config(&[("transport", "claude")], &[]), None, &[]);
        assert_eq!(v["tiers"][0]["model"], "opus");
    }

    #[test]
    fn smart_free_is_a_preview_not_a_candidate_list() {
        let c = config(&[("transport", "api")], &[]);
        let v = describe(&c, Some(Ok(Some("a:free".into()))), &[]);
        assert_eq!(v["smartFree"], true);
        assert_eq!(v["tiers"][0]["previewPrimary"], "a:free");
        assert!(v["tiers"][0]["note"]
            .as_str()
            .unwrap()
            .starts_with("只能預覽"));
        assert!(v["tiers"][0].get("models").is_none());
    }
}
