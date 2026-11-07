//! 開作品包（plans/vn-mode-pkg8.md §2.5 #1–#7）：先查檔案大小與條目數，再掃中央目錄擋路徑、重複名、連結、
//! 加密與少見壓縮法；讀條目一律以實讀位元組計上限（不信標頭的 `size()`），並累計總解壓量。
//! 條目數在交給 zip crate 解析前，整檔掃過每個結尾目錄候選驗原始條目數與中央目錄大小（crate 的 `len()` 是同名去重後的數，
//! 擋不住大量同名條目），zip64 一律拒收（作品包上限遠小於 4 GB，用不到）。
//! 條目名只當查表鍵，從不拼進落檔路徑。重複名：zip crate 解析中央目錄時同名只留一筆，我們只透過這份解析結果讀，
//! 不會出現「驗的是 A、讀到 B」。
use super::format::Limits;
use super::opaque::{self, Opaque};
use crate::ui_msg::UiMsg;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use zip::{CompressionMethod, ZipArchive};

/// 一般檔 / 目錄的 Unix 檔案類型位元；其他（連結、裝置、FIFO）拒收
const S_IFMT: u32 = 0o170000;
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;

pub(crate) struct Pack {
    archive: ZipArchive<BufReader<Opaque<File>>>,
    index: HashMap<String, usize>,
    read_total: u64,
    limits: Limits,
}

fn too_large() -> String {
    UiMsg::VnWorkTooLarge.to_string()
}

/// 條目名規則：相對、不含 `..` 段、反斜線、冒號、控制字元，長度有界
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('/')
        && !name.contains(['\\', ':'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| part != ".." && part != ".")
}

/// 中央目錄每筆的寬鬆上限（固定 46 位元組＋條目名 255＋常見額外欄位），用來限制解析前的中央目錄大小
const DIRECTORY_ENTRY_BYTES: u64 = 1024;

/// 解析前整檔串流掃一遍：zip crate 會在整個檔案裡往回找結尾目錄（EOCD），容許其後有垃圾，候選解析失敗還會退到更前面的，
/// 所以每個它可能採用的候選都要先驗——原始條目數（crate 用「本碟條目數」，兩個都驗）與中央目錄大小不得超限，
/// 前面緊貼 zip64 定位標記就拒（crate 會改讀 zip64 的 64 位元條目數）。本碟號與目錄所在碟號不同的候選 crate 必拒，
/// 跳過不驗，免得圖片位元組裡碰巧出現的簽名誤擋正常包。檔案已受 `pack_bytes` 限，串流讀不吃記憶體。
fn check_directory(file: &mut (impl Read + Seek), entries: usize) -> Result<(), String> {
    const CHUNK: usize = 1 << 20;
    // 候選前 20 位元組（定位標記）＋本身 22 位元組，換塊時留尾巴接著驗
    const CARRY: usize = 64;
    let max_entries = entries as u64;
    let not_pack = |_| UiMsg::VnWorkNotPack.to_string();
    let mut buf: Vec<u8> = Vec::with_capacity(CHUNK + CARRY);
    let mut chunk = vec![0; CHUNK];
    // buf[0] 在檔案中的位置、下一個還沒驗的位置
    let (mut base, mut next) = (0u64, 0u64);
    loop {
        let read = file.read(&mut chunk).map_err(not_pack)?;
        buf.extend_from_slice(&chunk[..read]);
        let eof = read == 0;
        let u16_at = |i: usize| u64::from(u16::from_le_bytes([buf[i], buf[i + 1]]));
        let u32_at = |i: usize| {
            u64::from(u32::from_le_bytes([
                buf[i],
                buf[i + 1],
                buf[i + 2],
                buf[i + 3],
            ]))
        };
        // 最後 21 個位置湊不滿一個 EOCD：沒到檔尾就留到下一塊，到檔尾 crate 也解析不了
        let stop = buf.len().saturating_sub(21);
        for i in (next - base) as usize..stop {
            if &buf[i..i + 4] != b"PK\x05\x06" {
                continue;
            }
            if i >= 20 && &buf[i - 20..i - 16] == b"PK\x06\x07" {
                return Err(too_large());
            }
            if u16_at(i + 4) == u16_at(i + 6)
                && (u16_at(i + 8) > max_entries
                    || u16_at(i + 10) > max_entries
                    || u32_at(i + 12) > max_entries * DIRECTORY_ENTRY_BYTES)
            {
                return Err(too_large());
            }
        }
        if eof {
            break;
        }
        next = base + stop as u64;
        let keep = buf.len().saturating_sub(CARRY);
        buf.drain(..keep);
        base += keep as u64;
    }
    file.seek(SeekFrom::Start(0)).map_err(not_pack)?;
    Ok(())
}

