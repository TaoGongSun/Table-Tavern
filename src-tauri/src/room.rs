//! 房間模式（multiplayer-room 包 2）：桌面版當 GM 執行端，規格見 `.ai/plans/multiplayer-room-protocol.md` §6、§7、§14。
//! 執行端迴圈整個在 Rust 的 tokio 任務裡（人不在電腦前，視窗縮小或鎖定時 webview 可能被節流，不賭它醒著）：
//! `link` 管連線、心跳、重連退避與重送佇列；`run` 管派工、GM 鏈接力、換幕工作與遠端控制；`seed` 產公開名冊、
//! 公開歷史與 capabilities；`awake` 防休眠。GM 引擎本身不動，經 `Engine` 呼叫（測試換成替身）。
mod awake;
mod link;
mod member;
mod run;
mod seed;

pub(crate) use member::{member_close, member_open, member_send};

use crate::data;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

pub(crate) type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
/// 串流增量的去處（每段一支）
pub(crate) type Delta = Arc<dyn Fn(&str) + Send + Sync>;

/// `gm_narrate` 回傳的執行端投影
pub(crate) struct Narration {
    pub(crate) text: String,
    pub(crate) raw: Option<String>,
    pub(crate) next: Option<String>,
    pub(crate) next_player: Option<String>,
    /// 長文字欄變動紀錄本文；空＝這輪沒有
    pub(crate) state_updates: String,
    pub(crate) arrived: Vec<String>,
    pub(crate) aborted: bool,
}

/// `chat_with_character` 回傳的執行端投影
pub(crate) struct Spoken {
    pub(crate) text: String,
    pub(crate) raw: Option<String>,
    pub(crate) aborted: bool,
}

