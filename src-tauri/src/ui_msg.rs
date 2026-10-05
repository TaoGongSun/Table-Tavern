//! 給畫面看的後端訊息：回代碼與參數，翻譯交給前端（src/shared/ui/backend-text.ts）。
//! 字串格式是 `TTMSG:` 接 serde JSON，仍走既有的 `Result<_, String>`／`invalid_data`。
//! code 一旦落檔就是持久契約，不得改名；新增變體要在 src/i18n/features/backend-msg.ts
//! 補十語系 `be_<code>` 與參數表，`npm run check:i18n` 會從這支檔抽 code 與欄位核對。
//! 包裹別人的錯誤一律用名為 `error` 的欄位：前端只對這個欄位做巢狀翻譯，其餘參數原文代入。

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::data::invalid_data;

pub const MARK: &str = "TTMSG:";
/// 巢狀翻譯最多幾層；超過就原文保留，跟前端一致。
const MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum UiMsg {
    /// 讀寫檔案失敗；error 是系統錯誤原文或另一則 TTMSG。
    IoFailed {
        error: String,
    },

    // ── 桌與桌目錄
    /// 別的操作正握著這張桌的獨佔鎖。
    WorldBusy,
    /// GM 回合生成中或剛提交、正文還沒落檔：卡片變數模式的面板手改先擋（回合結果會蓋掉它）。
    StateEditDuringTurn,
    /// GM 回合生成中或正文還沒落檔：換幕、分岔、退幕先擋，回合不會套到別的幕。
    SceneChangeDuringTurn,
    WorldNotFound,
    /// 桌目錄旁有轉換日誌／暫存／垃圾桶，寫入先擋。
    WorldConverting,
    WorldReadOnly,
    WorldNotReadOnly,
    /// 主資料夾不見了。
    WorldMainMissing,
    /// 桌目錄旁還有轉換或還原留下的東西。
    WorldComboDirty,
    /// 轉換／還原途中目錄組合變成表格沒列的樣子。
    WorldComboUnexpected,
    /// 桌檔內容讀不懂；detail 是技術細節原文。
    WorldDataInvalid {
        detail: String,
    },
    /// 轉換／還原日誌讀不懂；detail 是技術細節原文。
    OpLogInvalid {
        detail: String,
    },
    NoMigrationPath {
        from: u64,
        to: u64,
    },
    BackupNewer,
    BackupNotFound,
    NoPreMigrationBackup,
    RenameFailed {
        from: String,
        to: String,
    },
    RenameFailedIo {
        from: String,
        to: String,
        error: String,
    },
    RemoveFailed {
        path: String,
    },
    TargetExists {
        path: String,
    },
    DataRootNotFound {
        path: String,
    },
    PathOutsideWorld {
        path: String,
    },

    // ── 設定檔與贊助包
    ConfigNotObject,
    ConfigPatchNotObject,
    ConfigFieldPatchInvalid {
        key: String,
    },
    ConfigFieldNotObject {
        key: String,
    },
    SponsorPackInvalidJson {
        error: String,
    },
    SponsorPackNotObject,
    SponsorPackWrongType,
    SponsorPackBadFormat,

    // ── 角色卡與世界書
    WorldbookEntryNotFound {
        uid: String,
    },
    EntryUntitled,
    PlayerCardExists,
    PlayerCardNotConvertible,
    CharacterNotFound {
        id: String,
    },

    // ── 幕與紀錄
    SceneForkNotEarlier,
    SceneNothingToContinue,
    SceneFirstNoPrevious,
    SceneRewindHasNewContent,
    SummaryFirstScene,
    SummaryContinuedScene,
    SummaryHasNewContent,
    SceneEmptyCannotAdvance,
    PreviousSceneEmpty,
    TranscriptEmpty,
    SceneNotFound {
        scene: u64,
    },

    // ── 匯入／匯出與圖片
    CardJsonInvalid {
        error: String,
    },
    CardMissingName,
    WorldbookJsonInvalid {
        error: String,
    },
    CardNothingToImport,
    /// PNG 結構壞掉；detail 是技術細節原文。
    PngInvalid {
        detail: String,
    },
    CardPngNoData,
    /// 卡片 PNG 裡的角色資料解不開；detail 是技術細節原文。
    CardDataInvalid {
        detail: String,
    },
    ImageNotPng,
    InvalidBase64,
    InvalidFileName,
    UnsupportedImageFormat,
    ImageMissingInReply,
    ImageMissingInReplyTail {
        tail: String,
    },
    ImportReceiptCorrupt {
        error: String,
    },
    NoImportToUndo,

    // ── AI 卡重構
    RefactorShellConflict {
        branch: String,
    },
    RefactorValueMismatch {
        first: String,
        first_value: String,
        second: String,
        second_value: String,
    },
    RefactorRuleMismatch {
        path: String,
        target: String,
    },
    RefactorPathConflict {
        path: String,
    },
    RefactorGroupSpanMissing {
        group: String,
        title: String,
        span: String,
    },
    RefactorSpanMissing {
        span: String,
    },
    /// 重構卡封套／PNG 讀不懂或沒過驗證；detail 是技術細節原文。
    RefactorCardInvalid {
        detail: String,
    },
    /// 重構卡封套版本比這版 App 新。
    RefactorCardNewer,

    // ── 更新器與版本庫
    UpdateNotChecked,
    UpdateAlreadyDownloading,
    /// 已有一次安裝在跑（更新槽或安裝鎖）。
    UpdateAlreadyInstalling,
    UpdateNotDownloaded,
    /// 下載的版本跟要求安裝的版本對不上。
    UpdateVersionMismatch,
    /// 槽裡的版本已被後來的檢查換掉；前端靠這個碼重新顯示最新的更新資訊。
    UpdateChanged,
    /// 下載或安裝進行中，槽裡卻沒有那份更新。
    UpdateBusy,
    UpdateAutoCheckOff,
    /// 安裝閘門已開，暫停一切寫入。
    UpdateGateClosed,
    /// Mac 無法自動替換 App；前端靠這個碼改給下載頁。
    UpdateCannotReplace,
    /// 版本庫已有下載或安裝在跑。
    VersionStoreBusy,
    PlatformUnsupported,
    InstallerWrongPlatform,
    InstallerNameInvalid,
    /// 解出來的 App 版本跟要裝的不同。
    InstallerVersionMismatch,
    VersionNameInvalid,
    SignatureInvalid,
    ArchivePathUnsafe,
    ArchiveHasLink,
    ArchiveWrongRoot,
    RollbackNotOlder,
    RollbackNotEligible,
    RollbackPlatformMismatch,
    RollbackNoFormat,
    VersionDeleteCurrent,
    VersionInUse,
    VersionNotFound,
    UpdateEndpointInvalid,
    /// 回退點網址不是 https。
    RollbackPointUrlInvalid,
    /// 回退點下載回非成功狀態；status 是 HTTP 狀態原文。
    RollbackPointDownloadFailed {
        status: String,
    },
    SwapRecordMissing,
    AppIdUnavailable,
    ResidueCleanupStuck,
    VersionsSyncFailed {
        error: String,
    },

    // ── AI 連線（API／CLI／續聊線）
    // code 不得含 ai-error.ts 分流正則會認的字樣（rate limit、credential…），否則改變分流；
    // openrouter_api_key_missing 由 explainAiError 明確認碼判成認證錯誤。
    CliWorkspaceFailed {
        error: String,
    },
    GrokProfileFailed {
        error: String,
    },
    CliRiskNotAccepted,
    /// cli 是 claude／codex／agy／grok 的 id 原文。
    CliNotFound {
        cli: String,
    },
    AgyTooOld {
        version: String,
    },
    UnknownTransport {
        transport: String,
    },
    /// tier 是檔位鍵原文（best／balanced／fast）。
    TierModelMissing {
        tier: String,
    },
    OpenrouterApiKeyMissing,
    NoFreeModels,
    NoStableFreeModel,
    /// 穩定名單全都擁擠（都在 exhausted）：不換、不重送。
    SmartFreeAllBusy,
    /// 接在 `AI_HTTP_STATUS_429: ` 後面：前綴留在起首給前端分流。
    SmartFreeDailyExhausted,
    /// Responses API 回報失敗卻沒附原因。
    ResponsesApiFailed,
    /// 包住 CLI 吐的原話或下面幾則 CLI 失敗代碼。
    CliReplyError {
        error: String,
    },
    CliStdinTimeout,
    CliStalled,
    /// status 是程序結束狀態原文，tail 是 stderr 最後幾行原文。
    CliCrashed {
        status: String,
        tail: String,
    },
    CliNoReply {
        status: String,
        tail: String,
    },
    /// CLI 收尾事件報失敗卻沒附原因；cli 是顯示名（Codex／Gemini／Grok）。
    CliTurnFailed {
        cli: String,
    },
    CliTurnFailedStatus {
        cli: String,
        status: String,
    },
    /// 沒有取消訊號卻收到中止；理論上走不到。
    CliUnexpectedAbort,
    AgyConversationMismatch {
        expected: String,
        actual: String,
    },
    AgyLockPoisoned,
    LaneStateWriteFailed {
        path: String,
        error: String,
    },
    SessionAbandonFailed {
        path: String,
        error: String,
    },
    /// provider 是續聊線 id 原文。
    LaneRewriteUnsupported {
        provider: String,
    },

    // ── 畫面說明（非錯誤）：落檔在重構結果、機制帳本、匯入收據、模型清單快取
    // 帳本 detail 會進重構 AI 的脈絡，ai_text 要給英文短句。
    /// 重構審閱：淘汰缺編號或越界，整條退回照搬。
    RefactorDropRuleCarried,
    /// 重構審閱：淘汰缺編號或越界，這一段併進餘段照搬。
    RefactorDropRuleLeftover,
    /// 重構審閱：這一段沒有有效路由，併進餘段條目照搬；title 是產生當時的完整餘段標題。
    RefactorSpanLeftover {
        title: String,
    },
    /// 重構審閱：人物 mode=clean 但段落引用無效；name 是人物名原文。
    RefactorPersonSpanInvalid {
        name: String,
    },
    /// 重構審閱：條目沒出現在任何分類，自動補列照搬。
    RefactorCoverageCarried,
    /// 重構審閱：預掃訊號落在照搬條目卻沒附理由；pattern 是訊號樣式原文。
    RefactorSignalNoReason {
        pattern: String,
    },
    /// 重構審閱「未接管機制」的固定說明：預掃訊號落在照搬條目。
    RefactorSignalOnCarry,
    /// 重構審閱：AI 給的照搬理由；reason 是 AI 原文。
    RefactorCarryReason {
        reason: String,
    },
    /// 機制帳本：AI 卡重構產生的機制條目已接管。
    LedgerRefactorMechanism,
    /// 機制帳本：卡片腳本認不出來，跳過。
    LedgerScriptUnrecognized,
    /// 機制帳本：機制鷹架條目已由本地接管。
    LedgerScaffoldAbsorbed,
    /// 匯入收據的名稱：AI 卡重構套用。
    ReceiptRefactorApply,
    /// 模型下拉的 Claude 官方別名；alias 是別名原文（fable／opus…）。
    CliModelAlias {
        alias: String,
    },
}

