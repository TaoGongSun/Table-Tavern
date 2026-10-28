use super::*;

/// 份數上限：常駐作者註記每輪都在，最多留這麼多份舊注入〔模型判斷·未裁決〕
const TAIL_STALE_MAX_COPIES: u32 = 8;
/// 比例門檻至少要這麼多份才觸發，保證兩次重開之間的間隔下限〔作者裁決 2026-10-11〕
const TAIL_STALE_RATIO_MIN_COPIES: u32 = 4;
/// 舊尾段佔該路徑固定容量的比例上限（百分比）〔模型判斷·未裁決〕
const TAIL_STALE_RATIO_PERCENT: f64 = 15.0;

/// 舊尾段世界書到門檻了沒：份數到上限，或份數夠且佔容量比例到上限。計數未知（升級前的舊線）一律算到。
fn tail_reopen(state: &LaneState, input: &TurnInput<'_>) -> bool {
    if !input.tail_tracked {
        return false;
    }
    let Some(stale) = state.tail_stale else {
        return true;
    };
    stale.copies >= TAIL_STALE_MAX_COPIES
        || (stale.copies >= TAIL_STALE_RATIO_MIN_COPIES
            && stale
                .percent(input.capacity)
                .is_some_and(|percent| percent >= TAIL_STALE_RATIO_PERCENT))
}

/// FNV-1a 64：跨執行、跨版本皆穩定的事件指紋（std 的雜湊器不保證跨版本一致）。
pub(super) fn events_fingerprint(events: &[TranscriptEvent]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for event in events {
        let kind = match event.kind {
            TranscriptKind::Dialogue => "dialogue",
            TranscriptKind::Narration => "narration",
            TranscriptKind::Player => "player",
            TranscriptKind::System => "system",
        };
        let marker = event
            .marker
            .as_ref()
            .and_then(|marker| serde_json::to_string(marker).ok())
            .unwrap_or_default();
        let gm_only = if event.gm_only { "1" } else { "0" };
        for field in [
            kind,
            &event.speaker_id,
            &event.speaker_name,
            &event.text,
            &marker,
            gm_only,
        ] {
            eat(field.as_bytes());
            eat(&[0x1f]);
        }
        eat(&[0x1e]);
    }
    format!("{hash:016x}")
}

pub(super) enum TurnPlan {
    Resume {
        session_id: String,
        base: usize,
        /// 本輪實際傳給 CLI 的 system（claude 的 --system-prompt-file 內容）。
        system: String,
        patch: Option<String>,
        /// 追平只供用量 log 區分；不改變續聊流程。
        rebased: bool,
    },
    Reopen {
        reason: ReopenReason,
    },
}

#[derive(Clone, Copy)]
pub(super) enum ReopenReason {
    FirstTurn,
    PendingRewrite,
    SceneChanged,
    HistoryRewound,
    HistoryEdited,
    ReplyDiverged,
    ResumeFailed,
    ProviderChanged,
    /// codex 這條線換了 reasoning effort（玩家改檔位覆寫）：同線中途換 effort 對快取的影響不明，直接重開
    EffortChanged,
    SystemChanged,
    /// 未抹線換了開口者（含別的角色解析到同一模型而共用同一 key）
    OwnerChanged,
    /// 未抹線遇到多角色模式（在場變多）
    ModeChanged,
    /// 不抹尾段的角色線（claude 單角色、Agy 一角一線）狀態區塊整塊消失（歷史裡的舊值撤不掉）
    StateBlockGone,
    /// 不抹尾段的線提進 system 的世界書變了（補丁會留在歷史裡，只能重開）
    WorldbookChanged,
    /// 歷史裡的舊尾段世界書累積到門檻（或計數未知），重開壓平
    TailWorldbook,
}

impl ReopenReason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::FirstTurn => "first-turn",
            Self::PendingRewrite => "pending-rewrite",
            Self::SceneChanged => "scene-changed",
            Self::HistoryRewound => "history-rewound",
            Self::HistoryEdited => "history-edited",
            Self::ReplyDiverged => "reply-diverged",
            Self::ResumeFailed => "resume-failed",
            Self::ProviderChanged => "provider-changed",
            Self::EffortChanged => "effort-changed",
            Self::SystemChanged => "system-changed",
            Self::OwnerChanged => "owner-changed",
            Self::ModeChanged => "mode-changed",
            Self::StateBlockGone => "state-block-gone",
            Self::WorldbookChanged => "worldbook-changed",
            Self::TailWorldbook => "tail-worldbook",
        }
    }
}

