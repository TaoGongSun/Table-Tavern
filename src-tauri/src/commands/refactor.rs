use crate::ai_transport::{chat_transport, prepare_lane_call, stream_via_transport};
use crate::{
    config_root, data, data_root, import, inflight, lanes, receipts, refactor, refactor_ai,
    refactor_assemble, refactor_session, transport, usage_log,
};

/// AI 卡重構中止時的錯誤字串 sentinel：前端靠它分流「玩家主動取消」與其他失敗，一字不差。
pub(crate) const REFACTOR_ABORTED: &str = "refactor-aborted";

/// AI 卡重構套用：玩家勾選的角色／介面／機制落檔，收據記「實際套用的那份」供一鍵倒退。
#[tauri::command]
pub(crate) fn refactor_apply(
    app: tauri::AppHandle,
    world_id: String,
    outcome: refactor::RefactorOutcome,
    selection: refactor::RefactorSelection,
    record_receipt: Option<bool>,
) -> Result<refactor::RefactorApplySummary, String> {
    let root = data_root(&app)?;
    let before = receipts::snapshot(&root, &world_id);
    let result =
        refactor::apply(&root, &world_id, &outcome, &selection).map_err(|error| error.to_string())?;
    if record_receipt.unwrap_or(true) {
        receipts::record_refactor_apply(
            &root,
            &world_id,
            "AI 卡重構",
            result.character_ids,
            result.rewritten_entries,
            result.deleted_entries,
            before,
        );
    }
    Ok(result.summary)
}

/// AI 卡重構定向（初判）：supported 卡在玩家二選一之前的快速判斷，帶全卡只出
/// RECOMMEND＋EVIDENCE 兩行。解析不出合法建議＝Err，前端照拍板走「不偽造證據、預設介面優先」。
/// claude lane 開短命 session（refactor_session）並回 run_id＋卡片指紋，第二段憑它 resume
/// 承前綴快取；其餘 transport 走無狀態單發、run_id 空。
#[tauri::command]
pub(crate) async fn refactor_recommend(
    app: tauri::AppHandle,
    world_id: String,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorRecommendOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let messages = refactor_ai::recommend_messages(&context, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    if chat_transport(&config) == "claude" {
        let call = prepare_lane_call(
            &app,
            &config,
            transport::gm_tier(&config),
            lanes::LaneProvider::Claude,
        )
        .await?;
        let (raw, session_id) = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
            result = refactor_session::open_stage(
                &call,
                &world_id,
                &messages[0].content,
                &messages[1].content,
                |delta| {
                    let _ = on_delta.send(delta.to_owned());
                },
            ) => result?,
        };
        let mut outcome = refactor_ai::parse_recommend(&raw)
            .ok_or_else(|| "refactor-recommend-unparsable".to_owned())?;
        outcome.run_id = session_id;
        outcome.fingerprint = usage_log::text_hash(&context);
        return Ok(outcome);
    }
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::gm_tier(&config),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    refactor_ai::parse_recommend(&raw).ok_or_else(|| "refactor-recommend-unparsable".to_owned())
}

