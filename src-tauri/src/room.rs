//! 房間模式（multiplayer-room 包 2）：桌面版當 GM 執行端，規格見 `.ai/plans/multiplayer-room-protocol.md` §6、§7、§14。
//! 執行端迴圈整個在 Rust 的 tokio 任務裡（人不在電腦前，視窗縮小或鎖定時 webview 可能被節流，不賭它醒著）：
//! `link` 管連線、心跳、重連退避與重送佇列；`run` 管派工、GM 鏈接力、換幕工作與遠端控制；`seed` 產公開名冊、
//! 公開歷史與 capabilities；`awake` 防休眠。GM 引擎本身不動，經 `Engine` 呼叫（測試換成替身）。
mod awake;
mod link;
mod run;
mod seed;

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

/// 本機持久（設定目錄 `room.json`，只限本人可讀）：憑證、房間覆寫、重送佇列、最後一筆派工與工作。
/// 退出／暫停／關程式都只斷線，這份留著；房間不存在或憑證失效才清掉。
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

const FILE: &str = "room.json";
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

pub(crate) fn read_saved(config_root: &Path) -> Saved {
    std::fs::read(config_root.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
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
            self.note_error(&format!("room.json：{error}"));
        }
        result
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
    let path = engine.roots().0.join(FILE);
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

pub(crate) fn status(config_root: &Path) -> Status {
    if let Some(running) = running().as_ref() {
        return running.room.panel();
    }
    let saved = read_saved(config_root);
    let known = |value: String| (!value.is_empty()).then_some(value);
    Status {
        world_id: known(saved.world_id),
        room_code: known(saved.room_code),
        server: known(saved.server).unwrap_or_else(|| OFFICIAL_SERVER.to_owned()),
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

/// 進入房間模式：同一桌同一伺服器有憑證就接回原房間，否則建新房（換桌＝放掉舊房間的憑證）
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
    let server = server.trim().trim_end_matches('/').to_owned();
    if !server_allowed(&server) {
        return Err("ROOM_SERVER_INVALID".to_owned());
    }
    let config_root = engine.roots().0;
    let mut saved = read_saved(&config_root);
    if saved.world_id != world_id || saved.server != server || saved.executor_token.is_empty() {
        saved = link::create_room(&server).await?;
        saved.world_id = world_id;
    }
    saved.active = true;
    write_saved(&config_root.join(FILE), &saved)?;
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
    run::abort(&room);
    let job_turn = room.work().job_turn.clone();
    if let Some(turn) = job_turn {
        room.engine.abort(&room.world(), &turn);
    }
    running.stop().await;
    release_table(&room, &room.world())
}

/// 發一張新的 grantHost 配對碼（面板「重設房主」）；結果在下次讀狀態時出現
pub(crate) fn request_pair_code() -> Result<(), String> {
    let slot = running();
    let running = slot.as_ref().ok_or("ROOM_NOT_ACTIVE")?;
    link::request_pair(&running.room, true);
    Ok(())
}

/// 開程式時：上次沒有按退出就自動接回同一房間（協定 §7.4）
pub(crate) async fn resume(engine: Arc<dyn Engine>) {
    let saved = read_saved(&engine.roots().0);
    if !saved.active || saved.executor_token.is_empty() || running().is_some() {
        return;
    }
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
