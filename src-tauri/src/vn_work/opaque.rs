//! `.ttvn` 的不透明外殼（plans/vn-pack-opaque.md §1）。
//! 檔頭 16 位元組：`TTVNPK\x01\x00` 加 8 位元組 seed。後面整段 zip 逐位元組 XOR 一條看位置就能算的 keystream。
//! 目的只是讓雙擊與解壓軟體認不出 zip，不是加密。沒有這個檔頭的舊明文 zip 照原樣讀。
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MAGIC: &[u8] = b"TTVNPK\x01\x00";
const HEADER: usize = 16;
const CHUNK: usize = 8 << 10;

pub(crate) struct Opaque<T> {
    inner: T,
    /// 檔頭長度。明文模式是 0，zip 從檔案開頭算。
    offset: u64,
    /// `None`＝明文，不 XOR。
    seed: Option<u64>,
    /// 相對 zip 起點的位置，跟 keystream 對齊。
    pos: u64,
}

/// 寫檔頭，回傳包在檔案外面的轉接器。seed 取系統時間奈秒再過一次 splitmix64。
pub(crate) fn create(mut file: File) -> io::Result<Opaque<File>> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let seed = splitmix64(nanos);
    let mut header = [0u8; HEADER];
    header[..MAGIC.len()].copy_from_slice(MAGIC);
    header[MAGIC.len()..].copy_from_slice(&seed.to_le_bytes());
    file.write_all(&header)?;
    Ok(Opaque {
        inner: file,
        offset: HEADER as u64,
        seed: Some(seed),
        pos: 0,
    })
}

/// magic 對就走不透明模式。對不上、或檔案連檔頭都不到，倒回開頭當明文 zip。
pub(crate) fn open(mut file: File) -> io::Result<Opaque<File>> {
    let mut header = [0u8; HEADER];
    match file.read_exact(&mut header) {
        Ok(()) if header[..MAGIC.len()] == *MAGIC => {
            let mut seed = [0u8; 8];
            seed.copy_from_slice(&header[MAGIC.len()..]);
            Ok(Opaque {
                inner: file,
                offset: HEADER as u64,
                seed: Some(u64::from_le_bytes(seed)),
                pos: 0,
            })
        }
        Ok(()) => plaintext(file),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => plaintext(file),
        Err(error) => Err(error),
    }
}

fn plaintext(mut file: File) -> io::Result<Opaque<File>> {
    file.seek(SeekFrom::Start(0))?;
    Ok(Opaque {
        inner: file,
        offset: 0,
        seed: None,
        pos: 0,
    })
}

impl<T> Opaque<T> {
    pub(crate) fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Write> Write for Opaque<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Some(seed) = self.seed else {
            let written = self.inner.write(buf)?;
            self.pos += written as u64;
            return Ok(written);
        };
        // 不改呼叫者的緩衝，8 KiB 小塊複製後再 XOR，避免整首 mp3 另配一份暫存。
        let take = buf.len().min(CHUNK);
        let mut tmp = [0u8; CHUNK];
        tmp[..take].copy_from_slice(&buf[..take]);
        xor(&mut tmp[..take], seed, self.pos);
        let written = self.inner.write(&tmp[..take])?;
        self.pos += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<T: Read> Read for Opaque<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if let Some(seed) = self.seed {
            xor(&mut buf[..read], seed, self.pos);
        }
        self.pos += read as u64;
        Ok(read)
    }
}

impl<T: Seek> Seek for Opaque<T> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let absolute = match from {
            SeekFrom::Start(at) => {
                let at = at
                    .checked_add(self.offset)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek"))?;
                self.inner.seek(SeekFrom::Start(at))?
            }
            SeekFrom::End(at) => self.inner.seek(SeekFrom::End(at))?,
            SeekFrom::Current(at) => self.inner.seek(SeekFrom::Current(at))?,
        };
        if absolute < self.offset {
            self.inner
                .seek(SeekFrom::Start(self.offset.saturating_add(self.pos)))?;
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek"));
        }
        self.pos = absolute - self.offset;
        Ok(self.pos)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        let absolute = self.inner.stream_position()?;
        if absolute < self.offset {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek"));
        }
        self.pos = absolute - self.offset;
        Ok(self.pos)
    }
}

/// 第 `pos` 個位元組用 `splitmix64(seed ^ (pos >> 3))` 小端的第 `pos & 7` 個位元組；同一個 8 位元組區塊只算一次。
fn xor(buf: &mut [u8], seed: u64, mut pos: u64) {
    let mut rest = buf;
    while !rest.is_empty() {
        let block = splitmix64(seed ^ (pos >> 3)).to_le_bytes();
        let skip = (pos & 7) as usize;
        let take = (8 - skip).min(rest.len());
        let (head, tail) = rest.split_at_mut(take);
        for (byte, key) in head.iter_mut().zip(&block[skip..]) {
            *byte ^= key;
        }
        pos += take as u64;
        rest = tail;
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}
