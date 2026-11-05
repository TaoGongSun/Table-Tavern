//! VN 生圖佇列：臉層（plans/vn-mode-pkg7.md §4 第 5 步）與背景（vn-mode-pkg3.md §2）共用一個 worker，
//! 一次一張（同時只用一個來源，agy 並行會 429），背景排在臉層前面。狀態不落檔：臉層由「有定稿但缺哪些」、
//! 背景由「這幕用到但庫裡沒有」推導，重開 app 再補一次就續生；失敗只記在記憶體。
use super::store::SpriteDir;
use crate::data_root;
use crate::vn_backgrounds::Backdrop;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;

pub(crate) const STATUS_EVENT: &str = "vn-sprite-status";
pub(crate) const BACKGROUND_EVENT: &str = "vn-background-status";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Slot {
    world: String,
    character: String,
    /// None＝共用表情組；變體鍵＝該變體的獨立組
    variant: Option<String>,
    expression: String,
}

/// 背景工作的種類：基底（地點還沒有圖才生）、重生成（覆寫那張）、專屬時段圖（以同地點的圖當參考改光線）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BgMode {
    Base,
    Regenerate,
    Dedicated,
}

/// 背景工作：身分＝（桌、slug、時段）；地點文字與種類不算身分。基底鍵的時段固定是清單第一項，所以基底按地點去重
#[derive(Clone, Debug)]
pub(crate) struct BgSlot {
    pub world: String,
    pub backdrop: Backdrop,
    pub mode: BgMode,
}

impl BgSlot {
    pub(crate) fn new(world: &str, backdrop: Backdrop, mode: BgMode) -> Self {
        BgSlot {
            world: world.to_owned(),
            backdrop,
            mode,
        }
    }

    fn id(&self) -> (&str, &str, &str) {
        (&self.world, &self.backdrop.slug, &self.backdrop.period)
    }
}

impl PartialEq for BgSlot {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for BgSlot {}

impl Hash for BgSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Job {
    Face(Slot),
    Background(BgSlot),
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<Job>,
    running: Option<Job>,
    failed: HashMap<Job, String>,
    worker: bool,
}

impl Queue {
    /// 沒在跑、沒在排、沒失敗過
    fn is_new(&self, job: &Job) -> bool {
        self.running.as_ref() != Some(job)
            && !self.waiting.contains(job)
            && !self.failed.contains_key(job)
    }