/// 執行端對桌面版既有引擎的需要：模型呼叫、換幕摘要、中止、幕設定覆寫、通知前端。
pub(crate) trait Engine: Send + Sync + 'static {
    /// (設定目錄, 資料目錄)
    fn roots(&self) -> (PathBuf, PathBuf);
    fn narrate(
        &self,
        world: String,
        turn: String,
        action: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Narration, String>>;
    fn reply(
        &self,
        world: String,
        card: String,
        turn: String,
        action: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Spoken, String>>;
    /// 換幕（含摘要），回新幕號
    fn advance(&self, world: String, turn: String) -> BoxFut<'_, Result<u64, String>>;
    fn abort(&self, world: &str, turn: &str);
    /// 幕設定覆寫（路線, 模型）；`None`＝還原全域設定
    fn use_settings(&self, settings: Option<(String, String)>);
    /// 協定 `capabilities` 內容（照目前設定與覆寫產生）
    fn capabilities(&self) -> Value;
    fn table_changed(&self, world: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RunState {
    Running,
    Done,
    Failed,
}

/// GM 鏈的下一步（本機持久：程序重啟或 retry 從這裡續跑）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Step {
    Land,
    Narrate,
    Reply(String),
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DispatchRun {
    pub(crate) id: String,
    pub(crate) state: RunState,
    pub(crate) last_ended: Option<u32>,
    /// 本回合已跑幾次 GM 旁白（上限 `max_round_speakers`）
    pub(crate) used: u32,
    pub(crate) step: Step,
    pub(crate) arrived: Vec<String>,
    pub(crate) dispatch: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JobRun {
    pub(crate) id: String,
    pub(crate) state: RunState,
    /// 開工時桌上的幕號：之後桌上幕號比它大＝摘要已提交
    pub(crate) scene_before: u64,
}

/// 本機持久（設定目錄 `rooms/<桌 id>.json`，一桌一份、只限本人可讀）：憑證、房間覆寫、重送佇列、最後一筆派工與工作、
/// 桌面成員身分。關房／關程式都只斷線，這份留著（「重開房間」接回）；房間不存在或憑證失效才清掉憑證，檔案本身留著
/// （開過房的桌不被空桌回收）；「結束並刪除房間」才刪檔。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Saved {
    pub(crate) active: bool,
    pub(crate) server: String,
    pub(crate) world_id: String,
    pub(crate) room_code: String,
    pub(crate) join_secret: String,
    pub(crate) executor_token: String,
    pub(crate) invite_url: String,
    pub(crate) seeded: bool,
    pub(crate) settings: Option<(String, String)>,
    pub(crate) next_id: u64,
    /// 已送出但還沒收到 ok 的 gen.part.*／gen.done／gen.failed／job.result（含 id），重連後依序重送
    pub(crate) queue: Vec<Value>,
    pub(crate) dispatch: Option<DispatchRun>,
    pub(crate) job: Option<JobRun>,
    pub(crate) cards_made: Vec<String>,
    /// 桌面版自己的成員連線用的裝置碼（協定 §1.2 格式，跟這間房走）
    pub(crate) device_id: String,
    /// 房主交給了手機：桌面成員連線不再自動拿 grantHost 碼
    pub(crate) phone_host: bool,
    /// 伺服器回房間不在了（憑證已清）；下次開房建新房時歸零
    pub(crate) gone: bool,
}

/// 面板讀的狀態
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub(crate) active: bool,
    pub(crate) world_id: Option<String>,
    pub(crate) server: String,
    pub(crate) room_code: Option<String>,
    pub(crate) invite_url: Option<String>,
    pub(crate) pair_code: Option<String>,
    pub(crate) payer_link: Option<String>,
    pub(crate) online: bool,
    pub(crate) keep_awake: Option<bool>,
    pub(crate) model_label: Option<String>,
    pub(crate) last_error: Option<String>,
    /// 伺服器上的房間已被清（閒置過期或被結束）：重開會建新房、房間碼換新
    pub(crate) gone: bool,
}

/// 在程序裡跑著的那些（不落地）
#[derive(Default)]
pub(crate) struct Work {
    pub(crate) dispatch: Option<String>,
    pub(crate) job: Option<String>,
    pub(crate) turn: Option<String>,
    pub(crate) job_turn: Option<String>,
    pub(crate) abort: bool,
    /// 伺服器手上的名冊有哪些卡（段落說話者必須在名冊裡，否則整條連線被關）
    pub(crate) roster: Vec<String>,
}

/// 計時單位：心跳 15、退避 1／2／5／10／30、換幕進度 240 個單位。正式 1 秒，測試縮短。
#[derive(Clone, Copy)]
pub(crate) struct Timing {
    pub(crate) unit: Duration,
}

impl Timing {
    pub(crate) const REAL: Timing = Timing {
        unit: Duration::from_secs(1),
    };
}

pub(crate) struct Room {
    pub(crate) engine: Arc<dyn Engine>,
    path: PathBuf,
    saved: Mutex<Saved>,
    status: Mutex<Status>,
    work: Mutex<Work>,
    out: mpsc::UnboundedSender<Value>,
    pub(crate) timing: Timing,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// 舊版單一憑證檔：升級後作廢〔作者裁決 2026-11-03〕
const LEGACY_FILE: &str = "room.json";
const DIR: &str = "rooms";

fn saved_path(config_root: &Path, world: &str) -> Result<PathBuf, String> {
    data::validate_id(world).map_err(|error| error.to_string())?;
    Ok(config_root.join(DIR).join(format!("{world}.json")))
}
/// 官方房間伺服器：還沒開過房時面板預設填這個（自架者可改）
const OFFICIAL_SERVER: &str = "https://tt-room.pirahus.com";

/// 寫 room.json：暫存檔＋改名，unix 0600（憑證只限本人可讀）
fn write_saved(path: &Path, saved: &Saved) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(saved).map_err(|error| error.to_string())?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    if let Some(dir) = path.parent() {
        // world-write-exempt: 建立設定目錄（房間憑證檔），不是桌目錄
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    // world-write-exempt: 房間憑證暫存檔在設定目錄，不是桌目錄
    let mut file = options.open(&tmp).map_err(|error| error.to_string())?;
    // world-write-exempt: 寫房間憑證暫存檔，不是桌目錄
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    // world-write-exempt: 房間憑證暫存檔改名成 room.json，不是桌目錄
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

pub(crate) fn read_saved(config_root: &Path, world: &str) -> Saved {
    saved_path(config_root, world)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// 開過房的桌（憑證檔在）：空桌回收不收、桌卡顯示「重開房間」
pub(crate) fn has_room(config_root: &Path, world: &str) -> bool {
    saved_path(config_root, world).is_ok_and(|path| path.exists())
}

/// 開過房的桌與它們的房間碼（大廳桌卡用）
pub(crate) fn room_tables(config_root: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(config_root.join(DIR)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let world = name.strip_suffix(".json")?.to_owned();
            data::validate_id(&world).ok()?;
            let saved = read_saved(config_root, &world);
            Some((world, saved.room_code))
        })
        .collect()
}

impl Room {
    pub(crate) fn saved(&self) -> Saved {
        lock(&self.saved).clone()
    }

    /// 改本機持久狀態並落地。落地失敗只記最近錯誤：連線照跑，下一次改動再寫
    pub(crate) fn update<R>(&self, change: impl FnOnce(&mut Saved) -> R) -> R {
        let mut saved = lock(&self.saved);
        let result = change(&mut saved);
        if let Err(error) = write_saved(&self.path, &saved) {
            drop(saved);
            self.note_error(&format!("rooms/*.json：{error}"));
        }
        result
    }

    /// 同 `update`，但落地失敗就整個還原（記憶體也不留改動）並回錯：改動要先存進磁碟才能對外承認的時候用
    pub(crate) fn try_update(&self, change: impl FnOnce(&mut Saved)) -> Result<(), String> {
        let mut saved = lock(&self.saved);
        let before = saved.clone();
        change(&mut saved);
        if let Err(error) = write_saved(&self.path, &saved) {
            *saved = before;
            drop(saved);
            let message = format!("rooms/*.json：{error}");
            self.note_error(&message);
            return Err(message);
        }
        Ok(())
    }

    pub(crate) fn work(&self) -> MutexGuard<'_, Work> {
        lock(&self.work)
    }

    pub(crate) fn status(&self) -> MutexGuard<'_, Status> {
        lock(&self.status)
    }

    pub(crate) fn world(&self) -> String {
        lock(&self.saved).world_id.clone()
    }

    /// 即時訊框：連線中就送，斷線時丟掉
    pub(crate) fn send(&self, t: &str, p: Value) {
        let _ = self.out.send(json!({ "t": t, "p": p }));
    }

    /// 要等 ok 的訊框：配 id、排進本機佇列，與 `change` 同一次落地，之後才送出（重連後依序重送）
    pub(crate) fn durable(&self, t: &str, p: Value, change: impl FnOnce(&mut Saved)) {
        let frame = self.update(|saved| {
            saved.next_id += 1;
            let frame = json!({ "t": t, "id": format!("q{}", saved.next_id), "p": p });
            saved.queue.push(frame.clone());
            change(saved);
            frame
        });
        let _ = self.out.send(frame);
    }

    /// 房間覆寫值放進覆寫槽；已退出房間模式（還在收尾的任務）就不放，免得漏進單人桌
    pub(crate) fn use_settings(&self) {
        let saved = self.saved();
        if saved.active {
            self.engine.use_settings(saved.settings);
        }
    }

    pub(crate) fn note_error(&self, message: &str) {
        log::warn!("房間模式：{message}");
        self.status().last_error = Some(message.chars().take(300).collect());
    }

    /// 協定 §6.5 `InflightReport`
    fn inflight(&self) -> Value {
        let saved = lock(&self.saved);
        json!({
            "dispatch": saved.dispatch.as_ref().map(|run| json!({
                "dispatchId": run.id, "state": run.state, "lastEndedPart": run.last_ended,
            })),
            "job": saved.job.as_ref().map(|job| json!({
                "jobId": job.id,
                "state": if job.state == RunState::Running { "running" } else { "done" },
            })),
        })
    }

    fn panel(&self) -> Status {
        let saved = lock(&self.saved);
        let mut status = self.status().clone();
        status.active = true;
        status.world_id = Some(saved.world_id.clone());
        status.server = saved.server.clone();
        status.room_code = Some(saved.room_code.clone());
        status.invite_url = Some(saved.invite_url.clone());
        status
    }
}

/// 退出房間模式的桌面收尾（按退出、或房間關了／憑證失效）：清覆寫槽、桌子還原成單人桌、通知前端重讀
pub(crate) fn release_table(room: &Room, world: &str) -> Result<(), String> {
    room.engine.use_settings(None);
    if world.is_empty() {
        return Ok(());
    }
    let _permit = data::world_write_permit(world)?;
    data::clear_party(&room.engine.roots().1, world).map_err(|error| error.to_string())?;
    room.engine.table_changed(world);
    Ok(())
}

pub(crate) struct Running {
    pub(crate) room: Arc<Room>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    _awake: awake::Guard,
}

impl Running {
    /// 斷線：停掉連線迴圈、放開防休眠（Guard 跟著丟）。憑證留在本機
    pub(crate) async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(5), self.task).await;
    }
}