/// AI 卡重構讀卡（盤點階段）：AI 讀整張卡的世界書，認出人物（可能散在好幾條裡）／介面／機制
/// 三類候選。mode＝玩家選定的玩法（interface｜characters），包 1 先透傳進產物供路由與
/// 持久化；模式專屬提示詞與 MODE 回聲核對由包 3 接手。
/// run_id／fingerprint（包 2）＝初判開的短命 session：claude lane＋指紋沒變才 resume 承
/// 快取（只送盤點指示、卡片不重送）；條件不合或 resume 失敗一律降級單發重送全卡。
#[tauri::command]
pub(crate) async fn refactor_survey(
    app: tauri::AppHandle,
    world_id: String,
    mode: String,
    run_id: Option<String>,
    fingerprint: Option<String>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorSurveyOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let entries = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let signals = refactor_ai::prescan_worldbook(&entries);
    let messages = refactor_ai::survey_messages(&context, &signals, &lang, &mode);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let resumed = match (run_id.as_deref(), fingerprint.as_deref()) {
        (Some(rid), Some(fp))
            if !rid.is_empty()
                && chat_transport(&config) == "claude"
                && fp == usage_log::text_hash(&context) =>
        {
            match prepare_lane_call(
                &app,
                &config,
                transport::gm_tier(&config),
                lanes::LaneProvider::Claude,
            )
            .await
            {
                Ok(call) => {
                    let result = tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
                        result = refactor_session::resume_stage(
                            &call,
                            &world_id,
                            rid,
                            &messages[0].content,
                            &messages[1].content,
                            |delta| {
                                let _ = on_delta.send(delta.to_owned());
                            },
                        ) => result,
                    };
                    result.ok()
                }
                Err(_) => None,
            }
        }
        _ => None,
    };
    let raw = match resumed {
        Some(raw) => raw,
        None => tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
            result = stream_via_transport(
                &app,
                &config,
                None,
                false,
                transport::gm_tier(&config),
                Some(&world_id),
                "GM",
                "Output exactly in the requested marker format, nothing else.",
                &messages,
                true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
                |delta| {
                    let _ = on_delta.send(delta.to_owned());
                },
            ) => result?,
        },
    };
    let mut outcome = refactor_ai::parse_survey(&raw);
    // MODE 回聲核對（refactor-mode-split 拍板）：判官回寫的玩法必須與玩家選定一致才收，
    // 跑錯模式的小抄整份拒收；回聲缺席＝無法核對，同樣拒收，前端顯示錯誤讓玩家重跑。
    if outcome.mode != mode {
        return Err("refactor-mode-mismatch".to_owned());
    }
    refactor_ai::normalize_survey_for_mode(&mut outcome);
    // 臨時水印（驗完即刪）：判官對每個人實際寫的 mode，分辨「沒寫」與「明判 tangled」。
    for person in &outcome.persons {
        eprintln!(
            "[survey-persons] name={} mode={:?} uids={:?} spans={:?}",
            person.name, person.mode, person.uids, person.spans
        );
    }
    Ok(outcome)
}

/// AI 卡重構本地組裝（小抄合約 v1）：判官定案後，carry／drop 整條／split 逐段路由／clean
/// 人物這幾類不必再問 AI，App 本地零呼叫組裝＋四項機械稽核。純本地、無 AI 呼叫、不落檔——
/// 產物由前端後續彙整進 RefactorOutcome 送 refactor_apply。
#[tauri::command]
pub(crate) fn refactor_assemble_local(
    app: tauri::AppHandle,
    world_id: String,
    survey: refactor_ai::RefactorSurveyOutcome,
) -> Result<refactor_assemble::RefactorLocalAssembly, String> {
    let root = data_root(&app)?;
    refactor_assemble::assemble_local(&root, &world_id, &survey).map_err(|error| error.to_string())
}

/// AI 卡重構讀卡（展開階段，介面）：system 與盤點同一字串（快取命中），逐條展開成
/// 結構化產物。人物展開走專屬的 refactor_expand_person（一人一次呼叫、可能帶多條來源）。
#[tauri::command]
pub(crate) async fn refactor_expand(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    kind: String,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorExpandOutcome, String> {
    let entry_kind = refactor_ai::EntryKind::parse(&kind)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let entry_text = refactor_ai::entry_full_text(&root, &world_id, &entry_uid)
        .map_err(|error| error.to_string())?;
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::expand_messages(
        &context,
        &entry_uid,
        &entry_text,
        entry_kind,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_expand(entry_kind, &entry_uid, &raw))
}

/// AI 卡重構讀卡（展開階段，人物）：一人一次呼叫，帶上他名下全部來源條目全文（要點 8）；
/// is_player 由盤點結果直接帶過來，不是這裡自己判斷。
#[tauri::command]
pub(crate) async fn refactor_expand_person(
    app: tauri::AppHandle,
    world_id: String,
    name: String,
    uids: Vec<String>,
    is_player: bool,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorPersonExpandOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let mut sources = Vec::with_capacity(uids.len());
    for uid in &uids {
        let text =
            refactor_ai::entry_full_text(&root, &world_id, uid).map_err(|error| error.to_string())?;
        sources.push((uid.clone(), text));
    }
    let messages = refactor_ai::person_expand_messages(&context, &name, &sources, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_person_expand(&raw, &name, &uids, is_player))
}

/// `expand_span_placeholders` 的查表：接 `refactor_assemble::resolve_span` 找段落原文（trim
/// 過）；找不到（uid／段號無效）就回 None，讓佔位符原樣保留。absorb／split_group 共用。
fn span_lookup<'a>(
    by_uid: &'a std::collections::BTreeMap<u64, &'a data::WorldbookEntry>,
) -> impl Fn(&str) -> Option<String> + 'a {
    move |span_ref: &str| {
        refactor_assemble::resolve_span(by_uid, span_ref)
            .map(|(entry, span)| entry.content[span.start..span.end].trim().to_owned())
    }
}

