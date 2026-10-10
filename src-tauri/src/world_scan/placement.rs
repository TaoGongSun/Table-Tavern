//! 觸發條目的放置（方案三之 3）：穩定的定義、前／後兩組（範例上／下併入〔作者裁決 2026-10-10〕）與
//! 作者註記、依深度條目攤平成尾段的一串注入段落（照網頁版 `injections.ts`）。各組內的先後與網頁版
//! `WiResult` 的字串欄位同一套規則（由掃描給的放置前排序推得）。

use crate::data::{Visibility, WorldbookEntry};
use crate::st_macros::engine::is_static;
use crate::world_info::entry::{position, WiEntry};
use crate::world_info::js_semantics::js_trim;

/// 一條觸發的條目。
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub uid: u64,
    pub title: String,
    /// 掃描時代換過的內文（裝飾行已拆掉）
    pub content: String,
    pub position: f64,
    /// outlet 位置的名稱（`{{outlet::名稱}}`）
    pub outlet_name: String,
    pub depth: f64,
    pub role: f64,
    pub constant: bool,
    pub is_person: bool,
    pub visibility: Visibility,
    /// 觸發要靠私密片段才成立、或內文代換讀到私密來源（三之 3 機密分流）
    pub private_trigger: bool,
    /// 穩定（不看可見度與觸發來源的那幾條）
    pub stable: bool,
}

impl Placed {
    pub(super) fn new(
        view: &WorldbookEntry,
        wi: &WiEntry,
        content: String,
        private_trigger: bool,
    ) -> Self {
        Self {
            uid: view.uid,
            title: view.title.clone(),
            content,
            position: wi.position,
            outlet_name: wi.outlet_name.clone(),
            depth: wi.depth,
            role: wi.role,
            constant: wi.constant,
            is_person: view.is_person,
            visibility: view.visibility.clone(),
            private_trigger,
            stable: stable(wi, &view.title),
        }
    }

    /// 限定可見、或私密觸發：角色視角只能進機密段（或 hoist 進該角色自己的 system）。
    pub fn confidential(&self) -> bool {
        self.private_trigger || matches!(self.visibility, Visibility::Characters(_))
    }

    /// constant 人物條目只進名冊行、不送全文（AI 卡重構包 4a）。
    pub fn roster_only(&self) -> bool {
        self.constant && self.is_person
    }

    /// 前（0）與範例上（5）。
    pub fn in_front(&self) -> bool {
        self.position == position::BEFORE || self.position == position::EM_TOP
    }

    /// 後（1）與範例下（6）。
    pub fn in_back(&self) -> bool {
        self.position == position::AFTER || self.position == position::EM_BOTTOM
    }
}

/// 「穩定」（能進凍結 system／共用快照）裡不看可見度與觸發來源的條件：constant、不擲機率、
/// sticky／cooldown／delay 都是 0 或沒設、不在群組、沒有 `delayUntilRecursion`、沒有裝飾、`triggers` 為空，
/// 而且內文與標題只含靜態巨集（`{{char}}` 依說話者而變，也不算）。
pub(crate) fn stable(wi: &WiEntry, title: &str) -> bool {
    let no_timer = |value: Option<f64>| value.is_none_or(|value| value == 0.0 || value.is_nan());
    wi.constant
        && (!wi.use_probability || wi.probability == 100.0)
        && no_timer(wi.sticky)
        && no_timer(wi.cooldown)
        && no_timer(wi.delay)
        && wi.group.is_empty()
        && !wi.delay_until_recursion.truthy()
        && wi.decorators.is_empty()
        && wi.triggers.is_empty()
        && is_static(&wi.content, false)
        && is_static(title, false)
}

/// 注入段落：同深度同角色的條目內容照鍵名排序、各自 trim 後以換行接起（條目內文在 `prepare` 已代換過）。
#[derive(Debug, Clone, PartialEq)]
pub struct Injection<'a> {
    pub text: String,
    pub members: Vec<&'a Placed>,
}

/// 一組條目排好的樣子：前、後兩組（每條各自帶標題送出）與尾段注入段落（依閱讀順序）。
#[derive(Debug, Default)]
pub struct Arranged<'a> {
    pub front: Vec<&'a Placed>,
    pub back: Vec<&'a Placed>,
    pub injected: Vec<Injection<'a>>,
}

