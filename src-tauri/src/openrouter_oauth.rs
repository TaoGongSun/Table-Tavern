use crate::config_root;
use crate::data::{self, AppConfig, DataResult};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const OPENROUTER_AUTH_URL: &str = "https://openrouter.ai/auth";
const OPENROUTER_KEY_URL: &str = "https://openrouter.ai/api/v1/auth/keys";
const FREE_BOOTSTRAP_MODEL: &str = "openrouter/free";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_CALLBACK_REQUEST_BYTES: usize = 16 * 1024;

const ERR_BROWSER: &str = "openrouter_oauth_browser";
const ERR_TIMEOUT: &str = "openrouter_oauth_timeout";
const ERR_CALLBACK: &str = "openrouter_oauth_callback";
const ERR_STATE: &str = "openrouter_oauth_state";
const ERR_CANCELLED: &str = "openrouter_oauth_cancelled";
const ERR_NETWORK: &str = "openrouter_oauth_network";
const ERR_EXCHANGE: &str = "openrouter_oauth_exchange";
const ERR_SAVE: &str = "openrouter_oauth_save";
const ERR_PKCE: &str = "openrouter_oauth_pkce";
const ERR_KEY_EMPTY: &str = "openrouter_key_empty";

#[derive(Serialize)]
struct KeyExchangeRequest<'a> {
    code: &'a str,
    code_verifier: &'a str,
    code_challenge_method: &'static str,
}

#[derive(Deserialize)]
struct KeyExchangeResponse {
    key: String,
}

fn new_nonce() -> String {
    format!("{}{}", ulid::Ulid::new(), ulid::Ulid::new())
}

fn valid_unreserved(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
}

fn validate_pkce_input(verifier: &str, challenge: &str) -> Result<(), String> {
    if !(43..=128).contains(&verifier.len()) || !valid_unreserved(verifier) {
        return Err(ERR_PKCE.to_owned());
    }
    if challenge.len() != 43
        || !challenge
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ERR_PKCE.to_owned());
    }
    Ok(())
}

fn callback_url(port: u16, state: &str) -> String {
    format!("http://localhost:{port}/callback?state={state}")
}

fn authorization_url(callback_url: &str, challenge: &str) -> Result<Url, String> {
    let mut url = Url::parse(OPENROUTER_AUTH_URL).map_err(|_| ERR_CALLBACK.to_owned())?;
    url.query_pairs_mut()
        .append_pair("callback_url", callback_url)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn parse_callback_target(target: &str, expected_state: &str) -> Result<String, String> {
    let url =
        Url::parse(&format!("http://localhost{target}")).map_err(|_| ERR_CALLBACK.to_owned())?;
    if url.path() != "/callback" {
        return Err(ERR_CALLBACK.to_owned());
    }

    let mut state = None;
    let mut code = None;
    let mut oauth_error = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "state" => state = Some(value.into_owned()),
            "code" => code = Some(value.into_owned()),
            "error" => oauth_error = Some(value.into_owned()),
            _ => {}
        }
    }

    if state.as_deref() != Some(expected_state) {
        return Err(ERR_STATE.to_owned());
    }
    if oauth_error.is_some() {
        return Err(ERR_CANCELLED.to_owned());
    }
    code.filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ERR_CALLBACK.to_owned())
}

async fn read_request_target(stream: &mut TcpStream) -> Result<String, String> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| ERR_CALLBACK.to_owned())?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > MAX_CALLBACK_REQUEST_BYTES {
            return Err(ERR_CALLBACK.to_owned());
        }
    }

    let request = std::str::from_utf8(&request).map_err(|_| ERR_CALLBACK.to_owned())?;
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| ERR_CALLBACK.to_owned())?;
    let mut parts = first_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err(ERR_CALLBACK.to_owned());
    }
    parts
        .next()
        .map(str::to_owned)
        .ok_or_else(|| ERR_CALLBACK.to_owned())
}