/// 起一個執行端（不登記成全域，測試直接用）。`keep_awake` 為假時不碰系統睡眠
pub(crate) async fn start(
    engine: Arc<dyn Engine>,
    saved: Saved,
    timing: Timing,
    keep_awake: bool,
) -> Running {
    let (out, rx) = mpsc::unbounded_channel();
    let (awake, held) = if keep_awake {
        awake::hold().await
    } else {
        (awake::Guard::default(), None)
    };
    engine.use_settings(saved.settings.clone());
    let path = saved_path(&engine.roots().0, &saved.world_id).unwrap_or_default();
    let room = Arc::new(Room {
        engine,
        path,
        saved: Mutex::new(saved),
        status: Mutex::new(Status {
            keep_awake: held,
            ..Status::default()
        }),
        work: Mutex::new(Work::default()),
        out,
        timing,
    });
    let (stop, stop_rx) = watch::channel(false);
    let task = tokio::spawn(link::run(room.clone(), rx, stop_rx));
    Running {
        room,
        stop,
        task,
        _awake: awake,
    }
}

// ── 全域：桌面版同時只有一個房間 ──

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// 連線迴圈自己停下的原因（哪張桌、哪個錯誤）：停了之後面板照樣看得到為什麼停（例：伺服器版本太舊）
static LAST_STOP: Mutex<std::collections::BTreeMap<String, String>> =
    Mutex::new(std::collections::BTreeMap::new());

