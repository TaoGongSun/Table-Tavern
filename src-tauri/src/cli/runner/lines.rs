//! 有單行上限的逐行讀取（runaway-output-cap A8）。行為對齊 `tokio::io::Lines::next_line`：
//! 去掉 `\n`／`\r\n`、EOF 前沒有換行的殘段當最後一行、整行湊齊才做 UTF-8 解碼。
//! 取消安全：已讀的位元組存在 `buf`（跟著讀取器活，不在 future 裡）；唯一的 await 是
//! `fill_buf`，被 `select!` 的別支搶先時 BufReader 內的資料沒被 consume，下次照樣讀得到。

use tokio::io::AsyncBufReadExt;

pub(super) enum LineRead {
    Line(String),
    Eof,
    /// 這一行在湊齊之前就超過上限（位元組，不含換行）。
    TooLong,
}

pub(super) struct CappedLines<R> {
    reader: R,
    buf: Vec<u8>,
    cap: usize,
}

impl<R: tokio::io::AsyncBufRead + Unpin> CappedLines<R> {
    pub fn new(reader: R, cap: usize) -> Self {
        Self {
            reader,
            buf: Vec::new(),
            cap,
        }
    }

    pub async fn next_line(&mut self) -> std::io::Result<LineRead> {
        loop {
            let available = self.reader.fill_buf().await?;
            if available.is_empty() {
                if self.buf.is_empty() {
                    return Ok(LineRead::Eof);
                }
                return self.take_line(false);
            }
            let (take, newline) = match available.iter().position(|byte| *byte == b'\n') {
                Some(index) => (index, true),
                None => (available.len(), false),
            };
            if self.buf.len() + take > self.cap {
                // 超限的那段不搬進 buf：記憶體只長到上限為止
                self.buf.clear();
                return Ok(LineRead::TooLong);
            }
            self.buf.extend_from_slice(&available[..take]);
            self.reader.consume(take + usize::from(newline));
            if newline {
                return self.take_line(true);
            }
        }
    }

    fn take_line(&mut self, newline: bool) -> std::io::Result<LineRead> {
        let mut bytes = std::mem::take(&mut self.buf);
        if newline && bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        String::from_utf8(bytes).map(LineRead::Line).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};

    async fn collect(input: &[u8], cap: usize, capacity: usize) -> Vec<Result<String, String>> {
        let reader = BufReader::with_capacity(capacity, input);
        let mut lines = CappedLines::new(reader, cap);
        let mut out = Vec::new();
        loop {
            match lines.next_line().await {
                Ok(LineRead::Line(line)) => out.push(Ok(line)),
                Ok(LineRead::Eof) => break,
                Ok(LineRead::TooLong) => {
                    out.push(Err("too long".to_owned()));
                    break;
                }
                Err(error) => {
                    out.push(Err(error.to_string()));
                    break;
                }
            }
        }
        out
    }

    #[tokio::test]
    async fn matches_tokio_lines_for_crlf_and_trailing_fragment() {
        let input = "第一行\r\n第二行\n\n最後沒換行".as_bytes();
        let ours = collect(input, 1024, 3).await;
        let mut theirs = Vec::new();
        let mut lines = BufReader::new(input).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            theirs.push(Ok(line));
        }
        assert_eq!(ours, theirs);
    }

    #[tokio::test]
    async fn multibyte_split_across_reads_survives() {
        // 容量 1：每個多位元組字元都被切開
        let out = collect("你好世界\n".as_bytes(), 1024, 1).await;
        assert_eq!(out, vec![Ok("你好世界".to_owned())]);
    }

    #[tokio::test]
    async fn invalid_utf8_is_an_error_like_tokio() {
        let out = collect(b"\xff\xfe\n", 1024, 8).await;
        assert!(matches!(&out[0], Err(message) if message.contains("UTF-8")));
    }

    #[tokio::test]
    async fn line_over_cap_is_reported_before_reading_it_all() {
        let mut input = vec![b'a'; 10];
        input.push(b'\n');
        input.extend(vec![b'b'; 5_000]);
        input.push(b'\n');
        let out = collect(&input, 4_096, 64).await;
        assert_eq!(out, vec![Ok("a".repeat(10)), Err("too long".to_owned())]);
        // 剛好等於上限仍放行
        let mut exact = vec![b'c'; 4_096];
        exact.push(b'\n');
        assert_eq!(
            collect(&exact, 4_096, 64).await,
            vec![Ok("c".repeat(4_096))]
        );
    }

    #[tokio::test]
    async fn cancelled_read_keeps_partial_line() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let mut lines = CappedLines::new(BufReader::new(reader), 1024);
        writer.write_all("半條".as_bytes()).await.unwrap();
        // 讀到半條就被別支搶走（模擬 select! 的 stderr 分支勝出）
        let raced =
            tokio::time::timeout(std::time::Duration::from_millis(50), lines.next_line()).await;
        assert!(raced.is_err());
        writer.write_all("接上\n".as_bytes()).await.unwrap();
        match lines.next_line().await.unwrap() {
            LineRead::Line(line) => assert_eq!(line, "半條接上"),
            _ => panic!("expected a line"),
        }
    }
}