impl fmt::Display for UiMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{MARK}{json}")
    }
}

impl From<UiMsg> for String {
    fn from(msg: UiMsg) -> Self {
        msg.to_string()
    }
}

impl UiMsg {
    /// 給 `DataResult`／`?` 用，等同 `invalid_data(msg.to_string())`。
    pub fn into_error(self) -> Box<dyn Error + Send + Sync> {
        invalid_data(self.to_string())
    }

    /// 送 AI 的英文短句。
    fn ai_text(&self, depth: usize) -> String {
        match self {
            UiMsg::IoFailed { error } => {
                format!("File read/write failed: {}", nested(error, depth))
            }
            UiMsg::RefactorDropRuleCarried => {
                "Drop rule missing or not 1-4; the entry was carried over as-is.".to_owned()
            }
            UiMsg::RefactorDropRuleLeftover => {
                "Drop rule missing or not 1-4; this span was merged into the leftover entry."
                    .to_owned()
            }
            UiMsg::RefactorSpanLeftover { title } => {
                format!("This span had no valid route and was merged into the leftover entry \"{title}\".")
            }
            UiMsg::RefactorPersonSpanInvalid { name } => {
                format!("Person \"{name}\" used mode=clean with invalid span references; sent back to the expand queue.")
            }
            UiMsg::RefactorCoverageCarried => {
                "This entry was not classified anywhere and was carried over as-is.".to_owned()
            }
            UiMsg::RefactorSignalNoReason { pattern } => {
                format!("Prescan signal ({pattern}) fell in a carried entry with no reason given.")
            }
            UiMsg::RefactorSignalOnCarry => "Prescan signal fell in a carried entry.".to_owned(),
            UiMsg::RefactorCarryReason { reason } => format!("Carry reason: {reason}"),
            UiMsg::LedgerRefactorMechanism => "Mechanism entry produced by AI card refactor: \
                field rules and trigger tables run locally in the app; the description stays \
                in the worldbook (read-only)."
                .to_owned(),
            UiMsg::LedgerScriptUnrecognized => "Card script not recognized; not converted to a \
                trigger table and not sent to the model by default."
                .to_owned(),
            UiMsg::LedgerScaffoldAbsorbed => "Mechanism scaffold entry; taken over by the local \
                mechanism and no longer sent in the prompt."
                .to_owned(),
            UiMsg::ReceiptRefactorApply => "AI card refactor".to_owned(),
            UiMsg::CliModelAlias { alias } => format!("{alias} (official alias)"),
            // 其餘是只上畫面的操作錯誤，不該進提示詞；萬一進了也給可讀的英文，不讓 AI 看到 JSON。
            other => generic_ai_text(other, depth),
        }
    }