async fn respond_to_browser(stream: &mut TcpStream, ok: bool) {
    let body = if ok {
        "<!doctype html><meta charset=\"utf-8\"><title>Table Tavern</title><p>OpenRouter authorization received. You can return to Table Tavern.</p>"
    } else {
        "<!doctype html><meta charset=\"utf-8\"><title>Table Tavern</title><p>OpenRouter authorization was not completed. Return to Table Tavern and try again.</p>"
    };
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        if ok { "200 OK" } else { "400 Bad Request" },
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    let (mut stream, _) = tokio::time::timeout(CALLBACK_TIMEOUT, listener.accept())
        .await
        .map_err(|_| ERR_TIMEOUT.to_owned())?
        .map_err(|_| ERR_CALLBACK.to_owned())?;

    let result = match read_request_target(&mut stream).await {
        Ok(target) => parse_callback_target(&target, expected_state),
        Err(error) => Err(error),
    };
    respond_to_browser(&mut stream, result.is_ok()).await;
    result
}

async fn exchange_code(code: &str, verifier: &str) -> Result<String, String> {
    let response = reqwest::Client::new()
        .post(OPENROUTER_KEY_URL)
        .json(&KeyExchangeRequest {
            code,
            code_verifier: verifier,
            code_challenge_method: "S256",
        })
        .send()
        .await
        .map_err(|_| ERR_NETWORK.to_owned())?;

    if !response.status().is_success() {
        return Err(ERR_EXCHANGE.to_owned());
    }
    let payload: KeyExchangeResponse =
        response.json().await.map_err(|_| ERR_EXCHANGE.to_owned())?;
    let key = payload.key.trim();
    if key.is_empty() {
        return Err(ERR_EXCHANGE.to_owned());
    }
    Ok(key.to_owned())
}

fn has_explicit_api_tier(config: &AppConfig) -> bool {
    ["best", "balanced", "fast"].iter().any(|tier| {
        config
            .tier_models
            .get(*tier)
            .is_some_and(|model| !model.trim().is_empty())
    })
}

fn apply_free_bootstrap(config: &mut AppConfig) {
    if has_explicit_api_tier(config) {
        return;
    }
    for tier in ["best", "balanced", "fast"] {
        config
            .tier_models
            .insert(tier.to_owned(), FREE_BOOTSTRAP_MODEL.to_owned());
    }
}

fn persist_openrouter_key(root: &Path, key: &str) -> DataResult<AppConfig> {
    let mut config = data::read_config(root)?;
    config
        .api_keys
        .insert("openrouter".to_owned(), key.trim().to_owned());
    apply_free_bootstrap(&mut config);
    data::write_config(root, &config)?;
    Ok(config)
}

#[tauri::command]
pub(crate) fn save_openrouter_key(
    app: tauri::AppHandle,
    api_key: String,
) -> Result<AppConfig, String> {
    if api_key.trim().is_empty() {
        return Err(ERR_KEY_EMPTY.to_owned());
    }
    persist_openrouter_key(&config_root(&app)?, &api_key).map_err(|_| ERR_SAVE.to_owned())
}

