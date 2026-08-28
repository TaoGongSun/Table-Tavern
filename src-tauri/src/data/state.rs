use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use super::DataResult;
use super::paths::world_dir;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableState {
    #[serde(default)]
    pub table: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tree: BTreeMap<String, StateNode>,
    /// 拒收回饋句：上一輪被系統擋下的更新，跟著逐則快照走，供下一輪模型自癒。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// 上一輪本地套用的變動：路徑（點分）→ 顯示標記（"+5"／"-80"／"更新"）。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<String, String>,
    /// 上一輪觸發表命中的文本：trigger id → 已代換好的文本，跟著逐則快照走。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub triggers: BTreeMap<String, String>,
    /// 全量桌的跳動警示：路徑（點分）→ 顯示標記（"+40"／"-80"）。只給玩家看，不進提示詞；
    /// 每回合重算，跟著逐則快照走，不需另外處理回滾。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub jumps: BTreeMap<String, String>,
}

/// 狀態樹節點保留自然 JSON 形狀，讓匯出的初始值仍可由人閱讀與手改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StateNode {
    Leaf(String),
    Branch(BTreeMap<String, StateNode>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    #[default]
    Number,
    Pair,
    Roll,
    Text,
    List,
    Counter,
    ReadOnly,
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    Delta,
    Replace,
    Local,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectLevel {
    Snapshot,
    Turn,
    Rare,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRule {
    pub kind: FieldKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    pub update: UpdateMode,
    pub inject: InjectLevel,
    /// 角色卡只帶走自己的分支，避免把整桌的規則偷偷塞進單張卡。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// `derived` 專用：算式（見 `evaluator.rs`），以狀態樹為取值來源算出這一欄的值。
    /// 規範書 v1 對 `derived` 只定義「schema 預留，未實作」，沒定公式欄位名——沿用最
    /// 直覺的 `formula`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

impl FieldRule {
    pub fn for_kind(kind: FieldKind) -> Self {
        let (update, inject) = match kind {
            FieldKind::Number | FieldKind::Pair | FieldKind::Counter => {
                (UpdateMode::Delta, InjectLevel::Turn)
            }
            FieldKind::Roll => (UpdateMode::Local, InjectLevel::Turn),
            FieldKind::Text => (UpdateMode::Replace, InjectLevel::Snapshot),
            FieldKind::List => (UpdateMode::Replace, InjectLevel::Turn),
            FieldKind::ReadOnly | FieldKind::Derived => (UpdateMode::Reject, InjectLevel::Rare),
        };
        Self {
            kind,
            min: None,
            max: None,
            update,
            inject,
            branch: None,
            formula: None,
        }
    }
}

/// 觸發條件三型（拍板 13 的四種：計數器門檻與數值區間判斷邏輯逐字相同，
/// 差別只在欄位型別，共用 `Range` 不另立型別）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    /// 數值區間：`min`／`max` 任一可省，`*_exclusive` 為真＝嚴格大於／小於。
    Range {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        min_exclusive: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        max_exclusive: bool,
        /// 路徑不存在時當成這個值（來源腳本的 `{defaults: 0}`）；沒填就當條件不成立。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<f64>,
    },
    /// 字串包含：任一命中即成立。路徑不存在視為空字串。
    Contains { path: String, any: Vec<String> },
    /// 旗標：`expect` 為 false＝「還沒發生過」。路徑不存在視為 false。
    Flag { path: String, expect: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// 區間型：條件成立就持續注入（關係階段、環境氛圍）。
    Range,
    /// 一次性事件：命中後把旗標釘成 true，模型翻不了案——事件演過就是演過。
    Once,
}

/// 一組觸發：來源是一條卡片腳本，`cases` 保留它 if／else-if 鏈的順序語意。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    /// 穩定 id（來源條目標題正規化），`TableState.triggers` 與面板都用它對帳。
    pub id: String,
    pub title: String,
    pub mode: TriggerMode,
    /// 依序求值，第一個命中就停；空 `when` 的那筆＝else 兜底。
    pub cases: Vec<TriggerCase>,
    /// 命中文本前固定加的一段（來源腳本前言，通常是「當隱藏背景、別複述」）。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preamble: String,
    /// 這組觸發看的是哪一支分支（點分路徑分段）——該支不在場就不注入，沿用包 5 的裁切。
    /// 空＝桌級，永遠注入。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// `Once` 專用：命中後要釘成 `true` 的旗標路徑（點分）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerCase {
    /// 全部成立才算命中（AND）；空＝else 兜底。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<Condition>,
    /// 命中要注入的文本。`{{state:World.Invasion}}` 這種佔位在注入前換成現值。
    pub text: String,
}

fn mechanism_version() -> u32 {
    1
}

/// 規則留在桌級設定，逐則快照只存會隨對話改變的狀態值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mechanism {
    #[serde(default = "mechanism_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, FieldRule>,
    /// 觸發表：每回合本地求值，命中文本進下一輪回合尾。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    /// 這桌的數值走增量協定、由本地記帳（MVU 卡匯入後開啟）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub incremental: bool,
    /// 這張卡自己的回報指引：介面接管後跟在通用協定後面進系統提示詞，講這張卡每回合
    /// 必報哪些欄位、哪些只在變動時報。空＝這桌沒有卡專屬規矩，只走通用協定。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub guide: String,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !value
}

impl Default for Mechanism {
    fn default() -> Self {
        Self {
            version: mechanism_version(),
            rules: BTreeMap::new(),
            triggers: Vec::new(),
            incremental: false,
            guide: String::new(),
        }
    }
}

impl Mechanism {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.triggers.is_empty() && !self.incremental
    }
}