const ONE_HOUR_CACHE_TTL_SECS: u64 = 3600;

/// 成功一輪後這條 claude 線的估計快取壽命〔模型判斷·未裁決，見 plans/claude-1h-cache.md〕。
/// 只有確定整段都寫成 1 小時才給 3600；混合或時效不明一律保守取 5 分鐘；
/// 純命中沿用上輪（同 session 才算）；讀寫皆 0 或沒拿到用量＝當作已過期，不延壽。
pub(super) fn next_cache_ttl(
    usage: Option<&transport::PromptCacheUsage>,
    prior_ttl: Option<u64>,
    resumed: bool,
) -> u64 {
    let Some(usage) = usage else {
        return 0;
    };
    let created = usage.created_tokens.unwrap_or(0);
    let cached = usage.cached_tokens.unwrap_or(0);
    if created == 0 {
        return match (cached > 0, resumed, prior_ttl) {
            (false, _, _) => 0,
            (true, true, Some(prior)) => prior,
            (true, _, _) => LEGACY_CACHE_TTL_SECS,
        };
    }
    match usage.created_1h_tokens {
        Some(one_hour) if one_hour == created => ONE_HOUR_CACHE_TTL_SECS,
        _ => LEGACY_CACHE_TTL_SECS,
    }
}

/// 決定這一輪續聊還是重開。所有「對不上」都走 Reopen：重開永遠正確，只是少省一次快取。
/// 素材漂移的處置：claude 角色共線的 system 每輪隨旗標重帶，補丁補得動、快取死了還能整份追平；
/// grok 的 system 凍在 session 建立那刻，補丁只是一則 user 訊息、壓不過權重更高的舊 system，
/// 所以 grok 一有漂移就整線重開（改卡、改世界書都是低頻動作）。GM 線三家與 claude 單角色線都一變就重開：
/// world.md／卡／私設的動態巨集讓凍結 system 每輪可能變，補丁會在不抹的歷史裡一份份疊上去〔作者裁決 2026-10-11〕。
/// 歷史裡的舊尾段世界書到門檻（`tail_reopen`）也重開，排在其他原因之後、每個續聊出口之前。
pub(super) fn plan_turn(
    state: Option<&LaneState>,
    input: &TurnInput<'_>,
    now_epoch: u64,
    provider: LaneProvider,
    effort: Option<&str>,
) -> TurnPlan {
    let Some(state) = state else {
        return TurnPlan::Reopen {
            reason: ReopenReason::FirstTurn,
        };
    };
    if state.provider != provider.as_str() {
        return TurnPlan::Reopen {
            reason: ReopenReason::ProviderChanged,
        }; // 線名撞到別家 CLI 開的線：session id 是對方的，resume 一定失敗
    }
    if state.effort.as_deref() != effort {
        return TurnPlan::Reopen {
            reason: ReopenReason::EffortChanged,
        }; // 只有 codex 帶 effort；其他 CLI 兩邊都是 None
    }
    if state.pending_rewrite.is_some() {
        return TurnPlan::Reopen {
            reason: ReopenReason::PendingRewrite,
        }; // 上一輪中途斷掉，session 內容不可信（可能殘留機密段）
    }
    if state.scene != input.scene {
        return TurnPlan::Reopen {
            reason: ReopenReason::SceneChanged,
        }; // 換場＝重開（拍板行為）
    }
    let mut base = state.sent_events;
    if base > input.events.len() {
        return TurnPlan::Reopen {
            reason: ReopenReason::HistoryRewound,
        }; // 正典被收回到水位之前
    }
    if events_fingerprint(&input.events[..base]) != state.sent_hash {
        return TurnPlan::Reopen {
            reason: ReopenReason::HistoryEdited,
        }; // 已送段被改動
    }
    if let Some(expected) = &state.expected_reply {
        match input.events.get(base) {
            Some(event)
                if event.speaker_id == expected.speaker_id
                    && event.kind == expected.kind
                    && event.text == expected.text
                    && event.marker.is_none()
                    && !event.gm_only =>
            {
                base += 1; // 上輪回覆已在 session 裡（assistant），跳過不重送
            }
            _ => {
                return TurnPlan::Reopen {
                    reason: ReopenReason::ReplyDiverged,
                }
            } // 回覆沒落檔或被改＝session 與正典分岔
        }
    }
    // 未抹線（claude 單角色）：先判換人、換模式，緊接著判 StateBlockGone，不能被「同一人續用」短路
    if let Some(owner) = &state.unerased_owner {
        let speaker = match &input.echo {
            ReplyEcho::Dialogue { speaker_id, .. } => Some(speaker_id.as_str()),
            ReplyEcho::Narration => None,
        };
        if speaker != Some(owner.as_str()) {
            return TurnPlan::Reopen {
                reason: ReopenReason::OwnerChanged,
            };
        }
        if input.single_owner.as_deref() != Some(owner.as_str()) {
            return TurnPlan::Reopen {
                reason: ReopenReason::ModeChanged,
            };
        }
    }
    // 不抹尾段的角色線（claude 單角色、Agy 一角一線）：歷史裡的舊狀態區塊撤不掉，整塊消失就重開
    if state.had_state_block && !input.has_state_block {
        return TurnPlan::Reopen {
            reason: ReopenReason::StateBlockGone,
        };
    }
    if state.hoisted_worldbook != worldbook_fingerprint(input.hoisted_worldbook.as_deref()) {
        return TurnPlan::Reopen {
            reason: ReopenReason::WorldbookChanged,
        };
    }
    if provider != LaneProvider::Claude || input.lane == Lane::Gm || input.single_owner.is_some() {
        // snapshot != applied＝升級前快取內補過丁、尚未追平的舊 claude GM／單角色線，歷史裡有補丁，一併重開
        if state.applied != input.frozen_system || state.snapshot != state.applied {
            return TurnPlan::Reopen {
                reason: ReopenReason::SystemChanged,
            };
        }
        if tail_reopen(state, input) {
            return TurnPlan::Reopen {
                reason: ReopenReason::TailWorldbook,
            };
        }
        return TurnPlan::Resume {
            session_id: state.session_id.clone(),
            base,
            system: state.snapshot.clone(),
            patch: None,
            rebased: false,
        };
    }
    // claude 角色共線的兩個續聊出口（追平、補丁）之前
    if tail_reopen(state, input) {
        return TurnPlan::Reopen {
            reason: ReopenReason::TailWorldbook,
        };
    }
    let age = now_epoch.saturating_sub(state.last_call_epoch);
    if age > state.cache_ttl_secs {
        return TurnPlan::Resume {
            session_id: state.session_id.clone(),
            base,
            system: input.frozen_system.clone(),
            patch: None,
            rebased: state.snapshot != input.frozen_system,
        };
    }
    TurnPlan::Resume {
        session_id: state.session_id.clone(),
        base,
        system: state.snapshot.clone(),
        patch: snapshot_patch::render_patch(&state.applied, &input.frozen_system, input.lang),
        rebased: false,
    }
}

