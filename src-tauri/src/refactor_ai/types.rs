use crate::data::{FieldRule, Trigger, Visibility};
use crate::refactor::{RefactorCharacter, RefactorInterface};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 條目內容依「空行」切出的一段：start/end 是原文（`WorldbookEntry.content`）的 byte 區間
/// （左閉右開）。id 從 1 起編，各條目各自從 s1 起編，對應 `format_worldbook_entry` 注入的
/// `⟦sN⟧` 標記與小抄裡的 `uid#sN` 引用寫法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntrySpan {
    pub id: usize,
    pub start: usize,
    pub end: usize,
}

/// 結構預掃訊號：某條目某個 span 的原文（不含 `⟦sN⟧` 標記）不分大小寫比對到封閉字彙 pattern
/// 之一，隨 survey user 訊息注入判官參考；一個 span 命中多個 pattern 各記一筆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrescanSignal {
    pub uid: String,
    /// "uid#sN" 格式，對應 `format_worldbook_entry` 標記的 span 引用寫法。
    pub span: String,
    /// 命中的封閉字彙：`"trigger:"`／`"rule:"`／`"逐日樣式"`（第 X 天、每日、day N 三個子樣式
    /// 合併算一個 pattern，命中其中之一即算，不重複計）。
    pub pattern: String,
}

/// 初判結果。recommend 只有 "interface"／"characters" 兩值；解析不出合法值＝整個呼叫回 Err，
/// 由前端照拍板走「不偽造證據、預設介面優先」。
/// run_id／fingerprint（包 2）：claude lane 開了短命 session 時帶回，第二段憑它 resume 承
/// 快取；run_id 空字串＝單發（非 claude lane），第二段直接重送全卡。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorRecommendOutcome {
    pub recommend: String,
    pub evidence: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub raw: String,
}

/// 展開類型：對應前端傳來的 `kind` 字串。人物走專屬的 person_expand_messages、接管走
/// absorb_messages、合組走 group_messages，都不經這裡。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// 狀態欄格式條目：只抽 STATE，不產殼。
    Interface,
    /// 盤點判 playable 的介面條目：STATE＋SHELL。
    InterfaceShell,
}

impl EntryKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "interface" => Ok(Self::Interface),
            "interface_shell" => Ok(Self::InterfaceShell),
            _ => Err(format!(
                "未知的展開類型：{value}（只接受 interface／interface_shell）"
            )),
        }
    }
}

/// GROUPS 條目的種類（取代舊 PLAN 中段的 PlanKind，隨包 3 一併整段刪除）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Setting,
    Mechanism,
}

impl GroupKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "setting" => Ok(Self::Setting),
            "mechanism" => Ok(Self::Mechanism),
            _ => Err(format!(
                "未知的合組種類：{value}（只接受 setting／mechanism）"
            )),
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Setting => "setting",
            Self::Mechanism => "mechanism",
        }
    }
}

/// 盤點結果：人物已經是「認人」後的結果——一人一筆，來源 uid 可能多條（字串——避免前端 JS
/// number 精度問題）；is_player＝盤點階段 AI 標記的疑似玩家本人，整份輸出至多一筆為 true。
/// mode／spans／private_spans 是清爽個案零呼叫組裝用的選配欄：mode="clean" 時 spans 是這個人
/// 全部段落引用（`uid#sN`，行內欄名 `spans:`），private_spans 是其中屬於私密段的子集（行內
/// 欄名 `private:`）；mode 缺席（沿舊格式）或="tangled" 一律照現行 person_expand 流程處理，
/// spans／private_spans 不使用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorSurveyPerson {
    pub name: String,
    pub uids: Vec<String>,
    #[serde(default)]
    pub is_player: bool,
    /// ""｜"clean"｜"tangled"。
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub spans: Vec<String>,
    #[serde(default)]
    pub private_spans: Vec<String>,
}

/// ENTRIES 一行的判定：uid 這條原始條目該怎麼處置。action 是封閉字彙 carry／absorb／drop／
/// split；rule 只有 action="drop" 才有意義（1|2|3|4，對應淘汰四理由）；reason 選填，跟結構
/// 預掃訊號衝突（例如訊號命中卻判 carry）時判官必須附一句。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorEntryVerdict {
    pub uid: String,
    pub action: String,
    #[serde(default)]
    pub rule: Option<u8>,
    #[serde(default)]
    pub reason: String,
}

/// SPLITS 一行：某個 span 的去處。route 封閉字彙 statusbar｜gm｜drop｜person｜entry｜group｜
/// unabsorbed；rule／name／title／group／note 依 route 種類擇一使用（見 `parse_split_line`），
/// 其餘欄位維持空值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorSpanRoute {
    pub span: String,
    pub route: String,
    #[serde(default)]
    pub rule: Option<u8>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub note: String,
}