pub(crate) fn note_stop(world: &str, message: &str) {
    lock(&LAST_STOP).insert(world.to_owned(), message.to_owned());
}

fn stop_reason(world: &str) -> Option<String> {
    lock(&LAST_STOP).get(world).cloned()
}

/// 前端認得的錯誤代碼：另一張桌正在房間模式
const OTHER_TABLE: &str = "ROOM_OTHER_TABLE";

/// 全域那一份；連線迴圈已自己結束（房間關了、憑證失效、被取代）就移出，防休眠跟著放掉
fn running() -> MutexGuard<'static, Option<Running>> {
    let mut slot = lock(&RUNNING);
    if slot
        .as_ref()
        .is_some_and(|running| running.task.is_finished())
    {
        slot.take();
    }
    slot
}

/// 這時在房間模式的桌（桌面的輸入框與 GM／角色鈕據此拒收）
pub(crate) fn active_world() -> Option<String> {
    running().as_ref().map(|running| running.room.world())
}

/// 這桌在房間模式就拒收（`commands` 的玩家句、GM 旁白、角色接話；房間執行端自己走 domain 不經這裡）
pub(crate) fn refuse_if_active(world_id: &str) -> Result<(), String> {
    match active_world().as_deref() == Some(world_id) {
        true => Err(crate::ui_msg::UiMsg::RoomModeActive.into()),
        false => Ok(()),
    }
}