/// AI 卡重構讀卡（接管階段）：ENTRIES 判 absorb 的條目一條一次呼叫。本文由 App 原文照搬＋
/// 鎖定，AI 只補可本地執行的 RULES／TRIGGERS——輸出天生短，取代舊「條目重寫」機制分支。
/// 觸發敘事裡的 `{{span:uid#sN}}` 指位在這裡換回原文全文；解析全空（抽不出規則）也照樣回
/// entry，本文照搬仍然成立，套用端看 rules／triggers 是否非空決定要不要鎖。
#[tauri::command]
pub(crate) async fn refactor_absorb_entry(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorRewriteOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let source = worldbook
        .iter()
        .find(|entry| entry.uid.to_string() == entry_uid)
        .ok_or_else(|| format!("找不到 uid={entry_uid} 的世界書條目"))?;
    let entry_text = refactor_ai::entry_full_text(&root, &world_id, &entry_uid)
        .map_err(|error| error.to_string())?;
    let known_fields = known_fields.unwrap_or_default();
    let messages =
        refactor_ai::absorb_messages(&context, &entry_uid, &entry_text, &known_fields, &lang);
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    let outcome = refactor_ai::parse_absorb(&raw);
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let lookup = span_lookup(&by_uid);
    let triggers = outcome
        .triggers
        .into_iter()
        .map(|mut trigger| {
            trigger.preamble = refactor_ai::expand_span_placeholders(&trigger.preamble, &lookup);
            for case in &mut trigger.cases {
                case.text = refactor_ai::expand_span_placeholders(&case.text, &lookup);
            }
            trigger
        })
        .collect();
    Ok(refactor_ai::RefactorRewriteOutcome {
        entry: Some(refactor_ai::RefactorNewEntry {
            title: source.title.clone(),
            kind: "mechanism".to_owned(),
            content: source.content.clone(),
            source_uids: vec![entry_uid.clone()],
            rules: outcome.rules,
            triggers,
            meta: Some(refactor_assemble::build_meta(source)),
        }),
        raw: outcome.raw,
    })
}