struct Prompt<'a> {
    key: String,
    value: String,
    depth: f64,
    /// 0 system、1 user、2 assistant（ST `extension_prompt_roles`；其餘當 system）
    role: u8,
    members: Vec<&'a Placed>,
}

const MAX_INJECTION_DEPTH: f64 = 10_000.0;

fn role_slot(role: f64) -> u8 {
    if role == 1.0 {
        1
    } else if role == 2.0 {
        2
    } else {
        0
    }
}

/// JS 的數字轉字串（鍵名用；深度一律要是整數才會送出，這裡只需要整數與一般小數）。
fn js_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// `entries` 依放置前的排序（掃描給的順序）；內文空的條目不送（同網頁版）。outlet 不在這裡：內容由
/// `{{outlet::名稱}}` 代入卡寫的位置（P5）。
pub fn arrange<'a>(entries: &[&'a Placed]) -> Arranged<'a> {
    let live: Vec<&'a Placed> = entries
        .iter()
        .copied()
        .filter(|entry| !entry.content.is_empty())
        .collect();
    // 網頁版各組是 `insert(0, …)`：組內順序是排序的反向
    let reversed: Vec<&'a Placed> = live.iter().rev().copied().collect();
    let pick = |wanted: f64| -> Vec<&'a Placed> {
        reversed
            .iter()
            .copied()
            .filter(|entry| entry.position == wanted)
            .collect()
    };
    let mut front = pick(position::BEFORE);
    front.extend(pick(position::EM_TOP));
    let mut back = pick(position::AFTER);
    back.extend(pick(position::EM_BOTTOM));

    let mut prompts: Vec<Prompt<'a>> = Vec::new();
    let top = pick(position::AN_TOP);
    let bottom = pick(position::AN_BOTTOM);
    if !top.is_empty() || !bottom.is_empty() {
        let join = |items: &[&Placed]| {
            items
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut value = format!("{}\n\n{}", join(&top), join(&bottom));
        if value.starts_with('\n') {
            value.remove(0);
        }
        if value.ends_with('\n') {
            value.pop();
        }
        prompts.push(Prompt {
            key: "2_floating_prompt".to_owned(),
            value,
            depth: 4.0,
            role: 0,
            members: top.into_iter().chain(bottom).collect(),
        });
    }
    // 依深度：照排序走、同 (深度, 角色) 一組、組內 insert(0)
    let mut groups: Vec<(f64, f64, Vec<&'a Placed>)> = Vec::new();
    for entry in live
        .iter()
        .copied()
        .filter(|entry| entry.position == position::AT_DEPTH)
    {
        match groups
            .iter_mut()
            .find(|(depth, role, _)| *depth == entry.depth && *role == entry.role)
        {
            Some((_, _, members)) => members.insert(0, entry),
            None => groups.push((entry.depth, entry.role, vec![entry])),
        }
    }
    for (depth, role, members) in groups {
        prompts.push(Prompt {
            key: format!("customDepthWI_{}_{}", js_number(depth), js_number(role)),
            value: members
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            depth,
            role: role_slot(role),
            members,
        });
    }
    prompts.retain(|prompt| {
        !prompt.value.is_empty()
            && prompt.depth.fract() == 0.0
            && (0.0..=MAX_INJECTION_DEPTH).contains(&prompt.depth)
    });
    let mut depths: Vec<f64> = Vec::new();
    for prompt in &prompts {
        if !depths.contains(&prompt.depth) {
            depths.push(prompt.depth);
        }
    }
    depths.sort_by(|a, b| b.partial_cmp(a).expect("有限數字"));
    // 攤平：深的在前；同深度閱讀順序 assistant → user → system（injections.ts 插進新到舊再整體 reverse）
    let mut injected = Vec::new();
    for depth in depths {
        for role in [2u8, 1, 0] {
            let mut same: Vec<&Prompt<'a>> = prompts
                .iter()
                .filter(|prompt| prompt.depth == depth && prompt.role == role)
                .collect();
            if same.is_empty() {
                continue;
            }
            same.sort_by(|a, b| a.key.cmp(&b.key));
            injected.push(Injection {
                text: same
                    .iter()
                    .map(|prompt| js_trim(&prompt.value))
                    .collect::<Vec<_>>()
                    .join("\n"),
                members: same
                    .iter()
                    .flat_map(|prompt| prompt.members.iter().copied())
                    .collect(),
            });
        }
    }
    Arranged {
        front,
        back,
        injected,
    }
}
