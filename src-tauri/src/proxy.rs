// Windows 系統代理自動下傳：牆內玩家的代理軟體常只開「系統代理」（瀏覽器吃得到），
// 但 CLI 只認 HTTPS_PROXY 環境變數，安裝／登入／聊天子程序因此連不上服務商。
// 這裡讀系統代理設定，代為塞進子程序環境變數；使用者自設的環境變數優先。
use tokio::process::Command;

/// 解析註冊表 ProxyServer 值成 scheme://host:port。
/// 兩種格式：整體 `127.0.0.1:7890`；分協定 `http=…;https=…;socks=…`（取 https > http > socks）。
/// https= 的代理本身講 HTTP CONNECT，scheme 一律 http；socks= 給 socks5。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_proxy_server(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if !value.contains('=') {
        return Some(with_scheme("http", value));
    }
    let pick = |wanted: &str| {
        value.split(';').find_map(|entry| {
            let (key, address) = entry.split_once('=')?;
            let address = address.trim();
            (key.trim().eq_ignore_ascii_case(wanted) && !address.is_empty())
                .then(|| address.to_owned())
        })
    };
    if let Some(address) = pick("https").or_else(|| pick("http")) {
        return Some(with_scheme("http", &address));
    }
    pick("socks").map(|address| with_scheme("socks5", &address))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn with_scheme(scheme: &str, address: &str) -> String {
    if address.contains("://") {
        address.to_owned()
    } else {
        format!("{scheme}://{address}")
    }
}

#[cfg(target_os = "windows")]
fn system_proxy() -> Option<String> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    let enabled: u32 = key.get_value("ProxyEnable").ok()?;
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    parse_proxy_server(&server)
}

#[cfg(not(target_os = "windows"))]
fn system_proxy() -> Option<String> {
    None
}

/// 掛在需要連外網的子程序上。PAC 自動設定檔（AutoConfigURL）解不了，維持現狀不處理。
pub fn apply_system_proxy(command: &mut Command) {
    const KEYS: [&str; 4] = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"];
    if KEYS.iter().any(|key| std::env::var_os(key).is_some()) {
        return;
    }
    if let Some(proxy) = system_proxy() {
        command.env("HTTPS_PROXY", &proxy).env("HTTP_PROXY", proxy);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_proxy_server;

    #[test]
    fn plain_address_gets_http_scheme() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:7890"),
            Some("http://127.0.0.1:7890".to_owned())
        );
    }

    #[test]
    fn per_protocol_prefers_https_then_http() {
        assert_eq!(
            parse_proxy_server("http=1.1.1.1:80;https=2.2.2.2:443"),
            Some("http://2.2.2.2:443".to_owned())
        );
        assert_eq!(
            parse_proxy_server("http=1.1.1.1:80;socks=3.3.3.3:1080"),
            Some("http://1.1.1.1:80".to_owned())
        );
    }

    #[test]
    fn socks_only_maps_to_socks5() {
        assert_eq!(
            parse_proxy_server("socks=127.0.0.1:1080"),
            Some("socks5://127.0.0.1:1080".to_owned())
        );
    }

    #[test]
    fn empty_or_irrelevant_yields_none() {
        assert_eq!(parse_proxy_server(""), None);
        assert_eq!(parse_proxy_server("ftp=1.2.3.4:21"), None);
    }

    #[test]
    fn existing_scheme_passes_through() {
        assert_eq!(
            parse_proxy_server("socks5://127.0.0.1:1080"),
            Some("socks5://127.0.0.1:1080".to_owned())
        );
    }
}