/// AI 卡重構讀卡（合組階段）：SPLITS 標 group 的 span 們合組成一條新條目——一組一次呼叫，拆出
/// 屬於這個主題的資訊、合併改寫（小抄合約 v1 GROUPS 區塊）。CONTENT 裡的 `{{span:uid#sN}}`
/// 指位（大組保險）在這裡換回原文全文。
#[tauri::command]
pub(crate) async fn refactor_split_group(
    app: tauri::AppHandle,
    world_id: String,
    group_id: String,
    title: String,
    kind: String,
    spans: Vec<String>,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorRewriteOutcome, String> {
    let group_kind = refactor_ai::GroupKind::parse(&kind)?;
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let mut materials = Vec::with_capacity(spans.len());
    let mut source_uids: Vec<String> = Vec::new();
    for span_ref in &spans {
        let (entry, span) = refactor_assemble::resolve_span(&by_uid, span_ref)
            .ok_or_else(|| format!("合組 {group_id}（{title}）找不到段落引用：{span_ref}"))?;
        materials.push((
            span_ref.clone(),
            entry.content[span.start..span.end].trim().to_owned(),
        ));
        let uid = entry.uid.to_string();
        if !source_uids.contains(&uid) {
            source_uids.push(uid);
        }
    }
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::group_messages(
        &context,
        &title,
        group_kind,
        &materials,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    let mut outcome = refactor_ai::parse_group(&raw, &title, group_kind, &source_uids);
    if let Some(entry) = outcome.entry.as_mut() {
        let lookup = span_lookup(&by_uid);
        entry.content = refactor_ai::expand_span_placeholders(&entry.content, &lookup);
    }
    Ok(outcome)
}

/// AI 卡重構讀卡（展開階段，statusbar 段）：SPLITS route=statusbar 的段落材料＝該條全部
/// statusbar 段原文串接，走既有 interface 型呼叫（只抽 STATE、永不產殼——這些段落本來就只是
/// 介面格式，不是完整可玩介面）。spans 內每個引用共享同一個來源 uid（route=statusbar 不跨
/// 條目），entry_uid 只用來標記結果的 source_uids。
#[tauri::command]
pub(crate) async fn refactor_expand_spans(
    app: tauri::AppHandle,
    world_id: String,
    entry_uid: String,
    spans: Vec<String>,
    known_fields: Option<Vec<String>>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<refactor_ai::RefactorExpandOutcome, String> {
    let config = data::read_config(&config_root(&app)?).map_err(|error| error.to_string())?;
    let lang = transport::ui_language(&config);
    let root = data_root(&app)?;
    let context =
        refactor_ai::assemble_card_context(&root, &world_id).map_err(|error| error.to_string())?;
    let worldbook = data::read_worldbook(&root, &world_id).map_err(|error| error.to_string())?;
    let by_uid: std::collections::BTreeMap<u64, &data::WorldbookEntry> =
        worldbook.iter().map(|entry| (entry.uid, entry)).collect();
    let mut parts = Vec::with_capacity(spans.len());
    for span_ref in &spans {
        let (entry, span) = refactor_assemble::resolve_span(&by_uid, span_ref)
            .ok_or_else(|| format!("找不到段落引用：{span_ref}"))?;
        parts.push(entry.content[span.start..span.end].trim().to_owned());
    }
    let material = parts.join("\n\n");
    let known_fields = known_fields.unwrap_or_default();
    let messages = refactor_ai::expand_messages(
        &context,
        &entry_uid,
        &material,
        refactor_ai::EntryKind::Interface,
        &known_fields,
        &lang,
    );
    let (_guard, mut cancel) = inflight::register(&world_id);
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(REFACTOR_ABORTED.to_owned()),
        result = stream_via_transport(
            &app,
            &config,
            None,
            false,
            transport::refactor_expand_tier(&config, &chat_transport(&config)),
            Some(&world_id),
            "GM",
            "Output exactly in the requested marker format, nothing else.",
            &messages,
            true, // 思考增量餵進度字尾：玩家分得出「在想」與「掛了」
            |delta| {
                let _ = on_delta.send(delta.to_owned());
            },
        ) => result?,
    };
    Ok(refactor_ai::parse_expand(
        refactor_ai::EntryKind::Interface,
        &entry_uid,
        &raw,
    ))
}

/// AI 卡重構中止：立即殺該桌全部在途呼叫（CLI 殺子程序、API 斷線即停止計費）。
#[tauri::command]
pub(crate) fn refactor_abort(world_id: String) {
    inflight::abort_world(&world_id);
}

/// 讀 AI 卡重構套用介面時可能順便產的靜態渲染殼（interface-shell.html）；沒套用過或那次沒
/// 產出殼就回 None，前端退回保底狀態欄／卡片自帶殼（既有兩層，零改動）。
#[tauri::command]
pub(crate) fn refactor_interface_shell(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Option<String>, String> {
    data::read_interface_shell(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}

/// 桌面玩法標記（refactor-mode-split）：重構套用時寫入；"characters"＝前端停用這桌的
/// 卡片介面 fallback（覆蓋層按鈕不出現、近 10 則掃 raw 的路徑不啟動）。讀取端正規化
/// 舊版壞值：合法大小寫就地修正、未知值回 Err，controller 維持未知不 fallback。
#[tauri::command]
pub(crate) fn refactor_table_mode(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Option<String>, String> {
    let stored = data::read_state(&data_root(&app)?, &world_id)
        .map_err(|error| error.to_string())?
        .refactor_mode;
    refactor::normalize_stored_mode(stored)
}

/// AI 卡重構匯出（結果卡摘要頁用）：產物來自前端 state（就算還沒套用過也能匯出），
/// 直接序列化寫到玩家選的路徑，供之後用「匯入重構產物」讀回重玩。
#[tauri::command]
pub(crate) fn refactor_export_outcome(
    outcome: refactor::RefactorOutcome,
    path: String,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&outcome).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| error.to_string())
}

/// AI 卡重構匯出（世界書工具列用）：讀 apply() 套用成功時桌內落下的存檔；沒有就回固定錯誤
/// 字串（前端比對 "refactor-export-none" 顯示對應提示）。
#[tauri::command]
pub(crate) fn refactor_export_saved(
    app: tauri::AppHandle,
    world_id: String,
    path: String,
) -> Result<(), String> {
    let root = data_root(&app)?;
    let content = data::read_refactor_outcome(&root, &world_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "refactor-export-none".to_owned())?;
    std::fs::write(&path, content).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn refactor_outcome_exists(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<bool, String> {
    Ok(data::read_refactor_outcome(&data_root(&app)?, &world_id)
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
pub(crate) fn card_interfaces(
    app: tauri::AppHandle,
    world_id: String,
) -> Result<Vec<import::CardInterface>, String> {
    import::read_card_interfaces(&data_root(&app)?, &world_id).map_err(|error| error.to_string())
}