/// GROUPS 一行：SPLITS 標 group 的 span 們合組成的一條新條目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorSplitGroup {
    pub id: String,
    pub title: String,
    /// "setting"|"mechanism"。
    pub kind: String,
    pub spans: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorSurveyOutcome {
    #[serde(default)]
    pub persons: Vec<RefactorSurveyPerson>,
    /// 全部介面條目 uid（含 playable 與否）。
    #[serde(default)]
    pub interface_uids: Vec<String>,
    /// 其中盤點判 playable（可完全在裡面遊玩）的介面條目 uid：展開時走 interface_shell、產殼；
    /// 其餘介面條目走 interface、只抽 STATE。
    #[serde(default)]
    pub playable_interface_uids: Vec<String>,
    /// 非純人物、非純介面條目的分類判定：一條原始條目一筆。
    #[serde(default)]
    pub verdicts: Vec<RefactorEntryVerdict>,
    /// action=split 條目的逐 span 路由。
    #[serde(default)]
    pub splits: Vec<RefactorSpanRoute>,
    /// SPLITS 用到的 group id 對應的合組宣告。
    #[serde(default)]
    pub groups: Vec<RefactorSplitGroup>,
    /// 狀態欄位命名唯一權威：後續每次展開呼叫的 known_fields 都從這裡起算。
    #[serde(default)]
    pub fields: Vec<String>,
    /// 這份小抄依哪種玩法產出："interface"（保留原卡玩法）｜"characters"（多角色對話）。
    /// 舊產物缺席＝空字串，前端照 interface 行為處理。
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub raw: String,
}

/// 展開結果（介面）：raw 永遠回傳（模型原始輸出，
/// 前端與除錯用，也是解析失敗時的雙軌保底）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorExpandOutcome {
    #[serde(default)]
    pub interface: Option<RefactorInterface>,
    #[serde(default)]
    pub raw: String,
}

/// 人物展開結果：character＝None 代表 AI 完全沒照 EMOJI／PUBLIC／PRIVATE 任何一個標記輸出
/// （多半是離題或整段拒答）；raw 永遠回傳，是這種情況下的雙軌保底，也給前端除錯用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorPersonExpandOutcome {
    #[serde(default)]
    pub character: Option<RefactorCharacter>,
    #[serde(default)]
    pub raw: String,
}

/// carry 產物條目要原樣保留的來源條目元資料：keys／constant／order／disabled／visibility／
/// is_person 直接照抄，套用時 apply() 用這份取代新條目預設值（keys=[]／constant=false／
/// order=遞增計數／visibility=Gm／is_person=false）。只有本地零呼叫組裝（refactor_assemble）
/// 產出的 carry 型條目才會帶這欄；AI 重寫的條目一律沒有。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorEntryMeta {
    pub keys: Vec<String>,
    pub constant: bool,
    pub order: i64,
    pub disabled: bool,
    pub visibility: Visibility,
    pub is_person: bool,
}

/// 新世界書條目：carry 整條照搬、absorb 接管、group 合組、split 逐段路由組裝的產物共用同一種
/// 形狀。locked（被接管唯讀）由套用端依 rules／triggers 是否非空決定，不是 AI 說了算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorNewEntry {
    pub title: String,
    /// "setting" | "mechanism"。
    pub kind: String,
    /// 重寫後的條目全文（markdown）；機制條目＝玩家讀得懂的機制說明。
    pub content: String,
    pub source_uids: Vec<String>,
    /// 機制條目抽出的本地可執行規則；setting 條目恆空。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    /// carry 型條目（原文照搬）才有：原條目 keys/constant/order/disabled/visibility/is_person。
    /// 舊產物 JSON 不帶這欄照舊可解（缺席＝None，apply() 走現行預設）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<RefactorEntryMeta>,
}

/// 條目重寫結果：entry＝None 代表 AI 連 CONTENT 都沒照標記輸出（離題或拒答），raw 雙軌保底。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorRewriteOutcome {
    #[serde(default)]
    pub entry: Option<RefactorNewEntry>,
    #[serde(default)]
    pub raw: String,
}

/// 接管結果：僅 RULES／TRIGGERS 結構化骨架——本文由 App 原文照搬，不經 AI，沒有 CONTENT、
/// 沒有「整條失敗」的概念，抽不出規則就是兩個空集合。raw 永遠回傳，除錯與雙軌保底用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorAbsorbOutcome {
    #[serde(default)]
    pub rules: BTreeMap<String, FieldRule>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub raw: String,
}