/// 房間在跑就回它的面板狀態；否則回 `world` 那張桌留著的憑證摘要（`gone`＝伺服器上的房間已被清）
pub(crate) fn status(config_root: &Path, world: Option<&str>) -> Status {
    if let Some(running) = running().as_ref() {
        return running.room.panel();
    }
    let saved = world
        .map(|world| read_saved(config_root, world))
        .unwrap_or_default();
    let known = |value: String| (!value.is_empty()).then_some(value);
    let last_error = world.and_then(stop_reason);
    Status {
        world_id: world.map(str::to_owned),
        last_error,
        room_code: known(saved.room_code),
        server: known(saved.server).unwrap_or_else(|| OFFICIAL_SERVER.to_owned()),
        gone: saved.gone,
        ..Status::default()
    }
}

/// 伺服器網址的主機屬於本機（`private` 時連私網也算）：本機不走代理；明文 http 只放行這些
fn host_is_local(server: &str, private: bool) -> bool {
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
    match host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            ip.is_loopback() || (private && (ip.is_private() || ip.is_link_local()))
        }
        Ok(std::net::IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => host == "localhost",
    }
}

pub(crate) fn local_server(server: &str) -> bool {
    host_is_local(server, false)
}

/// token 與加入密鑰不走明文：`http://` 只放行本機與私網（自架、開發），其餘必須 `https://`
fn server_allowed(server: &str) -> bool {
    server.starts_with("https://") || (server.starts_with("http://") && host_is_local(server, true))
}

/// 開房／重開房間：這張桌有憑證（同一伺服器）就接回原房間，否則（沒開過、伺服器上已清掉）建新房
pub(crate) async fn enter(
    engine: Arc<dyn Engine>,
    world_id: String,
    server: String,
) -> Result<Status, String> {
    if let Some(running) = running().as_ref() {
        return match running.room.world() == world_id {
            true => Ok(running.room.panel()),
            false => Err(OTHER_TABLE.to_owned()),
        };
    }
    lock(&LAST_STOP).remove(&world_id);
    let server = server.trim().trim_end_matches('/').to_owned();
    if !server_allowed(&server) {
        return Err("ROOM_SERVER_INVALID".to_owned());
    }
    let config_root = engine.roots().0;
    let path = saved_path(&config_root, &world_id)?;
    let mut saved = read_saved(&config_root, &world_id);
    if saved.server != server || saved.executor_token.is_empty() {
        saved = link::create_room(&server).await?;
        saved.world_id = world_id;
    }
    saved.active = true;
    write_saved(&path, &saved)?;
    let running = start(engine, saved, Timing::REAL, true).await;
    let status = running.room.panel();
    *lock(&RUNNING) = Some(running);
    Ok(status)
}

