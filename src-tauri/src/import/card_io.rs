use crate::data::DataResult;
use crate::ui_msg::UiMsg;
use serde_json::Value;

pub(crate) const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub(super) fn string_field<'a>(data: &'a Value, field: &str) -> Option<&'a str> {
    data.get(field).and_then(Value::as_str)
}

/// PNG 結構壞掉：細節是技術原文，玩家看到的是「圖片檔損壞」加上這段。
pub(super) fn png_invalid(detail: &str) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::PngInvalid {
        detail: detail.to_owned(),
    }
    .into_error()
}

fn card_data_invalid(detail: &str) -> Box<dyn std::error::Error + Send + Sync> {
    UiMsg::CardDataInvalid {
        detail: detail.to_owned(),
    }
    .into_error()
}

/// 讀 PNG 卡的角色資料：優先 tEXt `chara`（V2），沒有才讀 `ccv3`（只寫 V3 的卡）；兩種 data 欄位同形。
pub(super) fn decode_png_character(bytes: &[u8]) -> DataResult<Vec<u8>> {
    match find_card_text(bytes, b"chara")? {
        Some(found) => Ok(found),
        None => find_card_text(bytes, b"ccv3")?.ok_or_else(|| UiMsg::CardPngNoData.into_error()),
    }
}

