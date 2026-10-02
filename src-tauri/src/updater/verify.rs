//! 安裝前重驗與版本庫沿用共用。`.sig` 與 conf 公鑰都是 base64 包著的 minisign 檔，
//! 先解碼再用 minisign-verify。標準 base64 不接受中間的空白。

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

use crate::ui_msg::UiMsg;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReleaseFile {
    pub version: String,
    pub platform: String,
    pub file: String,
    #[serde(default)]
    pub format_version: Option<u64>,
    pub size: u64,
    pub downloaded_at: String,
}

/// 簽章、版本、平台、檔名、大小有一項對不上就回「驗簽失敗」。
pub(crate) fn verify_artifact(
    bytes: &[u8],
    sig_b64: &str,
    pubkey_b64: &str,
    release: &ReleaseFile,
    expected_version: &str,
    expected_platform: &str,
    expected_file: &str,
) -> Result<(), String> {
    if release.version != expected_version
        || release.platform != expected_platform
        || release.file != expected_file
        || release.size != bytes.len() as u64
    {
        return Err(UiMsg::SignatureInvalid.into());
    }
    let signature_text = decode_armor(sig_b64)?;
    let key_text = decode_armor(pubkey_b64)?;
    let key = PublicKey::decode(&key_text).map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    let signature =
        Signature::decode(&signature_text).map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    // minisign 0.7 簽的是 prehash。allow_legacy 開了會把 prehash 簽章當成舊格式拒掉。
    key.verify(bytes, &signature, false)
        .map_err(|_| UiMsg::SignatureInvalid.to_string())
}

fn decode_armor(input: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .map_err(|_| UiMsg::SignatureInvalid.to_string())?;
    String::from_utf8(bytes).map_err(|_| UiMsg::SignatureInvalid.to_string())
}

#[cfg(test)]
pub(crate) fn sign_fixture(bytes: &[u8]) -> (String, String) {
    let pair = minisign::KeyPair::generate_unencrypted_keypair().expect("keypair");
    let signature = minisign::sign(
        Some(&pair.pk),
        &pair.sk,
        std::io::Cursor::new(bytes),
        None,
        None,
    )
    .expect("sign");
    let public_key = pair.pk.to_box().expect("public box").into_string();
    let engine = base64::engine::general_purpose::STANDARD;
    (
        engine.encode(public_key),
        engine.encode(signature.into_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(size: u64) -> ReleaseFile {
        ReleaseFile {
            version: "1.2.3".to_owned(),
            platform: "darwin-aarch64".to_owned(),
            file: "TableTavern_1.2.3_aarch64.app.tar.gz".to_owned(),
            format_version: Some(1),
            size,
            downloaded_at: "1970-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn base64_then_verify_accepts_a_real_signature_and_rejects_tamper() {
        let bytes = b"installer-bytes";
        let (public_key, signature) = sign_fixture(bytes);
        verify_artifact(
            bytes,
            &signature,
            &public_key,
            &release(bytes.len() as u64),
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .unwrap();

        let mut tampered = bytes.to_vec();
        tampered[0] ^= 0xff;
        let error = verify_artifact(
            &tampered,
            &signature,
            &public_key,
            &release(tampered.len() as u64),
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .unwrap_err();
        assert_eq!(error, UiMsg::SignatureInvalid.to_string());
    }

    #[test]
    fn raw_minisign_text_and_internal_whitespace_fail_before_verify() {
        let bytes = b"installer-bytes";
        let (public_key, signature) = sign_fixture(bytes);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(signature.trim())
            .unwrap();
        let raw = String::from_utf8(raw).unwrap();
        assert!(verify_artifact(
            bytes,
            &raw,
            &public_key,
            &release(bytes.len() as u64),
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .is_err());

        let mut broken = signature.clone();
        broken.insert(broken.len() / 2, ' ');
        assert!(verify_artifact(
            bytes,
            &broken,
            &public_key,
            &release(bytes.len() as u64),
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .is_err());

        let padded = format!("\n{signature}\n");
        verify_artifact(
            bytes,
            &padded,
            &public_key,
            &release(bytes.len() as u64),
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .unwrap();
    }

    #[test]
    fn release_json_version_and_platform_are_part_of_the_check() {
        let bytes = b"installer-bytes";
        let (public_key, signature) = sign_fixture(bytes);
        let mut wrong_version = release(bytes.len() as u64);
        wrong_version.version = "9.9.9".to_owned();
        assert!(verify_artifact(
            bytes,
            &signature,
            &public_key,
            &wrong_version,
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .is_err());

        let mut wrong_platform = release(bytes.len() as u64);
        wrong_platform.platform = "windows-x86_64".to_owned();
        assert!(verify_artifact(
            bytes,
            &signature,
            &public_key,
            &wrong_platform,
            "1.2.3",
            "darwin-aarch64",
            "TableTavern_1.2.3_aarch64.app.tar.gz",
        )
        .is_err());
    }
}
