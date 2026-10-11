//! 更新與回退點的網路設定：時限、轉址、外掛錯誤轉碼。
//! `read_timeout` 對「連線到回應頭（含轉址）」是單一期限，對本文是每收到一段就重算的閒置期限，
//! 慢但持續的大檔不會被殺。整段期限只套更新檢查（外掛的 `Update.timeout` 恆為 None，不傳到下載）。

use std::time::Duration;

use tauri_plugin_updater::UpdaterBuilder;

use crate::ui_msg::UiMsg;

#[derive(Clone, Copy, Debug)]
pub(crate) struct NetTimeouts {
    /// 更新檢查每個 endpoint 請求的整段期限。
    pub check_total: Duration,
    /// 回應頭前的期限與本文閒置期限。
    pub idle: Duration,
}

impl NetTimeouts {
    pub(crate) const PRODUCTION: Self = Self {
        check_total: Duration::from_secs(60),
        idle: Duration::from_secs(60),
    };
}

/// 外掛的檢查與下載都吃 `configure_client`（存在外掛 context，`check()` 建出的 `Update` 帶著走）。
pub(crate) fn with_timeouts(builder: UpdaterBuilder, timeouts: NetTimeouts) -> UpdaterBuilder {
    builder
        .timeout(timeouts.check_total)
        .configure_client(move |client| client.read_timeout(timeouts.idle))
}

/// 連線、送出、逾時、本文讀取失敗都轉成同一則給玩家看的訊息；原文進 log。解析錯誤與其他照原文。
pub(crate) fn plugin_error_text(error: tauri_plugin_updater::Error) -> String {
    if let tauri_plugin_updater::Error::Reqwest(inner) = &error {
        if inner.is_timeout() || inner.is_connect() || inner.is_body() || inner.is_request() {
            log::warn!("更新連線失敗：{inner:?}");
            return UiMsg::UpdateNetworkFailed.to_string();
        }
    }
    error.to_string()
}

#[derive(Clone, Copy, Debug)]
struct FetchConfig {
    idle: Duration,
    /// 正式版只走 https，轉址目標也一樣；測試的 http 假伺服器才關掉。
    https_only: bool,
    /// 測試的本機 https 假伺服器用自簽憑證。
    #[cfg(test)]
    accept_invalid_certs: bool,
}

const PRODUCTION_FETCH: FetchConfig = FetchConfig {
    idle: NetTimeouts::PRODUCTION.idle,
    https_only: true,
    #[cfg(test)]
    accept_invalid_certs: false,
};

/// GitHub release 資產網址一律 302 到 CDN，所以要跟轉址；完整性靠之後的驗簽。
const MAX_REDIRECTS: usize = 5;

/// 回退點本體與 `.sig` 的下載。
pub(crate) async fn fetch_release_asset(url: String) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(UiMsg::RollbackPointUrlInvalid.into());
    }
    fetch_with(&url, PRODUCTION_FETCH)
        .await
        .map_err(String::from)
}

#[derive(Debug)]
enum FetchFailure {
    Status(String),
    Http(reqwest::Error),
}

impl From<FetchFailure> for String {
    fn from(failure: FetchFailure) -> Self {
        match failure {
            FetchFailure::Status(status) => UiMsg::RollbackPointDownloadFailed { status }.into(),
            // 只進 log（回退點失敗轉成 rollback_ready: false），Debug 才看得到逾時等原因。
            FetchFailure::Http(error) => format!("{error:?}"),
        }
    }
}

async fn fetch_with(url: &str, config: FetchConfig) -> Result<Vec<u8>, FetchFailure> {
    let builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .https_only(config.https_only)
        .read_timeout(config.idle);
    #[cfg(test)]
    let builder = builder.danger_accept_invalid_certs(config.accept_invalid_certs);
    let response = builder
        .build()
        .map_err(FetchFailure::Http)?
        .get(url)
        .send()
        .await
        .map_err(FetchFailure::Http)?;
    if !response.status().is_success() {
        return Err(FetchFailure::Status(response.status().to_string()));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(FetchFailure::Http)
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
