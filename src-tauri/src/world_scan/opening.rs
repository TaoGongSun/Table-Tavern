//! 開場白的巨集（方案三之 7）：清單顯示用中性求值、不寫變數；落檔時以原卡的原文完整求值，玩家貼的是翻譯版時
//! 正文用翻譯版、副作用照原文。卡欄位巨集看到的是桌上同名角色卡的公開設定（開場白是公開旁白，不讀私設）。
//! 變數副作用比照一般生成走世界書落地日誌（三之 8，`OpeningLanding`）：崩潰後下一次結算辨識得出、照規則撤回並回報。

use super::landing::{fail, land_vars, with_kept};
use super::macros::{card_text, MacroInputs, MacroSession};
use super::Viewer;
use crate::data;
use crate::data::message_vars::{self, TurnKey};
use crate::data::world_info_store::{self as store, Perspective, Report};
use crate::import::OpeningEffects;
use crate::st_macros::substitute::CardText;
use crate::st_macros::variables::VarOp;
use crate::ui_msg::UiMsg;
use std::path::Path;

/// 一張卡的開場白求值環境。
pub struct OpeningMacros {
    session: MacroSession,
    card: CardText,
}

impl OpeningMacros {
    /// `live`＝落檔（系統亂數）；顯示用傳 false 也無妨，中性求值本來就不寫。
    pub fn load(
        root: &Path,
        world_id: &str,
        card_name: &str,
        path: &crate::scene_budget::PathLimits,
        lang: &str,
        live: bool,
    ) -> Self {
        let player = data::read_player_card(root, world_id).ok().flatten();
        let events = data::read_state(root, world_id)
            .ok()
            .and_then(|state| data::read_transcript(root, world_id, state.current_scene).ok())
            .unwrap_or_default();
        let session = MacroSession::new(
            MacroInputs::load(
                root,
                world_id,
                &Perspective::Gm,
                path.model.clone(),
                path.limits,
                live,
            ),
            &Viewer::Gm,
            "",
            Some(card_name),
            player.as_ref(),
            &events,
            lang,
        )
        // 開場白是公開旁白：只讀角色側看得到的事件、不帶 outlet
        .publicized(&events, lang);
        let card = data::list_characters(root, world_id)
            .ok()
            .and_then(|metas| {
                metas
                    .into_iter()
                    .find(|meta| meta.name == card_name && !meta.archived)
            })
            .and_then(|meta| data::read_character(root, world_id, &meta.id).ok())
            .map(|card| card_text(&card, false, session.persona()))
            .unwrap_or_else(|| session.own_card().clone());
        Self { session, card }
    }

    /// 清單顯示：中性求值。
    pub fn display(&self, raw: &str) -> String {
        self.session.neutral(raw, &self.card).text
    }

    /// 落檔：完整求值，回傳正文與這次寫下的操作序列。
    pub fn evaluate(&self, raw: &str) -> (String, Vec<VarOp>) {
        let text = self.session.own_as(raw, &self.card).text;
        (text, self.session.take_ops())
    }
}

/// 玩家挑的那則開場白的原文與原卡名：從那次匯入存下的原檔取（`import_source` 與序號都要有）。
pub fn original_opening(
    root: &Path,
    world_id: &str,
    import_source: Option<&str>,
    index: Option<usize>,
) -> Option<(String, String)> {
    let path = data::import_source_file_path(root, world_id, import_source?).ok()?;
    let bytes = std::fs::read(path).ok()?;
    let (name, openings) = crate::import::card_openings(&bytes)?;
    let raw = openings.into_iter().nth(index?)?;
    Some((name, raw))
}

/// 求值之前：上一回合代落、再結算這一幕的世界書落地。崩潰留下的寫入中落地要先撤回，開場白的 `{{getvar}}`
/// 才不會讀到還沒恢復的值（沒有變數操作的開場白也一樣）。失敗回 `WorldInfoSettleFailed`（前端給重設出路）。
pub fn settle_before_opening(root: &Path, world_id: &str, scene: u64) -> Result<(), String> {
    data::settle_pending_turn(root, world_id).map_err(|error| error.to_string())?;
    store::settle_pending(root, world_id, scene).map_err(|error| error.to_string())
}

