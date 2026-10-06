//! 換幕摘要（計畫 §3.8）：一次放得下就一次；放不下（或模型回「太長」）就分段摘要再合併。
//! - 每一層（分段、合併）都用自己的固定輸入量 F 與輸出空間 R 算可用容量；容量 ≤ 0 或小於最小塊就早退。
//! - 單一事件自己就超過一塊：按段落、句子，再不行按 Unicode 字元邊界硬切。
//! - 中間摘要要求 ≤ 1500 字；回來超過就請模型重寫一次，仍超過就截斷標「（節錄）」，量過才進合併。
//! - 任何一次呼叫回「太長」都把該層塊大小減半重來；所有呼叫（分段、合併、重寫、重試）共用 20 次上限。
//! - 最終失敗（次數耗盡、合併遞迴超過 3 層、最小塊仍太長、容量早退）回 `SceneSummaryFailed`；
//!   其他錯誤（空回覆、截斷、取消、連線）原樣上拋。這裡從不寫檔——呼叫端拿到 Ok 才提交，保持原子性。
use super::estimate::{char_weight, Unit};
use super::measure;
use crate::transport::{self, context_overflow, ChatMessage, SEGMENT_SUMMARY_CHARS};
use crate::ui_msg::UiMsg;

const MAX_CALLS: u32 = 20;
const MAX_DEPTH: u32 = 3;
const MIN_CHUNK_TOKENS: u64 = 4_000;
const MIN_CHUNK_BYTES: u64 = 12_000;

/// 換幕摘要能用的容量。`total` 為 None＝上限未知（自訂 base_url）：先整段送，收到「太長」才開始縮。
#[derive(Debug, Clone)]
pub struct Capacity {
    pub unit: Unit,
    pub total: Option<u64>,
    pub reserve: u64,
    /// 估計校正倍率（bytes 恆 1）
    pub ratio: f64,
    pub transport: String,
    pub lang: String,
}

/// 實際打模型的那一下（真 app 走 GM 檔單發；測試注入假的）。
#[allow(async_fn_in_trait)]
pub trait SummaryCaller {
    async fn call(&mut self, messages: Vec<ChatMessage>) -> Result<String, String>;
}

fn too_long(error: &str) -> bool {
    error.starts_with(context_overflow::CODE)
}

fn failed() -> String {
    UiMsg::SceneSummaryFailed.into()
}

struct Run<'a, C: SummaryCaller> {
    cap: &'a Capacity,
    caller: &'a mut C,
    calls: u32,
    takeover: bool,
}