/// 退出房間模式：中止進行中的生成、斷線、解除防休眠、清覆寫槽、桌子還原成單人桌。憑證保留
pub(crate) async fn exit() -> Result<(), String> {
    let Some(running) = running().take() else {
        data::set_play_override(None);
        return Ok(());
    };
    let room = running.room.clone();
    room.update(|saved| saved.active = false);
    // 手機看到「房主暫時關房」而不是「GM 主機離線」；送不到（斷線中）就只剩離線
    room.send(
        "status",
        json!({ "keepAwake": null, "busy": false, "paused": true }),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    run::abort(&room);
    let job_turn = room.work().job_turn.clone();
    if let Some(turn) = job_turn {
        room.engine.abort(&room.world(), &turn);
    }
    running.stop().await;
    release_table(&room, &room.world())
}

/// 發一張新的 grantHost 配對碼；結果在下次讀狀態時出現。`phone`：這張碼是要交給手機的（之後桌面成員連線不再自己拿房主），
/// 否則是桌面自己要拿回房主
pub(crate) fn request_pair_code(phone: bool) -> Result<(), String> {
    let slot = running();
    let running = slot.as_ref().ok_or("ROOM_NOT_ACTIVE")?;
    running.room.update(|saved| saved.phone_host = phone);
    link::request_pair(&running.room, true);
    Ok(())
}

/// 房主在房間畫面改了卡（匯入、編輯、刪除、改舞台卡）：照桌上現況重送公開名冊（協定 `roster.update`）。
/// 還沒送過種子就不送（種子本身就是最新名冊）
pub(crate) fn refresh_roster() -> Result<(), String> {
    let slot = running();
    let room = &slot.as_ref().ok_or("ROOM_NOT_ACTIVE")?.room;
    if !room.saved().seeded {
        return Ok(());
    }
    let cards = seed::roster(&room.engine.roots().1, &room.world())?;
    room.work().roster = seed::card_ids(&cards);
    room.send("roster.update", json!({ "cards": cards }));
    let device = device_id(room)?;
    if let Some(reserve) = seed::reserve_frame(&room.engine.roots().1, &room.world(), &device) {
        room.send("seat.reserve", reserve["p"].clone());
    }
    Ok(())
}

/// 桌面成員連線的身分（協定 §1.2）：裝置碼第一次用時產生、跟這間房存在一起
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Identity {
    device_id: String,
    invite_url: String,
    phone_host: bool,
}

/// 電腦這台的裝置碼：第一次用時產生（執行端保留玩家卡、成員連線都要它）
pub(crate) fn device_id(room: &Room) -> Result<String, String> {
    room.update(|saved| {
        if saved.device_id.len() != 22 {
            use base64::Engine as _;
            let mut bytes = [0u8; 16];
            if getrandom::fill(&mut bytes).is_err() {
                return Err("ROOM_DEVICE_ID".to_owned());
            }
            saved.device_id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        }
        Ok(saved.device_id.clone())
    })
}

pub(crate) fn identity() -> Result<Identity, String> {
    let slot = running();
    let running = slot.as_ref().ok_or("ROOM_NOT_ACTIVE")?;
    device_id(&running.room)?;
    let saved = running.room.saved();
    Ok(Identity {
        device_id: saved.device_id,
        invite_url: saved.invite_url,
        phone_host: saved.phone_host,
    })
}

/// 桌卡「結束並刪除房間」：伺服器上結束（協定 `room.close`，執行端憑證），再刪這張桌的憑證檔。桌子不動。
/// 伺服器那邊已經不在也照刪本機；`force`（刪桌時）連伺服器打不到也照刪本機
pub(crate) async fn close(
    engine: Arc<dyn Engine>,
    world: String,
    force: bool,
) -> Result<(), String> {
    if active_world().as_deref() == Some(world.as_str()) {
        exit().await?;
    }
    let config_root = engine.roots().0;
    let path = saved_path(&config_root, &world)?;
    let saved = read_saved(&config_root, &world);
    if !saved.executor_token.is_empty() {
        if let Err(error) = link::close_room(&saved).await {
            if !force {
                return Err(error);
            }
            log::warn!("刪桌時結束房間失敗（本機照清）：{error}");
        }
    }
    // world-write-exempt: 刪房間憑證檔（設定目錄），不是桌目錄
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
        _ => Ok(()),
    }
}