impl Pack {
    pub(crate) fn open(path: &Path, limits: Limits) -> Result<Self, String> {
        let size = std::fs::metadata(path)
            .map_err(|error| {
                UiMsg::IoFailed {
                    error: error.to_string(),
                }
                .to_string()
            })?
            .len();
        if size > limits.pack_bytes {
            return Err(too_large());
        }
        let file = File::open(path).map_err(|error| {
            UiMsg::IoFailed {
                error: error.to_string(),
            }
            .to_string()
        })?;
        let mut file = opaque::open(file).map_err(|error| {
            UiMsg::IoFailed {
                error: error.to_string(),
            }
            .to_string()
        })?;
        check_directory(&mut file, limits.entries)?;
        let mut archive =
            ZipArchive::new(BufReader::new(file)).map_err(|_| UiMsg::VnWorkNotPack.to_string())?;
        if archive.len() > limits.entries {
            return Err(too_large());
        }
        let mut index = HashMap::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index_raw(i)
                .map_err(|error| broken(format!("entry {i}: {error}")))?;
            let name = entry.name().to_owned();
            if !safe_name(&name) {
                return Err(UiMsg::ArchivePathUnsafe.to_string());
            }
            let kind = entry.unix_mode().map(|mode| mode & S_IFMT);
            if entry.is_symlink()
                || kind.is_some_and(|kind| kind != 0 && kind != S_IFREG && kind != S_IFDIR)
            {
                return Err(UiMsg::ArchiveHasLink.to_string());
            }
            if entry.encrypted()
                || !matches!(
                    entry.compression(),
                    CompressionMethod::Stored | CompressionMethod::Deflated
                )
            {
                return Err(broken(format!("{name}: unsupported entry")));
            }
            if entry.is_dir() {
                continue;
            }
            index.insert(name, i);
        }
        Ok(Self {
            archive,
            index,
            read_total: 0,
            limits,
        })
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// 讀一個條目（沒有回 None）；超過 `cap` 或總解壓量超限回「太大」。
    pub(crate) fn read(&mut self, name: &str, cap: u64) -> Result<Option<Vec<u8>>, String> {
        let Some(&i) = self.index.get(name) else {
            return Ok(None);
        };
        let entry = self
            .archive
            .by_index(i)
            .map_err(|error| broken(format!("{name}: {error}")))?;
        let mut bytes = Vec::new();
        entry
            .take(cap + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| broken(format!("{name}: {error}")))?;
        let read = bytes.len() as u64;
        self.read_total += read;
        if read > cap || self.read_total > self.limits.total_bytes {
            return Err(too_large());
        }
        Ok(Some(bytes))
    }

    /// 讀一個必須存在的 JSON 條目並解析
    pub(crate) fn json<T: serde::de::DeserializeOwned>(
        &mut self,
        name: &str,
        cap: u64,
    ) -> Result<T, String> {
        let bytes = self
            .read(name, cap)?
            .ok_or_else(|| broken(format!("{name} missing")))?;
        serde_json::from_slice(&bytes).map_err(|error| broken(format!("{name}: {error}")))
    }
}

pub(crate) fn broken(detail: String) -> String {
    UiMsg::VnWorkBroken { detail }.to_string()
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