/// 手動編輯撞到既有葉子時不覆寫資料夾，避免壞路徑把完整子樹吃掉。
pub fn set_tree_value(
    tree: &mut BTreeMap<String, StateNode>,
    path: &[String],
    value: &str,
) -> bool {
    if path.is_empty() {
        return false;
    }

    fn write(
        branch: &mut BTreeMap<String, StateNode>,
        path: &[String],
        value: &str,
    ) -> Option<bool> {
        let key = &path[0];
        if path.len() == 1 {
            if value.is_empty() {
                return Some(branch.remove(key).is_some());
            }
            let next = StateNode::Leaf(value.to_owned());
            let changed = branch.get(key) != Some(&next);
            if changed {
                branch.insert(key.clone(), next);
            }
            return Some(changed);
        }

        let child = match branch.get_mut(key) {
            Some(StateNode::Branch(child)) => child,
            Some(StateNode::Leaf(_)) => return None,
            None if value.is_empty() => return Some(false),
            None => {
                branch.insert(key.clone(), StateNode::Branch(BTreeMap::new()));
                let Some(StateNode::Branch(child)) = branch.get_mut(key) else {
                    return None;
                };
                child
            }
        };
        let changed = write(child, &path[1..], value)?;
        if changed && child.is_empty() {
            branch.remove(key);
        }
        Some(changed)
    }

    write(tree, path, value).unwrap_or(false)
}

/// 取路徑上的節點；路徑不存在或中途撞到葉子就是 None。
pub fn node_at<'a>(
    tree: &'a BTreeMap<String, StateNode>,
    path: &[String],
) -> Option<&'a StateNode> {
    let (first, rest) = path.split_first()?;
    let node = tree.get(first)?;
    if rest.is_empty() {
        return Some(node);
    }
    match node {
        StateNode::Branch(children) => node_at(children, rest),
        StateNode::Leaf(_) => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub model_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub player_card_id: Option<String>,
    #[serde(default)]
    pub current_scene: u64,
    #[serde(default)]
    pub catchup_summaries: BTreeMap<String, String>,
    // 換幕順手取的幕名：key 是場景號字串（比照 catchup_summaries），沒取到就不進這個表
    #[serde(default)]
    pub scene_titles: BTreeMap<String, String>,
    /// 分岔用的顯示編號：key 是內部幕號字串，比照 scene_titles。沒進這個表的幕＝原線，查詢一律走 scene_label。
    #[serde(default)]
    pub scene_labels: BTreeMap<String, SceneLabel>,
    #[serde(default)]
    pub state: TableState,
    #[serde(default, skip_serializing_if = "Mechanism::is_empty")]
    pub mechanism: Mechanism,
    /// 已做過全樹對齊的場景號；與 current_scene 不同＝這一幕還沒對齊，下一輪 GM 回合送全樹。
    #[serde(default)]
    pub aligned_scene: Option<u64>,
    /// 面板指認的分支綁定：角色卡 id → 狀態樹路徑。沒指認的靠同名自動比對。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub branch_bindings: BTreeMap<String, Vec<String>>,
    /// AI 卡重構套用時選定的玩法："interface"｜"characters"；None＝沒重構過或舊存檔。
    /// characters＝這桌的卡片介面 fallback 全面停用（refactor-mode-split）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refactor_mode: Option<String>,
}