impl<C: SummaryCaller> Run<'_, C> {
    fn size(&self, messages: &[ChatMessage]) -> u64 {
        let request =
            measure::summary_request_of(messages.to_vec(), &self.cap.lang, &self.cap.transport);
        (measure::size(&request, self.cap.unit) as f64 * self.cap.ratio).ceil() as u64
    }

    /// 一行在請求裡的量：未取整的字元權重加一個分隔，乘校正倍率再取整（與 split_text 同一算法）。
    fn line_size(&self, line: &str) -> u64 {
        let raw: f64 = 1.0
            + line
                .chars()
                .map(|ch| char_weight(self.cap.unit, ch))
                .sum::<f64>();
        (raw * self.cap.ratio).ceil() as u64
    }

    fn min_chunk(&self) -> u64 {
        match self.cap.unit {
            Unit::Tokens => MIN_CHUNK_TOKENS,
            Unit::Bytes => MIN_CHUNK_BYTES,
        }
    }

    async fn call(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
        if self.calls >= MAX_CALLS {
            return Err(failed());
        }
        self.calls += 1;
        self.caller.call(messages).await
    }

    /// 這一層可用的塊容量（budget 扣掉該呼叫自己的固定部分）；比最小塊還小就沒有可行的切法。
    fn chunk_room(&self, budget: u64, fixed: u64) -> Option<u64> {
        budget
            .checked_sub(fixed)
            .filter(|room| *room >= self.min_chunk())
    }

    /// 中間摘要量過才放行：超過 1500 字就請模型縮短——縮短請求一樣量自己的 F／R，放不下就切片各自縮，
    /// 收到「太長」該步減半重來（共用 20 次）；縮完仍超過就截斷並標節錄。
    async fn bounded(&mut self, text: String, mut budget: u64) -> Result<String, String> {
        if text.chars().count() <= SEGMENT_SUMMARY_CHARS {
            return Ok(text);
        }
        let lang = self.cap.lang.clone();
        let fixed = self.size(&transport::shorten_summary_messages("", &lang));
        let shorter = 'shrink: loop {
            let Some(room) = self.chunk_room(budget, fixed) else {
                return Err(failed());
            };
            let pieces = split_text(&text, room, self.cap.unit, self.cap.ratio);
            let mut results = Vec::with_capacity(pieces.len());
            for piece in &pieces {
                match self
                    .call(transport::shorten_summary_messages(piece, &lang))
                    .await
                {
                    Ok(result) => results.push(result),
                    Err(error) if too_long(&error) => {
                        budget /= 2;
                        continue 'shrink;
                    }
                    Err(error) => return Err(error),
                }
            }
            break results.join("\n");
        };
        if shorter.chars().count() <= SEGMENT_SUMMARY_CHARS {
            return Ok(shorter);
        }
        let marker = match transport::scaffold_en(&lang) {
            true => "(excerpt)",
            false => "（節錄）",
        };
        let kept: String = shorter.chars().take(SEGMENT_SUMMARY_CHARS).collect();
        Ok(format!("{kept}\n{marker}"))
    }

    /// 把 `lines` 依「每塊請求 ≤ budget」切塊；做不到（容量 ≤ 0、比最小塊還小）回 None。
    /// 先用逐行加總的估計裝箱，再逐塊用真組裝驗；有一塊超過就把可用量打九折重裝（最多 5 次）。
    fn pack(&self, lines: &[String], budget: u64) -> Option<Vec<Vec<String>>> {
        // 每塊的固定部分：同一份分段指示、塊內沒有任何行（段號位數影響極小，取三位數估）
        let fixed = self.size(&transport::segment_summary_messages(
            &[],
            999,
            999,
            &self.cap.lang,
        ));
        let mut avail = self.chunk_room(budget, fixed)?;
        for _ in 0..5 {
            if avail < self.min_chunk() {
                return None;
            }
            let chunks = self.greedy(lines, avail);
            let parts = chunks.len();
            let fits = chunks.iter().enumerate().all(|(index, chunk)| {
                self.size(&transport::segment_summary_messages(
                    chunk,
                    index + 1,
                    parts,
                    &self.cap.lang,
                )) <= budget
            });
            if fits {
                return Some(chunks);
            }
            avail = avail * 9 / 10;
        }
        None
    }

    fn greedy(&self, lines: &[String], avail: u64) -> Vec<Vec<String>> {
        let mut pieces: Vec<String> = Vec::new();
        for line in lines {
            if self.line_size(line) <= avail {
                pieces.push(line.clone());
            } else {
                pieces.extend(split_text(line, avail, self.cap.unit, self.cap.ratio));
            }
        }
        let mut chunks: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        let mut used = 0;
        for piece in pieces {
            let size = self.line_size(&piece);
            if !current.is_empty() && used + size > avail {
                chunks.push(std::mem::take(&mut current));
                used = 0;
            }
            used += size;
            current.push(piece);
        }
        if !current.is_empty() {
            chunks.push(current);
        }
        chunks
    }

    /// 一層：切塊、逐塊摘要、合併；合併放不下就把中間摘要當下一層的行再來一次。
    async fn layer(
        &mut self,
        lines: Vec<String>,
        mut budget: u64,
        depth: u32,
    ) -> Result<String, String> {
        if depth > MAX_DEPTH {
            return Err(failed());
        }
        let lang = self.cap.lang.clone();
        'shrink: loop {
            // 容量為正但比最小塊還小：沒有可行的切法，一次都不送
            let Some(chunks) = self.pack(&lines, budget) else {
                return Err(failed());
            };
            let parts = chunks.len();
            let mut segments = Vec::with_capacity(parts);
            for (index, chunk) in chunks.iter().enumerate() {
                match self
                    .call(transport::segment_summary_messages(
                        chunk,
                        index + 1,
                        parts,
                        &lang,
                    ))
                    .await
                {
                    Ok(text) => segments.push(self.bounded(text, budget).await?),
                    Err(error) if too_long(&error) => {
                        budget /= 2;
                        continue 'shrink;
                    }
                    Err(error) => return Err(error),
                }
            }
            let merge = transport::merge_summary_messages(&segments, &lang, self.takeover);
            if self.size(&merge) <= budget {
                match self.call(merge).await {
                    Ok(reply) => return Ok(reply),
                    Err(error) if !too_long(&error) => return Err(error),
                    Err(_) => budget /= 2,
                }
            }
            // 合併放不下（或模型說太長）：中間摘要當下一層的行
            return Box::pin(self.layer(segments, budget, depth + 1)).await;
        }
    }
}