/// 開程式時：舊版單一憑證檔作廢（那張桌還原成單人桌、檔案刪掉）；上次沒有關房的那間自動接回（協定 §7.4）
pub(crate) async fn resume(engine: Arc<dyn Engine>) {
    let (config_root, data_root) = engine.roots();
    let legacy = config_root.join(LEGACY_FILE);
    if let Ok(bytes) = std::fs::read(&legacy) {
        let old: Saved = serde_json::from_slice(&bytes).unwrap_or_default();
        if data::validate_id(&old.world_id).is_ok() {
            if let Ok(_permit) = data::world_write_permit(&old.world_id) {
                let _ = data::clear_party(&data_root, &old.world_id);
            }
        }
        data::set_play_override(None);
        // world-write-exempt: 刪舊版房間憑證檔（設定目錄），不是桌目錄
        let _ = std::fs::remove_file(&legacy);
    }
    if running().is_some() {
        return;
    }
    let Some(saved) = room_tables(&config_root)
        .into_iter()
        .map(|(world, _)| read_saved(&config_root, &world))
        .find(|saved| saved.active && !saved.executor_token.is_empty())
    else {
        return;
    };
    let running = start(engine, saved, Timing::REAL, true).await;
    *lock(&RUNNING) = Some(running);
}

// ── 正式引擎：桌面版既有的 GM／角色／換幕流程 ──

pub(crate) struct AppEngine {
    app: tauri::AppHandle,
    config_root: PathBuf,
    data_root: PathBuf,
}

impl AppEngine {
    pub(crate) fn shared(app: &tauri::AppHandle) -> Result<Arc<dyn Engine>, String> {
        Ok(Arc::new(AppEngine {
            app: app.clone(),
            config_root: crate::config_root(app)?,
            data_root: crate::data_root(app)?,
        }))
    }
}

/// 既有流程收 `tauri::ipc::Channel`：在程序內接住它的每則增量
fn channel(delta: Delta) -> tauri::ipc::Channel<String> {
    tauri::ipc::Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Json(json) = body {
            if let Ok(text) = serde_json::from_str::<String>(&json) {
                delta(&text);
            }
        }
        Ok(())
    })
}

impl Engine for AppEngine {
    fn roots(&self) -> (PathBuf, PathBuf) {
        (self.config_root.clone(), self.data_root.clone())
    }

    fn narrate(
        &self,
        world: String,
        turn: String,
        action: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Narration, String>> {
        Box::pin(async move {
            let app = self.app.clone();
            let n = crate::chat_turn::gm_narrate(app, world, turn, channel(delta), Some(action))
                .await?;
            Ok(Narration {
                state_updates: n.state_update_text(),
                text: n.text,
                raw: n.raw,
                next: n.next,
                next_player: n.next_player,
                arrived: n.arrived_characters,
                aborted: n.aborted,
            })
        })
    }

    fn reply(
        &self,
        world: String,
        card: String,
        turn: String,
        action: String,
        delta: Delta,
    ) -> BoxFut<'_, Result<Spoken, String>> {
        Box::pin(async move {
            let app = self.app.clone();
            let reply = crate::chat_turn::chat_with_character(
                app,
                world,
                card,
                turn,
                channel(delta),
                Some(action),
            )
            .await?;
            Ok(Spoken {
                text: reply.text,
                raw: reply.raw,
                aborted: reply.aborted,
            })
        })
    }

    fn advance(&self, world: String, turn: String) -> BoxFut<'_, Result<u64, String>> {
        // 換幕摘要沿用桌面版「換幕」鈕同一條（含停止登記）
        Box::pin(crate::commands::scene::advance_scene(
            self.app.clone(),
            world,
            Some(turn),
            // 房間換幕不是 VN 章末「下一章」，不加章
            None,
        ))
    }

    fn abort(&self, world: &str, turn: &str) {
        crate::inflight::abort_turn(world, turn);
    }

    fn use_settings(&self, settings: Option<(String, String)>) {
        data::set_play_override(settings);
    }

    fn capabilities(&self) -> Value {
        let config = data::read_config(&self.config_root).unwrap_or_default();
        seed::capabilities(&config)
    }

    fn table_changed(&self, world: &str) {
        use tauri::Emitter;
        let _ = self
            .app
            .emit("room-table-changed", json!({ "worldId": world }));
    }
}

#[cfg(test)]
mod tests;