    /// 背景插在第一個臉層前（背景之間先來後到），臉層排最後
    fn push(&mut self, job: Job) {
        let at = match job {
            Job::Background(_) => self
                .waiting
                .iter()
                .position(|queued| matches!(queued, Job::Face(_)))
                .unwrap_or(self.waiting.len()),
            Job::Face(_) => self.waiting.len(),
        };
        self.waiting.insert(at, job);
    }
}

fn queue() -> std::sync::MutexGuard<'static, Queue> {
    static QUEUE: OnceLock<Mutex<Queue>> = OnceLock::new();
    QUEUE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SlotStatus {
    pub world_id: String,
    pub character_id: String,
    pub variant: Option<String>,
    pub expression: String,
    /// queued／running／done／failed
    pub state: &'static str,
    pub reason: Option<String>,
}

fn status(slot: &Slot, state: &'static str, reason: Option<String>) -> SlotStatus {
    SlotStatus {
        world_id: slot.world.clone(),
        character_id: slot.character.clone(),
        variant: slot.variant.clone(),
        expression: slot.expression.clone(),
        state,
        reason,
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackgroundStatus {
    pub world_id: String,
    pub slug: String,
    pub period: String,
    /// queued／running／done／failed／cancelled
    pub state: &'static str,
    pub reason: Option<String>,
}

fn bg_status(slot: &BgSlot, state: &'static str, reason: Option<String>) -> BackgroundStatus {
    BackgroundStatus {
        world_id: slot.world.clone(),
        slug: slot.backdrop.slug.clone(),
        period: slot.backdrop.period.clone(),
        state,
        reason,
    }
}

fn emit(app: &tauri::AppHandle, job: &Job, state: &'static str, reason: Option<String>) {
    let _ = match job {
        Job::Face(slot) => app.emit(STATUS_EVENT, status(slot, state, reason)),
        Job::Background(slot) => app.emit(BACKGROUND_EVENT, bg_status(slot, state, reason)),
    };
}

/// 一桌的工作：在跑、排隊、失敗（依序）
fn jobs_of<'a>(
    queue: &'a Queue,
    world: &'a str,
) -> impl Iterator<Item = (&'a Job, &'static str, Option<String>)> + 'a {
    let running = queue.running.iter().map(|job| (job, "running", None));
    let waiting = queue.waiting.iter().map(|job| (job, "queued", None));
    let failed = queue
        .failed
        .iter()
        .map(|(job, reason)| (job, "failed", Some(reason.clone())));
    running
        .chain(waiting)
        .chain(failed)
        .filter(move |(job, ..)| match job {
            Job::Face(slot) => slot.world == world,
            Job::Background(slot) => slot.world == world,
        })
}

/// 這桌目前在佇列、生成中、失敗的臉層
pub(crate) fn snapshot(world: &str) -> Vec<SlotStatus> {
    let queue = queue();
    jobs_of(&queue, world)
        .filter_map(|(job, state, reason)| match job {
            Job::Face(slot) => Some(status(slot, state, reason)),
            Job::Background(_) => None,
        })
        .collect()
}

/// 這桌目前在佇列、生成中、失敗的背景
pub(crate) fn background_snapshot(world: &str) -> Vec<BackgroundStatus> {
    let queue = queue();
    jobs_of(&queue, world)
        .filter_map(|(job, state, reason)| match job {
            Job::Background(slot) => Some(bg_status(slot, state, reason)),
            Job::Face(_) => None,
        })
        .collect()
}

/// 排入背景（冪等：已在佇列、生成中、失敗過的不重排）
pub(crate) fn enqueue_backgrounds(app: &tauri::AppHandle, slots: Vec<BgSlot>) {
    let added: Vec<Job> = {
        let mut queue = queue();
        slots
            .into_iter()
            .map(Job::Background)
            .filter(|job| {
                let new = queue.is_new(job);
                if new {
                    queue.push(job.clone());
                }
                new
            })
            .collect()
    };
    for job in &added {
        emit(app, job, "queued", None);
    }
    start_worker(app);
}

/// 重生成一張背景或生專屬時段圖：清掉失敗記錄後排入（同鍵已在排就換成這一版）
pub(crate) fn regenerate_background(app: &tauri::AppHandle, slot: BgSlot) {
    let job = Job::Background(slot);
    {
        let mut queue = queue();
        queue.failed.remove(&job);
        queue.waiting.retain(|queued| *queued != job);
        queue.push(job.clone());
    }
    emit(app, &job, "queued", None);
    start_worker(app);
}

impl Queue {
    /// 取消同鍵（桌、slug、時段）排隊中的背景並清掉失敗記錄；在跑的那張不動
    fn cancel_background(&mut self, world: &str, slug: &str, period: &str) {
        let same =
            |job: &Job| matches!(job, Job::Background(slot) if slot.id() == (world, slug, period));
        self.waiting.retain(|job| !same(job));
        self.failed.retain(|job, _| !same(job));
    }
}

/// 玩家上傳替換一張背景後：排隊中的同鍵重生成不再跑（免得蓋回上傳的圖）
pub(crate) fn cancel_background(world: &str, slug: &str, period: &str) {
    queue().cancel_background(world, slug, period);
}

/// 背景不經佇列換了圖（上傳替換＝done，舞台與一覽重讀）或玩家取消排隊（cancelled）
pub(crate) fn announce_background(app: &tauri::AppHandle, slot: &BgSlot, state: &'static str) {
    emit(app, &Job::Background(slot.clone()), state, None);
}

/// 這桌在跑、排隊、失敗的背景工作帶的地點（自由地點第一次生成時，只有這裡知道名字）
pub(crate) fn background_jobs(world: &str) -> Vec<Backdrop> {
    let queue = queue();
    jobs_of(&queue, world)
        .filter_map(|(job, ..)| match job {
            Job::Background(slot) => Some(slot.backdrop.clone()),
            Job::Face(_) => None,
        })
        .collect()
}

/// 推導並排入這桌缺的臉層（冪等：已在佇列、生成中、失敗過的不重排）
pub(crate) fn fill(app: &tauri::AppHandle, world: &str, characters: &[String]) {
    let Ok(root) = data_root(app) else {
        return;
    };
    // 重播桌進桌也會打：靜默不排（plans/vn-mode-pkg8.md §2.6.1）
    if crate::data::vn::is_replay(&root, world) {
        return;
    }
    let mut added = Vec::new();
    {
        let mut queue = queue();
        for character in characters {
            let Ok(dir) = SpriteDir::of(&root, world, character) else {
                continue;
            };
            let faces = super::faces_of(&root, world, character);
            for (variant, expression) in dir.missing_faces(&faces) {
                let job = Job::Face(Slot {
                    world: world.to_owned(),
                    character: character.clone(),
                    variant,
                    expression,
                });
                if queue.is_new(&job) {
                    queue.push(job.clone());
                    added.push(job);
                }
            }
        }
    }
    for job in &added {
        emit(app, job, "queued", None);
    }
    start_worker(app);
}

/// 換定稿、改框、刪立繪後：清掉這角色的失敗記錄與排隊格（在途那張寫回時會被重驗擋下）
pub(crate) fn forget(world: &str, character: &str) {
    let mut queue = queue();
    let mine = |job: &Job| matches!(job, Job::Face(slot) if slot.world == world && slot.character == character);
    queue.waiting.retain(|job| !mine(job));
    queue.failed.retain(|job, _| !mine(job));
}

/// 玩家重骰一格：清掉失敗記錄，交給下一次 fill 排入
pub(crate) fn forget_slot(world: &str, character: &str, set: Option<&str>, expression: &str) {
    queue().failed.retain(|job, _| {
        !matches!(job, Job::Face(slot) if slot.world == world
            && slot.character == character
            && slot.variant.as_deref() == set
            && slot.expression == expression)
    });
}

fn start_worker(app: &tauri::AppHandle) {
    {
        let mut queue = queue();
        if queue.worker || queue.waiting.is_empty() {
            return;
        }
        queue.worker = true;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let job = {
                let mut queue = queue();
                match queue.waiting.pop_front() {
                    Some(job) => {
                        queue.running = Some(job.clone());
                        job
                    }
                    None => {
                        queue.running = None;
                        queue.worker = false;
                        break;
                    }
                }
            };
            emit(&app, &job, "running", None);
            match &job {
                Job::Face(slot) => run_face(&app, slot).await,
                Job::Background(slot) => {
                    let outcome = crate::vn_backgrounds::job(&app, slot).await;
                    let mut queue = queue();
                    queue.running = None;
                    if let Err(reason) = &outcome {
                        queue.failed.insert(job.clone(), reason.clone());
                    }
                    drop(queue);
                    match outcome {
                        Ok(_) => emit(&app, &job, "done", None),
                        Err(reason) => emit(&app, &job, "failed", Some(reason)),
                    }
                }
            }
        }
    });
}