    /// 把字串裡的 TTMSG 換成英文短句，給提示詞用。舊中文、未知或壞碼原樣保留。
    pub fn ai_text_from_str(text: &str) -> String {
        render_ai(text, 0)
    }
}

/// 沒寫專屬英文的代碼：code 拆成英文字、參數照列（`error` 巢狀轉換），例如
/// `World busy`、`Cli not found (cli: agy)`。
fn generic_ai_text(msg: &UiMsg, depth: usize) -> String {
    let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(msg) else {
        return msg.to_string();
    };
    let code = fields
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mut text = code.replace('_', " ");
    if let Some(first) = text.get(..1) {
        text.replace_range(..1, &first.to_ascii_uppercase());
    }
    let params: Vec<String> = fields
        .iter()
        .filter(|(name, _)| name.as_str() != "code")
        .map(|(name, value)| {
            let value = match value {
                serde_json::Value::String(raw) if name == "error" => nested(raw, depth),
                serde_json::Value::String(raw) => raw.clone(),
                other => other.to_string(),
            };
            format!("{name}: {value}")
        })
        .collect();
    if !params.is_empty() {
        text.push_str(&format!(" ({})", params.join(", ")));
    }
    text
}

fn nested(error: &str, depth: usize) -> String {
    if depth + 1 >= MAX_DEPTH {
        error.to_owned()
    } else {
        render_ai(error, depth + 1)
    }
}

