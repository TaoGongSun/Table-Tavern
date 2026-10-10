//! 兩條匯入路徑的本體：command 與重新重構的重匯（refactor/reset.rs）共用同一條路，重匯出來的桌才會
//! 跟當初匯入的一模一樣。
//!
//! 匯入與貼開場白都要出示整桌獨占（`_held`）：快照、資料變更、記帳與失敗復原之間不能插進別的寫入，
//! 否則 diff 會把別人的變更記進收據、復原會蓋掉別人剛寫的內容。
//!
//! 來源紀錄的順序（receipts/sources.rs）：先驗卡檔（壞檔不留任何東西）→ 寫自己的「未完成」標記（寫不進去
//! 整次不做）→ 另存原檔 → 匯入 → 記收據（原檔識別掛在收據上）→ 都成功才刪標記。中途任何一步失敗，標記
//! 留著，這桌的來源就判不完整、重新重構擋下；只有「原檔沒存成」這種什麼都還沒動的失敗會收掉標記。
use crate::data::{self, CharacterMeta, DataResult, TranscriptEvent, WorldbookImport};
use crate::receipts::{self, ImportRoute, ImportSource, Recorded};
use crate::{mechanism, transport};
use std::path::Path;

/// 匯入結果與這次匯入的原檔識別：前端貼開場白時帶回來，開場白才掛得到正確那筆匯入。
/// 沒留收據（什麼都沒新增）或記帳失敗時是 None。
/// `image_dropped`：卡圖（角色圖或 GM 圖）救不回、沒存成，前端要提示。
/// `book`／`book_failed`：角色卡路的隨身世界書收編數字／卡帶了書卻沒匯成（世界書路的數字在 `value`）。
pub struct Imported<T> {
    pub value: T,
    pub source: Option<String>,
    pub image_dropped: bool,
    pub book: Option<WorldbookImport>,
    pub book_failed: bool,
}

/// 寫標記並存原檔；原檔存不進去時什麼都還沒動，標記收掉再回錯。
fn begin(root: &Path, world_id: &str, bytes: &[u8]) -> DataResult<(String, String)> {
    let pending = receipts::begin_pending(root, world_id)?;
    match receipts::store_import_source(root, world_id, bytes) {
        Ok(file) => Ok((pending, file)),
        Err(error) => {
            receipts::finish_pending(root, world_id, &pending);
            Err(error)
        }
    }
}

/// 記帳完成：記到了（或什麼都沒新增）才收掉標記；記帳失敗標記留著。
fn finish(
    root: &Path,
    world_id: &str,
    pending: &str,
    file: String,
    recorded: Recorded,
) -> Option<String> {
    match recorded {
        Recorded::Yes => {
            receipts::finish_pending(root, world_id, pending);
            Some(file)
        }
        Recorded::NothingNew => {
            receipts::finish_pending(root, world_id, pending);
            None
        }
        Recorded::Failed => None,
    }
}

/// 世界書路徑：剝 character_book／人設欄轉條目、原卡介面檔、GM 圖、機制、卡擴充欄位，記收據。
pub fn import_worldbook_file(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    label: &str,
    _held: &data::WorldExclusive,
) -> DataResult<Imported<WorldbookImport>> {
    let json_text = super::worldbook_json(bytes)?;
    let (pending, file) = begin(root, world_id, bytes)?;
    let before = receipts::snapshot(root, world_id);
    // 匯入本身失敗：可能已寫了一半，標記留著（來源判不完整）。世界書路的卡由 GM 演：沒寫可見度的條目給 GM
    let book = data::import_worldbook_as(root, world_id, &json_text, &data::BookOwner::Gm)?;
    super::save_world_card(root, world_id, bytes);
    // 寫檔失敗照「匯入本身失敗」處理：可能已寫了一半，標記留著
    let gm_image = super::save_gm_image(root, world_id, bytes)?;
    if let Ok(book) = serde_json::from_str(&json_text) {
        super::import_mechanism(root, world_id, &book);
    }
    super::import_card_extension(root, world_id, label, bytes);
    let recorded = receipts::record_worldbook_import(
        root,
        world_id,
        label,
        before,
        book.restores,
        Some(ImportSource {
            route: ImportRoute::Worldbook,
            label: label.to_owned(),
            color: String::new(),
            file: file.clone(),
        }),
    );
    Ok(Imported {
        value: book.summary,
        source: finish(root, world_id, &pending, file, recorded),
        image_dropped: gm_image == super::GmImage::Dropped,
        book: None,
        book_failed: false,
    })
}

