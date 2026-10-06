use super::*;
use crate::data::{self, TranscriptEvent, TranscriptKind};

const LANG: &str = "zh-TW";

fn event(kind: TranscriptKind, name: &str, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        id: None,
        message_vars: None,
        vars_rev: None,
        vars_epoch: None,
        turn_key: None,
        action_id: None,
        raw: None,
        ts: "2026-10-06T09:00:00+08:00".to_owned(),
        speaker_id: String::new(),
        speaker_name: name.to_owned(),
        kind,
        text: text.to_owned(),
        state: None,
        truncated: false,
        gm_only: false,
        marker: None,
        opening: false,
    }
}

fn cap(total: Option<u64>) -> Capacity {
    Capacity {
        unit: Unit::Bytes,
        total,
        reserve: 0,
        ratio: 1.0,
        transport: "agy".to_owned(),
        lang: LANG.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Whole,
    Segment,
    Merge,
    Shorten,
}

fn kind_of(messages: &[ChatMessage]) -> Kind {
    let system = &messages[0].content;
    if system.contains("正在分段整理") {
        Kind::Segment
    } else if system.contains("以下依序是各段摘要") {
        Kind::Merge
    } else if system.contains("縮短到") {
        Kind::Shorten
    } else {
        Kind::Whole
    }
}

/// 假模型：真實上限（bytes，量的是 agy 實送 body）超過就回「太長」；其餘照腳本回。
struct Fake {
    real_limit: u64,
    calls: Vec<(Kind, u64)>,
    segment_reply: String,
    shorten_reply: String,
    /// 第幾次呼叫（1 起算）回這個錯誤
    fail_at: Option<(usize, String)>,
    /// 第幾次呼叫（1 起算）掛著不回（等停止）
    hang_at: Option<usize>,
    inputs: Vec<String>,
}

impl Fake {
    fn new(real_limit: u64) -> Self {
        Fake {
            real_limit,
            calls: Vec::new(),
            segment_reply: "・一段摘要".to_owned(),
            shorten_reply: "・縮短後".to_owned(),
            fail_at: None,
            hang_at: None,
            inputs: Vec::new(),
        }
    }
    fn count(&self, kind: Kind) -> usize {
        self.calls.iter().filter(|(k, _)| *k == kind).count()
    }
}

impl SummaryCaller for Fake {
    async fn call(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
        let kind = kind_of(&messages);
        let size = measure::size(
            &measure::summary_request_of(messages.clone(), LANG, "agy"),
            Unit::Bytes,
        );
        self.calls.push((kind, size));
        self.inputs.push(
            messages
                .iter()
                .skip(1)
                .map(|m| m.content.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        if self.hang_at == Some(self.calls.len()) {
            std::future::pending::<()>().await;
        }
        if let Some((at, error)) = &self.fail_at {
            if *at == self.calls.len() {
                return Err(error.clone());
            }
        }
        if size > self.real_limit {
            return Err(format!("{} too long", context_overflow::CODE));
        }
        Ok(match kind {
            Kind::Whole | Kind::Merge => "標題：雨夜\n\n・合併後的提要".to_owned(),
            Kind::Segment => self.segment_reply.clone(),
            Kind::Shorten => self.shorten_reply.clone(),
        })
    }
}

fn long_scene(blocks: usize, block_chars: usize) -> Vec<TranscriptEvent> {
    let mut events = vec![event(TranscriptKind::Player, "玩家", "開始。")];
    for index in 0..blocks {
        events.push(event(
            TranscriptKind::Narration,
            "GM",
            &format!("第{index}段。{}", "雨".repeat(block_chars)),
        ));
    }
    events
}

fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn fits_in_one_call() {
    let events = long_scene(3, 100);
    let mut fake = Fake::new(190_000);
    let reply = run(summarize_scene(
        &events,
        &cap(Some(190_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert_eq!(fake.calls.len(), 1);
    assert_eq!(fake.calls[0].0, Kind::Whole);
}

#[test]
fn too_big_is_chunked_every_request_within_the_limit_and_all_lines_sent() {
    let events = long_scene(40, 2_000); // 約 240KB
    let mut fake = Fake::new(60_000);
    let reply = run(summarize_scene(
        &events,
        &cap(Some(60_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert!(fake.count(Kind::Segment) >= 4);
    assert_eq!(fake.count(Kind::Merge), 1);
    assert!(fake.calls.iter().all(|(_, size)| *size <= 60_000));
    // 保證範圍：每一行可見原文都進過某一段
    let sent = fake.inputs.join("\n");
    for index in 0..40 {
        assert!(
            sent.contains(&format!("第{index}段。")),
            "missing block {index}"
        );
    }
}

#[test]
fn unknown_limit_sends_whole_then_shrinks_on_too_long() {
    let events = long_scene(30, 2_000);
    let mut fake = Fake::new(50_000);
    let reply = run(summarize_scene(&events, &cap(None), false, &mut fake)).unwrap();
    assert!(reply.starts_with("標題"));
    assert_eq!(fake.calls[0].0, Kind::Whole);
    assert!(fake.count(Kind::Segment) >= 2);
}

#[test]
fn known_limit_misestimate_shrinks_too() {
    // 以為上限 120KB，其實 40KB：分段時收到太長也要縮塊
    let events = long_scene(30, 2_000);
    let mut fake = Fake::new(40_000);
    let reply = run(summarize_scene(
        &events,
        &cap(Some(120_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert!(fake.calls.iter().any(|(_, size)| *size > 40_000));
    assert!(fake.calls.len() <= MAX_CALLS as usize);
}

#[test]
fn single_event_bigger_than_a_chunk_is_hard_cut_on_char_boundaries() {
    // 沒有任何段落或句界、混 emoji 與 CJK
    let text: String = "雨😀".repeat(20_000);
    let events = vec![
        event(TranscriptKind::Player, "玩家", "看。"),
        event(TranscriptKind::Narration, "GM", &text),
    ];
    let mut fake = Fake::new(40_000);
    run(summarize_scene(
        &events,
        &cap(Some(40_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(fake.count(Kind::Segment) >= 3);
    let joined: String = fake
        .inputs
        .iter()
        .zip(&fake.calls)
        .filter(|(_, (kind, _))| *kind == Kind::Segment)
        .map(|(input, _)| {
            input
                .chars()
                .filter(|ch| *ch == '雨' || *ch == '😀')
                .collect::<String>()
        })
        .collect();
    assert!(joined.starts_with("雨😀雨😀"));
    assert_eq!(joined.chars().filter(|ch| *ch == '😀').count(), 20_000);
}

#[test]
fn split_text_never_breaks_a_char_and_respects_fits() {
    let pieces = split_text(&"雨😀é".repeat(100), 50, Unit::Bytes, 1.0);
    assert!(pieces.iter().all(|piece| piece.len() < 50));
    assert_eq!(pieces.concat(), "雨😀é".repeat(100));
    let pieces = split_text("一句。二句。\n\n三段", 8, Unit::Tokens, 1.0);
    assert_eq!(pieces.concat(), "一句。二句。\n\n三段");
}

#[test]
fn long_intermediate_summary_is_rewritten_then_truncated_before_merge() {
    let events = long_scene(20, 2_000);
    let mut fake = Fake::new(60_000);
    fake.segment_reply = "長".repeat(2_000);
    fake.shorten_reply = "還是長".repeat(1_000);
    run(summarize_scene(
        &events,
        &cap(Some(60_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert_eq!(fake.count(Kind::Shorten), fake.count(Kind::Segment));
    let merge_input = fake
        .inputs
        .iter()
        .zip(&fake.calls)
        .find(|(_, (kind, _))| *kind == Kind::Merge)
        .unwrap()
        .0;
    assert!(merge_input.contains("（節錄）"));
    // 每段截到 1500 字：合併輸入不會出現整段 3000 字的重寫結果
    assert!(!merge_input.contains(&"還是長".repeat(600)));
}

#[test]
fn merge_too_big_recurses_into_another_layer() {
    let events = long_scene(30, 2_000);
    let mut fake = Fake::new(20_000);
    // 每段中間摘要接近 1500 字（約 4.5KB）：合併放不下 20KB，要再分一層
    fake.segment_reply = "摘".repeat(1_400);
    let reply = run(summarize_scene(
        &events,
        &cap(Some(20_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert!(fake.count(Kind::Segment) > 30_000 * 2 / 20_000);
    assert!(fake.calls.iter().all(|(_, size)| *size <= 20_000));
}

#[test]
fn smallest_chunk_still_too_long_gives_summary_failed_not_context_error() {
    let events = long_scene(10, 2_000);
    // 真上限比最小塊還小：縮到底仍太長
    let mut fake = Fake::new(3_000);
    let error = run(summarize_scene(&events, &cap(None), false, &mut fake)).unwrap_err();
    assert!(error.contains("scene_summary_failed"), "{error}");
    assert!(!error.starts_with(context_overflow::CODE));
    assert!(fake.calls.len() <= MAX_CALLS as usize);
}

#[test]
fn zero_capacity_exits_before_any_call() {
    let events = long_scene(10, 2_000);
    let mut fake = Fake::new(1_000_000);
    // 上限比固定部分還小：不送任何東西
    let error = run(summarize_scene(&events, &cap(Some(100)), false, &mut fake)).unwrap_err();
    assert!(error.contains("scene_summary_failed"));
    assert!(fake.calls.is_empty());
}

#[test]
fn call_budget_of_twenty_is_shared_by_every_retry() {
    let events = long_scene(200, 2_000); // 約 1.2MB，12KB 一塊要上百段
    let mut fake = Fake::new(14_000);
    let error = run(summarize_scene(
        &events,
        &cap(Some(14_000)),
        false,
        &mut fake,
    ))
    .unwrap_err();
    assert!(error.contains("scene_summary_failed"));
    assert_eq!(fake.calls.len(), MAX_CALLS as usize);
}

fn world_with(events: &[TranscriptEvent], tag: &str) -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!("tt-summarize-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let world = data::create_world(&root, "分段").unwrap();
    for event in events {
        data::append_transcript(&root, &world, 0, event).unwrap();
    }
    (root, world)
}

fn snapshot(root: &std::path::Path, world: &str) -> (u64, usize, usize) {
    let state = data::read_state(root, world).unwrap();
    (
        state.current_scene,
        data::read_transcript(root, world, 0).unwrap().len(),
        data::read_transcript(root, world, 1)
            .map(|events| events.len())
            .unwrap_or(0),
    )
}

/// 第二塊失敗（空回覆）、取消、截斷、最終太長：換幕一律零寫入
#[test]
fn advance_writes_nothing_unless_every_call_succeeds() {
    let events = long_scene(30, 2_000);
    for (tag, failure) in [
        ("empty", Some((2, "AI_EMPTY_RESPONSE: x".to_owned()))),
        (
            "cut",
            Some((
                2,
                "AI_INCOMPLETE_RESPONSE: scene summary truncated".to_owned(),
            )),
        ),
        ("abort", Some((3, "aborted".to_owned()))),
        ("toolong", None),
    ] {
        let (root, world) = world_with(&events, tag);
        let before = snapshot(&root, &world);
        let mut fake = Fake::new(if failure.is_some() { 60_000 } else { 2_000 });
        fake.fail_at = failure;
        let error = run(advance_with(&root, &world, &cap(Some(60_000)), &mut fake)).unwrap_err();
        assert!(!error.starts_with(context_overflow::CODE), "{tag}: {error}");
        assert_eq!(snapshot(&root, &world), before, "{tag}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn advance_commits_once_everything_succeeds() {
    let events = long_scene(30, 2_000);
    let (root, world) = world_with(&events, "ok");
    let mut fake = Fake::new(60_000);
    let scene = run(advance_with(&root, &world, &cap(Some(60_000)), &mut fake)).unwrap();
    assert_eq!(scene, 1);
    let recap = data::read_transcript(&root, &world, 1).unwrap();
    assert_eq!(recap.len(), 1);
    assert!(recap[0].text.contains("合併後的提要"));
    std::fs::remove_dir_all(&root).unwrap();
}

/// 重寫前情提要：收到太長走到底失敗回摘要失敗，原本那則提要不動
#[test]
fn regenerate_keeps_the_old_recap_on_failure() {
    let events = long_scene(30, 2_000);
    let (root, world) = world_with(&events, "regen");
    let mut ok = Fake::new(60_000);
    run(advance_with(&root, &world, &cap(Some(60_000)), &mut ok)).unwrap();
    let before = data::read_transcript(&root, &world, 1).unwrap();
    let mut tiny = Fake::new(2_000);
    let error = run(regenerate_with(
        &root,
        &world,
        &events,
        &cap(None),
        false,
        &mut tiny,
    ))
    .unwrap_err();
    assert!(error.contains("scene_summary_failed"), "{error}");
    let after = data::read_transcript(&root, &world, 1).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(after[0].text, before[0].text);
    std::fs::remove_dir_all(&root).unwrap();
}

/// 中間摘要大到連「縮短」那次都放不下：縮短請求量自己的 F／R、切片各自縮，不直接失敗
#[test]
fn shorten_request_is_measured_and_split_when_it_does_not_fit() {
    let events = long_scene(20, 2_000);
    let mut fake = Fake::new(30_000);
    // 每段中間摘要約 45KB：比可用容量還大
    fake.segment_reply = "長".repeat(15_000);
    fake.shorten_reply = "・縮".to_owned();
    let reply = run(summarize_scene(
        &events,
        &cap(Some(30_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert!(fake.count(Kind::Shorten) > fake.count(Kind::Segment));
    assert!(fake.calls.iter().all(|(_, size)| *size <= 30_000));
}

/// 縮短時模型說太長（容量估錯）：縮短那步減半、切片重來，不直接回摘要失敗
#[test]
fn shorten_too_long_shrinks_instead_of_failing() {
    let events = long_scene(20, 2_000);
    let mut fake = Fake::new(60_000);
    fake.segment_reply = "長".repeat(5_000);
    fake.shorten_reply = "・縮".to_owned();
    // 第一次縮短（第 2 次呼叫）回太長
    fake.fail_at = Some((2, format!("{} too long", context_overflow::CODE)));
    let reply = run(summarize_scene(
        &events,
        &cap(Some(60_000)),
        false,
        &mut fake,
    ))
    .unwrap();
    assert!(reply.starts_with("標題"));
    assert_eq!(fake.calls[1].0, Kind::Shorten);
    // 減半後重送縮短（不回摘要失敗、不重做分段）
    assert_eq!(fake.calls[2].0, Kind::Shorten, "{:?}", fake.calls);
    assert!(fake.calls.len() <= MAX_CALLS as usize);
}

/// 容量為正但比最小塊還小：一次都不送就回摘要失敗
#[test]
fn positive_room_below_the_minimum_chunk_fails_without_calling() {
    let events = long_scene(10, 2_000);
    let fixed = measure::size(
        &measure::summary_request_of(
            transport::segment_summary_messages(&[], 999, 999, LANG),
            LANG,
            "agy",
        ),
        Unit::Bytes,
    );
    let mut fake = Fake::new(1_000_000);
    let error = run(summarize_scene(
        &events,
        &cap(Some(fixed + 5_000)),
        false,
        &mut fake,
    ))
    .unwrap_err();
    assert!(error.contains("scene_summary_failed"), "{error}");
    assert!(fake.calls.is_empty());
}

fn stop_after(world: &str, turn: &str) {
    let world = world.to_owned();
    let turn = turn.to_owned();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        crate::inflight::abort_turn(&world, &turn);
    });
}

/// 停止：在途那一段丟掉、後續段不再送、零提交、桌級許可已放（兩個入口都驗）
#[test]
fn stop_mid_summary_commits_nothing_and_releases_the_table() {
    let events = long_scene(30, 2_000);
    // 換幕
    let (root, world) = world_with(&events, "stop-adv");
    let before = snapshot(&root, &world);
    let (_guard, cancel) = crate::inflight::register_turn(&world, "stop-1");
    let mut fake = Fake::new(60_000);
    fake.hang_at = Some(2);
    let mut caller = Cancellable::new(fake, Some(cancel));
    let error = run(async {
        stop_after(&world, "stop-1");
        advance_locked(&root, &world, &cap(Some(60_000)), &mut caller).await
    })
    .unwrap_err();
    assert!(error.contains("scene_summary_stopped"), "{error}");
    assert_eq!(caller.inner.calls.len(), 2, "第二段掛著時停止，之後不再送");
    assert_eq!(snapshot(&root, &world), before);
    // 許可已放：同一桌馬上拿得到
    run(async { data::world_write_permit_async(&world).await.unwrap() });
    std::fs::remove_dir_all(&root).unwrap();

    // 重寫前情提要：先成功換一次幕，再在重寫途中停止，原提要不動
    let (root, world) = world_with(&events, "stop-regen");
    let mut ok = Fake::new(60_000);
    run(advance_with(&root, &world, &cap(Some(60_000)), &mut ok)).unwrap();
    let recap = data::read_transcript(&root, &world, 1).unwrap();
    let (_guard, cancel) = crate::inflight::register_turn(&world, "stop-2");
    let mut fake = Fake::new(60_000);
    fake.hang_at = Some(1);
    let mut caller = Cancellable::new(fake, Some(cancel));
    let error = run(async {
        stop_after(&world, "stop-2");
        regenerate_locked(&root, &world, &cap(Some(60_000)), &mut caller).await
    })
    .unwrap_err();
    assert!(error.contains("scene_summary_stopped"), "{error}");
    assert_eq!(caller.inner.calls.len(), 1);
    assert_eq!(
        data::read_transcript(&root, &world, 1).unwrap()[0].text,
        recap[0].text
    );
    run(async { data::world_write_permit_async(&world).await.unwrap() });
    std::fs::remove_dir_all(&root).unwrap();
}

/// 重寫前情提要：空回覆、截斷也零寫入
#[test]
fn regenerate_writes_nothing_on_empty_or_truncated() {
    let events = long_scene(30, 2_000);
    for (tag, error) in [
        ("empty", "AI_EMPTY_RESPONSE: x"),
        ("cut", "AI_INCOMPLETE_RESPONSE: scene summary truncated"),
    ] {
        let (root, world) = world_with(&events, &format!("regen-{tag}"));
        let mut ok = Fake::new(60_000);
        run(advance_with(&root, &world, &cap(Some(60_000)), &mut ok)).unwrap();
        let recap = data::read_transcript(&root, &world, 1).unwrap();
        let mut fake = Fake::new(60_000);
        fake.fail_at = Some((2, error.to_owned()));
        let got = run(regenerate_locked(
            &root,
            &world,
            &cap(Some(60_000)),
            &mut fake,
        ))
        .unwrap_err();
        assert_eq!(got, error, "{tag}");
        assert_eq!(
            data::read_transcript(&root, &world, 1).unwrap()[0].text,
            recap[0].text
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}

/// 校正倍率 0.5、token 單位、10 萬個 a（沒有任何句界）：逐字取整會把 a 算成 0，切不開；
/// 改成累加未取整權重後每片都不超過容量，串回去與原文完全一致
#[test]
fn hard_cut_with_small_ratio_and_ascii_respects_room_and_round_trips() {
    let text = "a".repeat(100_000);
    let room = 4_000;
    let pieces = split_text(&text, room, Unit::Tokens, 0.5);
    let size = |piece: &str| ((1.0 + piece.len() as f64 * 0.3) * 0.5).ceil() as u64;
    assert!(pieces.len() > 1);
    assert!(
        pieces.iter().all(|piece| size(piece) <= room),
        "{:?}",
        pieces.iter().map(|p| size(p)).max()
    );
    assert_eq!(pieces.concat(), text);
    // 中文＋emoji、bytes、倍率 1.3 同樣成立
    let mixed = "雨😀a".repeat(5_000);
    let pieces = split_text(&mixed, 1_000, Unit::Bytes, 1.3);
    assert!(pieces
        .iter()
        .all(|piece| ((1.0 + piece.len() as f64) * 1.3).ceil() as u64 <= 1_000));
    assert_eq!(pieces.concat(), mixed);
}

/// 同一情境走完整流程：ratio 0.5 的 token 容量，一個 10 萬 a 的事件照樣切得開、摘要成功
#[test]
fn summarize_cuts_a_huge_ascii_event_under_half_ratio() {
    let events = vec![
        event(TranscriptKind::Player, "玩家", "看。"),
        event(TranscriptKind::Narration, "GM", &"a".repeat(100_000)),
    ];
    // 整包估計約 15,000（ratio 0.5 之後）：上限 8,000 放不下一次，必須真的切塊
    let mut small = cap(Some(8_000));
    small.unit = Unit::Tokens;
    small.ratio = 0.5;
    small.transport = "claude".to_owned();
    struct Counting(Vec<Kind>);
    impl SummaryCaller for Counting {
        async fn call(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
            let kind = kind_of(&messages);
            self.0.push(kind);
            Ok(match kind {
                Kind::Whole | Kind::Merge => "標題：甲\n\n・乙".to_owned(),
                _ => "・段".to_owned(),
            })
        }
    }
    let mut caller = Counting(Vec::new());
    let reply = run(summarize_scene(&events, &small, false, &mut caller)).unwrap();
    assert!(reply.starts_with("標題"));
    assert!(!caller.0.contains(&Kind::Whole), "{:?}", caller.0);
    assert!(
        caller
            .0
            .iter()
            .filter(|kind| **kind == Kind::Segment)
            .count()
            >= 2,
        "{:?}",
        caller.0
    );
    assert_eq!(caller.0.last(), Some(&Kind::Merge));
}