async fn run_face(app: &tauri::AppHandle, slot: &Slot) {
    let job = Job::Face(slot.clone());
    let outcome = super::face_job(
        app,
        &slot.world,
        &slot.character,
        slot.variant.as_deref(),
        &slot.expression,
    )
    .await;
    let mut requeue = false;
    {
        let mut queue = queue();
        queue.running = None;
        match &outcome {
            Ok(true) => {}
            // 開工後定稿、框或該格被換掉（成功或失敗都一樣）：丟掉結果，缺的格交給下面重新推導
            Ok(false) => requeue = true,
            Err(reason) => {
                queue.failed.insert(job.clone(), reason.clone());
            }
        }
    }
    match outcome {
        Ok(_) => emit(app, &job, "done", None),
        Err(reason) => emit(app, &job, "failed", Some(reason)),
    }
    // 過期任務：角色仍在、這格仍缺才依目前定稿重排（fill 當時因它在跑而略過）
    let still_missing = requeue
        && data_root(app).ok().is_some_and(|root| {
            super::character_exists(&root, &slot.world, &slot.character)
                && SpriteDir::of(&root, &slot.world, &slot.character).is_ok_and(|dir| {
                    let faces = super::faces_of(&root, &slot.world, &slot.character);
                    dir.missing_faces(&faces)
                        .contains(&(slot.variant.clone(), slot.expression.clone()))
                })
        });
    if still_missing {
        let mut queue = queue();
        if !queue.waiting.contains(&job) {
            queue.push(job);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(expression: &str) -> Job {
        Job::Face(Slot {
            world: "w".to_owned(),
            character: "c".to_owned(),
            variant: None,
            expression: expression.to_owned(),
        })
    }

    fn bg(location: &str, force: bool) -> Job {
        let mode = match force {
            true => BgMode::Regenerate,
            false => BgMode::Base,
        };
        Job::Background(BgSlot::new(
            "w",
            Backdrop::new(location, None, None, &Default::default()),
            mode,
        ))
    }

    #[test]
    fn backgrounds_jump_ahead_of_faces_in_arrival_order() {
        let mut queue = Queue::default();
        queue.push(face("happy"));
        queue.push(bg("港口", false));
        queue.push(face("sad"));
        queue.push(bg("教室", false));
        let order: Vec<Job> = queue.waiting.iter().cloned().collect();
        assert_eq!(
            order,
            vec![
                bg("港口", false),
                bg("教室", false),
                face("happy"),
                face("sad")
            ]
        );
    }

    /// 上傳替換：同鍵排隊中的重生成與失敗記錄清掉，別的鍵與臉層不動
    #[test]
    fn upload_cancels_queued_background_of_the_same_key() {
        let mut queue = Queue::default();
        queue.push(bg("港口", true));
        queue.push(bg("教室", true));
        queue.push(face("happy"));
        queue.failed.insert(bg("港口", false), "x".to_owned());
        let Job::Background(harbor) = bg("港口", false) else {
            unreachable!()
        };
        let (slug, period) = (harbor.backdrop.slug.clone(), harbor.backdrop.period.clone());
        queue.cancel_background("w", &slug, &period);
        assert!(queue.is_new(&bg("港口", true)));
        assert_eq!(queue.waiting.len(), 2);
        assert!(!queue.is_new(&bg("教室", false)));
    }

    #[test]
    fn background_identity_ignores_spelling_and_force() {
        let mut queue = Queue::default();
        queue.push(bg("港口", false));
        assert!(!queue.is_new(&bg(" 港口 ", true)));
        assert!(queue.is_new(&bg("碼頭", false)));
        queue.failed.insert(bg("碼頭", false), "x".to_owned());
        assert!(!queue.is_new(&bg("碼頭", false)));
    }
}