/// 落檔用的求值：先結算（`settle_before_opening`），再載入變數與事件、以原文完整求值。
pub fn evaluate_for_posting(
    root: &Path,
    world_id: &str,
    scene: u64,
    card_name: &str,
    raw: &str,
    path: &crate::scene_budget::PathLimits,
    lang: &str,
) -> Result<(String, Vec<VarOp>), String> {
    settle_before_opening(root, world_id, scene)?;
    Ok(OpeningMacros::load(root, world_id, card_name, path, lang, true).evaluate(raw))
}

/// 開場白的變數副作用落地（三之 7、三之 8）：一筆「開場白回合」的落地日誌（GM 視角、計時表照現值、回合鍵
/// `opening:<隨機>`，part `opening`）。逐字稿追加之前 `begin`：結算（呼叫端已結算過就是空操作）、開日誌、變數層照
/// 「意圖→寫層→記結果」寫好（同一般生成的重放與 compare-and-set）；開場白事件帶同一個回合鍵追加；之後 `commit`
/// 標已送出並清掉。追加失敗 `abort` 撤回。崩潰時：追加之前＝結算照四種情形撤回並回報；追加之後＝結算在逐字稿認得出
/// 這個回合鍵，判成功、保留副作用。沒有操作時什麼都不做。
pub struct OpeningLanding<'a> {
    root: &'a Path,
    world_id: &'a str,
    scene: u64,
    turn_id: String,
    ops: Vec<VarOp>,
}

impl<'a> OpeningLanding<'a> {
    pub fn new(root: &'a Path, world_id: &'a str, scene: u64, ops: Vec<VarOp>) -> Self {
        Self {
            root,
            world_id,
            scene,
            turn_id: format!("opening:{}", message_vars::new_token()),
            ops,
        }
    }
}

impl OpeningEffects for OpeningLanding<'_> {
    fn begin(&mut self) -> Result<(), String> {
        if self.ops.is_empty() {
            return Ok(());
        }
        let (root, world, scene, turn) = (self.root, self.world_id, self.scene, &self.turn_id);
        store::settle_pending(root, world, scene).map_err(|error| error.to_string())?;
        let timed = store::read_timed(root, world, scene, &Perspective::Gm).map_err(|error| {
            UiMsg::WorldInfoSettleFailed {
                error: error.to_string(),
            }
            .to_string()
        })?;
        store::begin_landing(root, world, scene, turn, &Perspective::Gm, &timed)
            .map_err(|error| error.to_string())?;
        if let Err(error) = land_vars(root, world, scene, turn, &self.ops) {
            let kept = fail(root, world, scene, turn, Report::Inline);
            return Err(with_kept(error, &kept));
        }
        Ok(())
    }

    fn turn_key(&self) -> Option<TurnKey> {
        (!self.ops.is_empty()).then(|| TurnKey {
            turn_id: self.turn_id.clone(),
            part: message_vars::PART_OPENING.to_owned(),
        })
    }

    fn commit(&mut self) {
        if self.ops.is_empty() {
            return;
        }
        let (root, world, scene, turn) = (self.root, self.world_id, self.scene, &self.turn_id);
        // 開場白已帶回合鍵落檔：清不掉只記 log，下一次結算認得出它、判成功
        let cleared = store::mark_sent(root, world, scene, turn)
            .and_then(|()| store::reply_landed(root, world, scene, turn));
        if let Err(error) = cleared {
            log::warn!("開場白：清落地日誌失敗，留給下一次結算：{error}");
        }
    }

    fn abort(&mut self) {
        if !self.ops.is_empty() {
            // 追加失敗沒有地方附回報：沒還原的層寫進待回報檔
            fail(
                self.root,
                self.world_id,
                self.scene,
                &self.turn_id,
                Report::Notices,
            );
        }
    }
}