/// 提進 system 的世界書段的指紋（存進 lanes.json，不存全文）；沒有或空的算沒有。
pub(super) fn worldbook_fingerprint(worldbook: Option<&str>) -> Option<String> {
    let worldbook = worldbook.filter(|text| !text.is_empty())?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in worldbook.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

pub(super) fn expected_reply_for(echo: &ReplyEcho, reply: &str) -> ExpectedReply {
    match echo {
        // 前端落檔的是收尾後的台詞（剝前綴與控制區塊，chat_with_character 同一個純函式）
        ReplyEcho::Dialogue { speaker_id, prefix } => ExpectedReply {
            speaker_id: speaker_id.clone(),
            kind: TranscriptKind::Dialogue,
            text: transport::finish_character_reply(reply, prefix, false).text,
        },
        // 前端落 transcript 的是剝掉狀態欄與「下一位」點名行的顯示文字（gm_narrate 的行為）。
        // 刻意不做顯示層剝除（strip_state_echo、strip_narrator_label）：剝過的回合對不上→下一輪
        // ReplyDiverged 重開，把帶照抄段的 assistant 訊息清出 session
        ReplyEcho::Narration => ExpectedReply {
            speaker_id: String::new(),
            kind: TranscriptKind::Narration,
            text: transport::extract_next_speaker(&transport::extract_state_block(reply).display).1,
        },
    }
}