/// 換幕摘要：回最終回覆（第一行標題＋前情提要，交給 `extract_scene_title`）。
/// `takeover`：介面接管桌（`data::is_interface_takeover`），整幕與合併的提示詞多一句禁標記。
pub async fn summarize_scene<C: SummaryCaller>(
    events: &[crate::data::TranscriptEvent],
    cap: &Capacity,
    takeover: bool,
    caller: &mut C,
) -> Result<String, String> {
    let mut run = Run {
        cap,
        caller,
        calls: 0,
        takeover,
    };
    let whole = transport::summary_messages(events, &cap.lang, takeover);
    let whole_size = run.size(&whole);
    let budget = cap.total.map(|total| total.saturating_sub(cap.reserve));
    let fits = budget.is_none_or(|budget| whole_size <= budget);
    let mut budget = budget.unwrap_or(whole_size);
    if fits {
        match run.call(whole).await {
            Ok(reply) => return Ok(reply),
            Err(error) if too_long(&error) => budget = budget.min(whole_size) / 2,
            Err(error) => return Err(error),
        }
    }
    let lines = transport::summary_lines(events, &cap.lang);
    run.layer(lines, budget, 1).await
}

/// 玩家按停止：包住真正的呼叫，停止訊號一到就丟掉在途呼叫（CLI 子程序隨 future 被 drop 收掉），
/// 之後的段一律不再送；換幕流程拿到錯誤就不提交。
pub struct Cancellable<C> {
    pub inner: C,
    pub cancel: Option<crate::inflight::CancelSignal>,
    stopped: bool,
}

impl<C> Cancellable<C> {
    pub fn new(inner: C, cancel: Option<crate::inflight::CancelSignal>) -> Self {
        Cancellable {
            inner,
            cancel,
            stopped: false,
        }
    }
}

fn stopped() -> String {
    UiMsg::SceneSummaryStopped.into()
}

impl<C: SummaryCaller> SummaryCaller for Cancellable<C> {
    async fn call(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
        if self.stopped {
            return Err(stopped());
        }
        let Some(cancel) = self.cancel.as_mut() else {
            return self.inner.call(messages).await;
        };
        let call = self.inner.call(messages);
        tokio::pin!(call);
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                self.stopped = true;
                Err(stopped())
            }
            result = call.as_mut() => result,
        }
    }
}

/// 換幕指令本體：桌級寫入許可在這裡拿、函式返回就放（成功、失敗、停止都一樣）。
pub async fn advance_locked<C: SummaryCaller>(
    root: &std::path::Path,
    world_id: &str,
    cap: &Capacity,
    caller: &mut C,
) -> Result<u64, String> {
    let _permit = crate::data::world_write_permit_async(world_id).await?;
    advance_with(root, world_id, cap, caller).await
}

/// 換幕：整理本幕、全部成功才開新幕（失敗一律零寫入，原紀錄不動）。
pub async fn advance_with<C: SummaryCaller>(
    root: &std::path::Path,
    world_id: &str,
    cap: &Capacity,
    caller: &mut C,
) -> Result<u64, String> {
    let state = crate::data::read_state(root, world_id).map_err(|error| error.to_string())?;
    let events = crate::data::read_transcript(root, world_id, state.current_scene)
        .map_err(|error| error.to_string())?;
    if events.is_empty() {
        return Err(UiMsg::SceneEmptyCannotAdvance.into());
    }
    let takeover =
        crate::data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    let reply = summarize_scene(&events, cap, takeover, caller).await?;
    // 換幕順手取幕名：回覆第一行「標題：…」／「Title: …」解析不到就整段當摘要，不報錯
    let (title, summary) = transport::extract_scene_title(&reply);
    crate::data::begin_next_scene(root, world_id, &summary, title.as_deref())
        .map_err(|error| error.to_string())
}