pub(super) fn find_card_text(bytes: &[u8], keyword: &[u8]) -> DataResult<Option<Vec<u8>>> {
    let mut offset = PNG_MAGIC.len();
    while offset < bytes.len() {
        if bytes.len() - offset < 12 {
            return Err(png_invalid("chunk header truncated"));
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
            .ok_or_else(|| png_invalid("chunk length overflow"))?;
        if chunk_end > bytes.len() {
            return Err(png_invalid("chunk runs past end of file"));
        }
        let kind = &bytes[offset + 4..offset + 8];
        let chunk_data = &bytes[offset + 8..offset + 8 + length];
        if kind == b"tEXt" {
            if let Some(separator) = chunk_data.iter().position(|byte| *byte == 0) {
                if &chunk_data[..separator] == keyword {
                    return decode_base64(&chunk_data[separator + 1..]).map(Some);
                }
            }
        }
        offset = chunk_end;
    }
    Ok(None)
}

fn decode_base64(input: &[u8]) -> DataResult<Vec<u8>> {
    if !input.len().is_multiple_of(4) {
        return Err(card_data_invalid("base64 length is not a multiple of 4"));
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for group in input.chunks_exact(4) {
        let padding = group.iter().filter(|byte| **byte == b'=').count();
        if padding > 2
            || (padding > 0
                && (group[..4 - padding].contains(&b'=')
                    || group[4 - padding..4].iter().any(|byte| *byte != b'=')))
        {
            return Err(card_data_invalid("invalid base64 padding"));
        }
        if padding > 0 && group != &input[input.len() - 4..] {
            return Err(card_data_invalid("base64 padding before the end"));
        }
        let mut values = [0u8; 4];
        for (index, byte) in group.iter().enumerate() {
            values[index] = if *byte == b'=' {
                0
            } else {
                base64_value(*byte).ok_or_else(|| card_data_invalid("invalid base64 character"))?
            };
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if padding < 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if padding == 0 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    Ok(output)
}

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let a = group[0];
        let b = *group.get(1).unwrap_or(&0);
        let c = *group.get(2).unwrap_or(&0);
        output.push(ALPHABET[(a >> 2) as usize] as char);
        output.push(ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        output.push(if group.len() > 1 {
            ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if group.len() > 2 {
            ALPHABET[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub(crate) fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
    chunk
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

/// 1×1 透明 PNG：匯出找不到可用底圖時的封面（卡資料在 chunk 裡，圖只是外觀）。
pub(crate) fn blank_png() -> Vec<u8> {
    let mut png = PNG_MAGIC.to_vec();
    let mut header = 1u32.to_be_bytes().to_vec();
    header.extend_from_slice(&1u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    png.extend_from_slice(&png_chunk(b"IHDR", &header));
    // zlib（stored deflate）：一列 filter 0 + 一個全透明像素
    const PIXEL: &[u8] = &[
        0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0, 0, 0x00, 0x05, 0x00, 0x01,
    ];
    png.extend_from_slice(&png_chunk(b"IDAT", PIXEL));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;
    use crate::import::images::{
        character_avatar, character_image, save_character_avatar, save_character_image,
    };
    use crate::import::import_character;
    use crate::import::test_support::{card_png, TestRoot};

    #[test]
    fn decodes_base64_and_rejects_invalid_input() {
        assert_eq!(decode_base64(b"SGVsbG8=").unwrap(), b"Hello");
        assert_eq!(
            decode_base64(b"SGVsbG8!").unwrap_err().to_string(),
            card_data_invalid("invalid base64 character").to_string()
        );
        assert_eq!(
            decode_base64(b"AA=A").unwrap_err().to_string(),
            card_data_invalid("invalid base64 padding").to_string()
        );
    }

    #[test]
    fn broken_png_and_png_without_card_data_report_codes() {
        let mut truncated = PNG_MAGIC.to_vec();
        truncated.extend_from_slice(&[0, 0, 0, 9]);
        assert_eq!(
            decode_png_character(&truncated).unwrap_err().to_string(),
            png_invalid("chunk header truncated").to_string()
        );
        assert_eq!(
            decode_png_character(PNG_MAGIC).unwrap_err().to_string(),
            UiMsg::CardPngNoData.to_string()
        );
    }

    #[test]
    fn reads_ccv3_only_when_chara_is_absent() {
        let text = |keyword: &str, json: &str| {
            let mut data = format!("{keyword}\0").into_bytes();
            data.extend_from_slice(base64_encode(json.as_bytes()).as_bytes());
            png_chunk(b"tEXt", &data)
        };
        let mut v3_only = PNG_MAGIC.to_vec();
        v3_only.extend_from_slice(&text("ccv3", "{\"v\":3}"));
        assert_eq!(decode_png_character(&v3_only).unwrap(), b"{\"v\":3}");

        let mut both = PNG_MAGIC.to_vec();
        both.extend_from_slice(&text("ccv3", "{\"v\":3}"));
        both.extend_from_slice(&text("chara", "{\"v\":2}"));
        assert_eq!(decode_png_character(&both).unwrap(), b"{\"v\":2}");
    }

    #[test]
    fn character_image_returns_png_base64_or_none() {
        let root = TestRoot::new("image");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = card_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();

        let encoded = character_image(root.path(), &world_id, &meta.id)
            .unwrap()
            .unwrap();
        assert_eq!(decode_base64(encoded.as_bytes()).unwrap(), png);
        assert_eq!(
            character_image(root.path(), &world_id, &data::new_id()).unwrap(),
            None
        );
    }

    #[test]
    fn saves_and_reads_character_image_and_avatar() {
        let root = TestRoot::new("save-images");
        let world_id = data::create_world(root.path(), "酒館").unwrap();
        let png = card_png(r#"{"data":{"name":"凱恩"}}"#);
        let meta = import_character(root.path(), &world_id, &png, "#111111", "zh-TW").unwrap();
        let image = crate::import::png_image::test_png::real_png(2, 2);
        let avatar = crate::import::png_image::test_png::real_png(3, 3);

        save_character_image(root.path(), &world_id, &meta.id, &image).unwrap();
        save_character_avatar(root.path(), &world_id, &meta.id, &avatar).unwrap();

        assert_eq!(
            decode_base64(
                character_image(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            image
        );
        assert_eq!(
            decode_base64(
                character_avatar(root.path(), &world_id, &meta.id)
                    .unwrap()
                    .unwrap()
                    .as_bytes()
            )
            .unwrap(),
            avatar
        );
    }
}