/// 角色卡路徑：建角色卡（含卡片隨身世界書與機制），記收據。
pub fn import_character_file(
    root: &Path,
    world_id: &str,
    bytes: &[u8],
    color: &str,
    lang: &str,
    _held: &data::WorldExclusive,
) -> DataResult<Imported<CharacterMeta>> {
    super::check_character_bytes(bytes)?;
    let (pending, file) = begin(root, world_id, bytes)?;
    let before = receipts::snapshot(root, world_id);
    // 卡檔已驗過；這裡失敗是寫檔錯，可能已寫了一半，標記留著（來源判不完整）
    let super::ImportedCharacter {
        meta,
        image_dropped,
        book,
        book_failed,
    } = super::import_character_reporting(root, world_id, bytes, color, lang)?;
    let (summary, restores) = match book {
        Some(book) => (Some(book.summary), book.restores),
        None => (None, Vec::new()),
    };
    let recorded = receipts::record_character_import(
        root,
        world_id,
        &meta.id,
        &meta.name,
        before,
        restores,
        Some(ImportSource {
            route: ImportRoute::Character,
            label: String::new(),
            color: color.to_owned(),
            file: file.clone(),
        }),
    );
    Ok(Imported {
        value: meta,
        source: finish(root, world_id, &pending, file, recorded),
        image_dropped,
        book: summary,
        book_failed,
    })
}

/// 貼開場白＝GM 旁白，但狀態區塊要走與 GM 回覆同一條解析：剝除、併進檯面、事件帶快照一次做完，
/// 並掛到它所屬那筆匯入的收據上（`import`＝那次匯入的原檔識別；含玩家挑的序號，復原匯入時這則開場白
/// 跟著收掉）。序號記不下來或歸屬不了時「未完成」標記留著。post_opening 與重新重構的重貼共用。
#[allow(clippy::too_many_arguments)]
pub fn post_opening_text(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    text: &str,
    lang: &str,
    index: Option<usize>,
    import: Option<&str>,
    held: &data::WorldExclusive,
) -> DataResult<TranscriptEvent> {
    post_opening_text_with(
        root,
        world_id,
        scene,
        ts,
        text,
        lang,
        index,
        import,
        held,
        &mut NoOpeningEffects,
    )
}

/// 開場白巨集的變數副作用（`world_scan::opening::OpeningLanding`）：追加之前開落地日誌並寫好變數（失敗時自己撤回、
/// 沒還原的層併進錯誤），開場白事件帶 `turn_key` 追加，之後 `commit` 清掉日誌；追加失敗 `abort` 撤回。
pub trait OpeningEffects {
    fn begin(&mut self) -> Result<(), String>;
    fn turn_key(&self) -> Option<data::message_vars::TurnKey>;
    fn commit(&mut self);
    fn abort(&mut self);
}

/// 沒有副作用（取不到原卡、重貼）。
pub struct NoOpeningEffects;

impl OpeningEffects for NoOpeningEffects {
    fn begin(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn turn_key(&self) -> Option<data::message_vars::TurnKey> {
        None
    }

    fn commit(&mut self) {}

    fn abort(&mut self) {}
}

/// `post_opening_text`，帶開場白巨集的變數副作用：上一回合代落之後開落地日誌、寫好變數，開場白帶回合鍵追加
/// （之後崩潰，結算從逐字稿認得出它已落檔），再清掉日誌；追加失敗就撤回變數。
#[allow(clippy::too_many_arguments)]
pub fn post_opening_text_with(
    root: &Path,
    world_id: &str,
    scene: u64,
    ts: &str,
    text: &str,
    lang: &str,
    index: Option<usize>,
    import: Option<&str>,
    held: &data::WorldExclusive,
    effects: &mut dyn OpeningEffects,
) -> DataResult<TranscriptEvent> {
    let pending = receipts::begin_pending(root, world_id)?;
    let block = transport::extract_state_block(text);
    let player_name = data::read_player_card(root, world_id)
        .ok()
        .flatten()
        .map(|card| card.name);
    let user_name = player_name
        .as_deref()
        .unwrap_or_else(|| transport::player_fallback_name(lang));
    // 上一回合沒落成的回覆先代落，再存檢查點：貼失敗回復時不會連代落的回覆一起倒掉，舊回覆也不會之後才
    // 插到開場後面
    data::settle_pending_turn(root, world_id)?;
    // 落地日誌在上一回合代落之後才開（之前結算會把「GM 已提交、正文還沒落檔」的回合誤判成失敗）；
    // 變數在追加之前寫好，崩潰在追加之前就整筆撤回、開場白也不在
    if let Err(error) = effects.begin() {
        receipts::finish_pending(root, world_id, &pending);
        return Err(crate::data::invalid_data(error));
    }
    let checkpoint = data::opening_checkpoint(root, world_id, scene);
    let appended = data::append_opening_keyed(
        root,
        world_id,
        scene,
        ts,
        text,
        &block,
        user_name,
        effects.turn_key(),
    );
    let (event, outcome) = match appended {
        Ok(posted) => posted,
        Err(error) => {
            // 逐字稿是直接 append，失敗時可能已留下半行：寫回並確認逐字稿與狀態都回到貼之前，
            // 才算什麼都沒貼上、解除標記並撤回變數；回不去（或貼前就讀不到）標記留著，來源判不完整，
            // 落地日誌也留給下一次結算看逐字稿裡有沒有這則開場白
            if checkpoint.is_ok_and(|checkpoint| checkpoint.restore()) {
                effects.abort();
                receipts::finish_pending(root, world_id, &pending);
            }
            return Err(error);
        }
    };
    effects.commit();
    mechanism::append_log(root, world_id, scene, &outcome.records);
    if receipts::record_posted_opening(root, world_id, scene, ts, index, import, held) {
        receipts::finish_pending(root, world_id, &pending);
    }
    Ok(event)
}