/// 重寫前情提要指令本體：持桌級寫入許可檢查能不能重寫（第一幕、分岔來的幕、已有新內容、前幕空的都不行），
/// 再整理前一幕；函式返回就放許可。
pub async fn regenerate_locked<C: SummaryCaller>(
    root: &std::path::Path,
    world_id: &str,
    cap: &Capacity,
    caller: &mut C,
) -> Result<(), String> {
    let _permit = crate::data::world_write_permit_async(world_id).await?;
    let text = |error: Box<dyn std::error::Error + Send + Sync>| error.to_string();
    let state = crate::data::read_state(root, world_id).map_err(text)?;
    let scene = state.current_scene;
    let label = crate::data::scene_label(&state, scene);
    let Some(previous_scene) = label.parent else {
        return Err(UiMsg::SummaryFirstScene.into());
    };
    if label.forked {
        return Err(UiMsg::SummaryContinuedScene.into());
    }
    let current_events = crate::data::read_transcript(root, world_id, scene).map_err(text)?;
    if current_events.len() != 1 {
        // 早退：這一幕已經有新內容，不值得先花一次模型呼叫才發現不能用
        return Err(UiMsg::SummaryHasNewContent.into());
    }
    let previous_events =
        crate::data::read_transcript(root, world_id, previous_scene).map_err(text)?;
    if previous_events.is_empty() {
        return Err(UiMsg::PreviousSceneEmpty.into());
    }
    let takeover =
        crate::data::is_interface_takeover(root, world_id, state.refactor_mode.as_deref());
    regenerate_with(root, world_id, &previous_events, cap, takeover, caller).await
}

/// 重寫前情提要：整理前一幕、全部成功才覆寫目前這幕的那則摘要。
pub async fn regenerate_with<C: SummaryCaller>(
    root: &std::path::Path,
    world_id: &str,
    previous_events: &[crate::data::TranscriptEvent],
    cap: &Capacity,
    takeover: bool,
    caller: &mut C,
) -> Result<(), String> {
    let reply = summarize_scene(previous_events, cap, takeover, caller).await?;
    let (title, summary) = transport::extract_scene_title(&reply);
    crate::data::replace_scene_summary(root, world_id, &summary, title.as_deref())
        .map_err(|error| error.to_string())
}

/// 切一段太長的文字：先按空行段落，再按句末標點，最後按 Unicode 字元；每片的量（與 `Run::line_size`
/// 同一算法：未取整的字元權重加一個分隔，統一乘校正倍率再取整）≤ `room`。
/// 字元硬切累加未取整的權重，不重掃整片、也不因逐字取整讓小權重的字被算成 0。
fn split_text(text: &str, room: u64, unit: Unit, ratio: f64) -> Vec<String> {
    let scaled = |raw: f64| (raw * ratio).ceil() as u64;
    let raw_of = |piece: &str| 1.0 + piece.chars().map(|ch| char_weight(unit, ch)).sum::<f64>();
    let fits = |piece: &str| scaled(raw_of(piece)) <= room;
    let mut out = Vec::new();
    for paragraph in split_keep(text, |ch, next| ch == '\n' && next == Some('\n')) {
        if fits(&paragraph) {
            out.push(paragraph);
            continue;
        }
        for sentence in split_keep(&paragraph, |ch, _| "。！？!?.；;\n".contains(ch)) {
            if fits(&sentence) {
                out.push(sentence);
                continue;
            }
            // 沒有段落、句界：按 Unicode 字元邊界硬切，不切開多 byte 字元
            let mut piece = String::new();
            let mut raw = 1.0;
            for ch in sentence.chars() {
                let weight = char_weight(unit, ch);
                if !piece.is_empty() && scaled(raw + weight) > room {
                    out.push(std::mem::take(&mut piece));
                    raw = 1.0;
                }
                piece.push(ch);
                raw += weight;
            }
            if !piece.is_empty() {
                out.push(piece);
            }
        }
    }
    // 小片再合併回不超過的大小，避免切出一大堆小塊
    let mut merged: Vec<String> = Vec::new();
    for piece in out {
        match merged.last_mut() {
            Some(last) if fits(&format!("{last}{piece}")) => last.push_str(&piece),
            _ => merged.push(piece),
        }
    }
    merged
}

/// 依分隔條件切開但保留分隔符在前一片尾端。
fn split_keep(text: &str, is_end: impl Fn(char, Option<char>) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        if is_end(ch, chars.peek().copied()) {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests;