/// 分岔幕的顯示編號：內部幕號單調遞增不變，玩家看到的編號靠這個脫鉤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneLabel {
    /// 玩家看到的幕號，內部 0 起算（前端顯示時再 +1）。
    pub base: u64,
    /// 同一個 base 的第幾個版本，1＝不顯示括號。
    pub version: u32,
    /// 上一幕的內部幕號；退回前幕靠它，不能再假設一定是「幕號 -1」。
    pub parent: Option<u64>,
    /// 這一幕是分岔複製來的（開頭那則是真實對話，不是前情提要）。
    /// 換幕來的幕開頭一定是摘要，分岔來的不是——凡是「改寫開頭那則」的操作都得先問這一格。
    #[serde(default)]
    pub forked: bool,
}

pub fn read_state(root: &Path, world_id: &str) -> DataResult<WorldState> {
    let path = world_dir(root, world_id)?.join("state.json");
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_state(root: &Path, world_id: &str, state: &WorldState) -> DataResult<()> {
    fs::write(
        world_dir(root, world_id)?.join("state.json"),
        serde_json::to_string_pretty(state)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::*;
    use crate::data::test_support::*;

    #[test]
    fn state_round_trips_and_errors_when_file_is_missing() {
        let root = TestRoot::new("state");
        fs::create_dir_all(root.path().join("worlds").join(new_id())).unwrap();
        let missing_id = new_id();
        fs::create_dir_all(root.path().join("worlds").join(&missing_id)).unwrap();
        // 沒有 state.json（不是新流程會出現的情況，但要確定不會誤當空狀態處理）
        assert!(read_state(root.path(), &missing_id).is_err());

        let world_id = create_world(root.path(), "無狀態").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        state.current_scene = 12;
        state
            .model_bindings
            .insert("船長代碼".to_owned(), "balanced".to_owned());
        state
            .catchup_summaries
            .insert("水手代碼".to_owned(), "錯過了序幕".to_owned());
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap(), state);
    }

    #[test]
    fn mechanism_round_trips_and_old_state_defaults_to_empty() {
        let root = TestRoot::new("mechanism-state");
        let world_id = create_world(root.path(), "機制桌").unwrap();
        let mut state = read_state(root.path(), &world_id).unwrap();
        let mut rule = FieldRule::for_kind(FieldKind::Pair);
        rule.min = Some(0.0);
        rule.max = Some(500.0);
        rule.branch = Some("亞瑟".to_owned());
        state.mechanism.rules.insert("亞瑟.HP".to_owned(), rule);
        write_state(root.path(), &world_id, &state).unwrap();
        assert_eq!(read_state(root.path(), &world_id).unwrap(), state);

        fs::write(
            root.path().join(format!("worlds/{world_id}/state.json")),
            r#"{"id":"old","name":"舊桌"}"#,
        )
        .unwrap();
        assert!(read_state(root.path(), &world_id)
            .unwrap()
            .mechanism
            .is_empty());
    }

    #[test]
    fn set_tree_value_creates_overwrites_prunes_and_preserves_leaf_collisions() {
        let path = ["World", "城市", "聲望"].map(str::to_owned);
        let mut tree = BTreeMap::new();
        assert!(!set_tree_value(&mut tree, &path, ""));
        assert!(tree.is_empty());
        assert!(set_tree_value(&mut tree, &path, "10"));
        assert!(set_tree_value(&mut tree, &path, "12"));
        assert_eq!(
            tree["World"],
            StateNode::Branch(BTreeMap::from([(
                "城市".to_owned(),
                StateNode::Branch(BTreeMap::from([(
                    "聲望".to_owned(),
                    StateNode::Leaf("12".to_owned()),
                )])),
            )]))
        );
        assert!(set_tree_value(&mut tree, &path, ""));
        assert!(tree.is_empty());

        tree.insert("World".to_owned(), StateNode::Leaf("不可展開".to_owned()));
        let before = tree.clone();
        assert!(!set_tree_value(&mut tree, &path, "13"));
        assert_eq!(tree, before);
    }

    /// 舊存檔沒有 aligned_scene／branch_bindings（WorldState）與 changes（TableState）
    /// 三個新欄位也要讀得起來，且各自落回預設值（狀態欄二期包 5）。
    #[test]
    fn old_state_json_without_pack5_fields_still_deserializes() {
        let json = r#"{
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "name": "舊桌",
            "state": { "table": { "time": "清晨" } }
        }"#;
        let state: WorldState = serde_json::from_str(json).unwrap();
        assert_eq!(state.aligned_scene, None);
        assert!(state.branch_bindings.is_empty());
        assert!(state.state.changes.is_empty());
        assert_eq!(state.state.table.get("time"), Some(&"清晨".to_owned()));
    }

}