#[tauri::command]
pub(crate) async fn connect_openrouter(
    app: tauri::AppHandle,
    code_verifier: String,
    code_challenge: String,
) -> Result<AppConfig, String> {
    let root = config_root(&app)?;
    let existing = data::read_config(&root).map_err(|_| ERR_SAVE.to_owned())?;
    if existing
        .api_keys
        .get("openrouter")
        .is_some_and(|key| !key.trim().is_empty())
    {
        return Ok(existing);
    }
    validate_pkce_input(&code_verifier, &code_challenge)?;

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| ERR_CALLBACK.to_owned())?;
    let port = listener
        .local_addr()
        .map_err(|_| ERR_CALLBACK.to_owned())?
        .port();
    let state = new_nonce();
    let callback = callback_url(port, &state);
    let auth_url = authorization_url(&callback, &code_challenge)?;

    app.opener()
        .open_url(auth_url.as_str(), None::<&str>)
        .map_err(|_| ERR_BROWSER.to_owned())?;

    let code = wait_for_callback(listener, &state).await?;
    let key = exchange_code(&code, &code_verifier).await?;
    persist_openrouter_key(&root, &key).map_err(|_| ERR_SAVE.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pkce_inputs_require_rfc_shapes() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(validate_pkce_input(verifier, challenge).is_ok());
        assert_eq!(
            validate_pkce_input("short", challenge).unwrap_err(),
            ERR_PKCE
        );
        assert_eq!(
            validate_pkce_input(verifier, "not+/base64url____________________________")
                .unwrap_err(),
            ERR_PKCE
        );
    }

    #[test]
    fn authorization_url_keeps_state_inside_callback_and_uses_s256() {
        let callback = callback_url(51423, "state-token");
        let url = authorization_url(&callback, "challenge-token").unwrap();
        let params: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params.get("callback_url"), Some(&callback));
        assert_eq!(
            params.get("code_challenge"),
            Some(&"challenge-token".to_owned())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_owned())
        );
    }

    #[test]
    fn callback_requires_matching_state_and_code() {
        assert_eq!(
            parse_callback_target("/callback?state=abc&code=xyz", "abc").unwrap(),
            "xyz"
        );
        assert_eq!(
            parse_callback_target("/callback?state=wrong&code=xyz", "abc").unwrap_err(),
            ERR_STATE
        );
        assert_eq!(
            parse_callback_target("/callback?state=abc&error=access_denied", "abc").unwrap_err(),
            ERR_CANCELLED
        );
        assert_eq!(
            parse_callback_target("/callback?state=abc", "abc").unwrap_err(),
            ERR_CALLBACK
        );
    }

    #[test]
    fn bootstrap_only_fills_completely_unconfigured_api_tiers() {
        let mut fresh = AppConfig::default();
        apply_free_bootstrap(&mut fresh);
        for tier in ["best", "balanced", "fast"] {
            assert_eq!(
                fresh.tier_models.get(tier).map(String::as_str),
                Some(FREE_BOOTSTRAP_MODEL)
            );
        }

        let mut customized = AppConfig::default();
        customized
            .tier_models
            .insert("best".to_owned(), "vendor/custom".to_owned());
        apply_free_bootstrap(&mut customized);
        assert_eq!(
            customized.tier_models.get("best").map(String::as_str),
            Some("vendor/custom")
        );
        assert!(!customized.tier_models.contains_key("balanced"));
        assert!(!customized.tier_models.contains_key("fast"));

        let mut cli_only = AppConfig::default();
        cli_only
            .tier_models
            .insert("claude:best".to_owned(), "opus".to_owned());
        apply_free_bootstrap(&mut cli_only);
        assert_eq!(
            cli_only.tier_models.get("best").map(String::as_str),
            Some(FREE_BOOTSTRAP_MODEL)
        );
    }

    #[test]
    fn persisting_oauth_key_preserves_existing_config_and_adds_bootstrap() {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-openrouter-oauth-{}",
            ulid::Ulid::new()
        ));
        fs::create_dir_all(&root).unwrap();

        let mut existing = AppConfig::default();
        existing.preferences.insert(
            "language".to_owned(),
            serde_json::Value::String("en".to_owned()),
        );
        existing
            .tier_models
            .insert("claude:fast".to_owned(), "haiku".to_owned());
        data::write_config(&root, &existing).unwrap();

        let saved = persist_openrouter_key(&root, "  sk-or-test  ").unwrap();
        assert_eq!(
            saved.api_keys.get("openrouter").map(String::as_str),
            Some("sk-or-test")
        );
        assert_eq!(
            saved.preferences.get("language"),
            existing.preferences.get("language")
        );
        assert_eq!(
            saved.tier_models.get("claude:fast"),
            existing.tier_models.get("claude:fast")
        );
        for tier in ["best", "balanced", "fast"] {
            assert_eq!(
                saved.tier_models.get(tier).map(String::as_str),
                Some(FREE_BOOTSTRAP_MODEL)
            );
        }
        assert_eq!(data::read_config(&root).unwrap(), saved);

        let _ = fs::remove_dir_all(root);
    }
}