fn render_ai(text: &str, depth: usize) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        out.push_str(&rest[..at]);
        let body = &rest[at + MARK.len()..];
        // 括號不成對或 JSON 語法壞掉：只跳過標記本身，裡面可能還夾著合法標記。
        let Some(end) = json_object_end(body)
            .filter(|&end| serde_json::from_str::<serde_json::Value>(&body[..end]).is_ok())
        else {
            out.push_str(MARK);
            rest = body;
            continue;
        };
        // 合法 JSON 但不是認得的訊息：整段原文保留。
        match serde_json::from_str::<UiMsg>(&body[..end]) {
            Ok(msg) => out.push_str(&msg.ai_text(depth)),
            Err(_) => out.push_str(&rest[at..at + MARK.len() + end]),
        }
        rest = &body[end..];
    }
    out.push_str(rest);
    out
}

/// `text` 以 `{` 開頭時，回傳對應右括號之後的位元組位置；跨過字串內的括號與跳脫。
fn json_object_end(text: &str) -> Option<usize> {
    if !text.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(error: impl Into<String>) -> UiMsg {
        UiMsg::IoFailed {
            error: error.into(),
        }
    }

    #[test]
    fn display_is_mark_plus_tagged_json() {
        assert_eq!(
            io("磁碟已滿").to_string(),
            r#"TTMSG:{"code":"io_failed","error":"磁碟已滿"}"#
        );
        let as_string: String = io("x").into();
        assert!(as_string.starts_with(MARK));
        assert_eq!(io("x").into_error().to_string(), io("x").to_string());
    }

    #[test]
    fn ai_text_translates_known_codes_and_keeps_the_rest() {
        let text = format!("前文 {} 後文", io("disk full"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "前文 File read/write failed: disk full 後文"
        );
        assert_eq!(UiMsg::ai_text_from_str("舊的中文說明"), "舊的中文說明");
    }

    #[test]
    fn ai_text_keeps_unknown_or_broken_marks_and_reads_later_good_ones() {
        let unknown = r#"TTMSG:{"code":"nope","error":"x"}"#;
        assert_eq!(UiMsg::ai_text_from_str(unknown), unknown);
        let wrong_type = r#"TTMSG:{"code":"io_failed","error":3}"#;
        assert_eq!(UiMsg::ai_text_from_str(wrong_type), wrong_type);
        let text = format!("TTMSG:{{broken {}", io("a"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "TTMSG:{broken File read/write failed: a"
        );
    }

    #[test]
    fn ai_text_reads_good_marks_inside_a_broken_balanced_one() {
        let text = format!("TTMSG:{{bad {}}}", io("ok"));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            "TTMSG:{bad File read/write failed: ok}"
        );
    }

    #[test]
    fn ai_text_nests_error_up_to_the_depth_limit() {
        let deep = io(io(io(io("root").to_string()).to_string()).to_string()).to_string();
        let rendered = UiMsg::ai_text_from_str(&deep);
        assert!(rendered.starts_with(
            "File read/write failed: File read/write failed: File read/write failed: "
        ));
        assert!(rendered.ends_with(&io("root").to_string()), "{rendered}");
    }

    #[test]
    fn ai_text_turns_ledger_and_review_codes_into_english() {
        let ledger = UiMsg::LedgerScaffoldAbsorbed.to_string();
        assert_eq!(
            UiMsg::ai_text_from_str(&ledger),
            "Mechanism scaffold entry; taken over by the local mechanism and no longer sent in the prompt."
        );
        let reason = UiMsg::RefactorCarryReason {
            reason: "純設定".to_owned(),
        }
        .to_string();
        assert_eq!(UiMsg::ai_text_from_str(&reason), "Carry reason: 純設定");
        // 舊檔的中文原樣送
        let old = "機制鷹架條目，已由本地機制接管，不再送入提示詞。";
        assert_eq!(UiMsg::ai_text_from_str(old), old);
        let broken = r#"TTMSG:{"code":"ledger_scaffold_absorbed""#;
        assert_eq!(UiMsg::ai_text_from_str(broken), broken);
        let unknown = r#"TTMSG:{"code":"ledger_from_the_future"}"#;
        assert_eq!(UiMsg::ai_text_from_str(unknown), unknown);
    }

    #[test]
    fn ai_text_without_a_dedicated_line_reads_code_and_params_not_json() {
        assert_eq!(
            UiMsg::ai_text_from_str(&UiMsg::WorldBusy.to_string()),
            "World busy"
        );
        let nested = UiMsg::RenameFailedIo {
            from: "a".to_owned(),
            to: "b".to_owned(),
            error: io("disk full").to_string(),
        };
        let text = UiMsg::ai_text_from_str(&nested.to_string());
        assert_eq!(
            text,
            "Rename failed io (error: File read/write failed: disk full, from: a, to: b)"
        );
        assert!(!text.contains(MARK));
    }

    #[test]
    fn json_scanner_skips_braces_inside_strings() {
        let text = format!("{} tail", io(r#"a } b \" { c"#));
        assert_eq!(
            UiMsg::ai_text_from_str(&text),
            r#"File read/write failed: a } b \" { c tail"#
        );
    }
}
