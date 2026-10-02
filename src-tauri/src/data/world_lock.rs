//! 每桌一把讀寫鎖。寫入 command 持共用許可直到最後一次落檔（可跨 await）；
//! 轉換、改用備份、刪桌取獨占，拿不到就回 busy，不跟在途寫入排隊。
//!
//! 更新閘門與鎖表共用這一把 mutex。開閘後新許可與新桌一律拒絕；在途的共用許可
//! 放掉之後，安裝才拿得到獨占。測試用自己的 [`LockState`]，不升起行程全域旗標，
//! 避免跟平行的 cargo test 搶同一張表。

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use tokio::sync::{Notify, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

pub const UPDATE_GATE_MESSAGE: &str = "更新進行中，暫停寫入";

/// 閘門已開。`?` 進 command 的 `Result<_, String>`，也進資料層的 `DataResult`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateGateClosed;

impl fmt::Display for UpdateGateClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(UPDATE_GATE_MESSAGE)
    }
}

impl Error for UpdateGateClosed {}

impl From<UpdateGateClosed> for String {
    fn from(value: UpdateGateClosed) -> Self {
        value.to_string()
    }
}

struct LockState {
    gate_raised: bool,
    worlds: HashMap<String, Arc<RwLock<()>>>,
    notify: Arc<Notify>,
}

impl LockState {
    fn new() -> Self {
        Self {
            gate_raised: false,
            worlds: HashMap::new(),
            notify: Arc::new(Notify::new()),
        }
    }
}

fn global_state() -> &'static Mutex<LockState> {
    static STATE: OnceLock<Mutex<LockState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(LockState::new()))
}

fn lock_state(state: &Mutex<LockState>) -> MutexGuard<'_, LockState> {
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn gate_is_raised(state: &Mutex<LockState>) -> bool {
    lock_state(state).gate_raised
}

/// 測試把「設定寫入看得到的閘門」抬起來，不動行程全域旗標。離開作用域就還原。
#[cfg(test)]
pub(crate) struct GateOverride {
    previous: Option<bool>,
}

#[cfg(test)]
thread_local! {
    static FORCE_GATE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl GateOverride {
    pub(crate) fn raised() -> Self {
        let previous = FORCE_GATE.with(|cell| cell.replace(Some(true)));
        Self { previous }
    }
}

#[cfg(test)]
impl Drop for GateOverride {
    fn drop(&mut self) {
        FORCE_GATE.with(|cell| cell.set(self.previous));
    }
}

/// 正式路徑看全域旗標。測試若設了執行緒覆寫，覆寫優先——設定檔測試才不會碰到全域鎖表。
pub(crate) fn update_gate_raised() -> bool {
    #[cfg(test)]
    {
        if let Some(forced) = FORCE_GATE.with(|cell| cell.get()) {
            return forced;
        }
    }
    gate_is_raised(global_state())
}

pub(crate) fn refuse_if_updating() -> Result<(), UpdateGateClosed> {
    if update_gate_raised() {
        Err(UpdateGateClosed)
    } else {
        Ok(())
    }
}

/// 共用許可。丟掉才放鎖，所以 command 要把它留到函式結束。
#[derive(Debug)]
pub struct WorldWritePermit {
    _guard: OwnedRwLockReadGuard<()>,
}

/// 旗標已開就不要把新桌插進表，否則安裝的快照會漏掉這把剛誕生的鎖。
fn lock_for(table: &mut LockState, world_id: &str) -> Result<Arc<RwLock<()>>, UpdateGateClosed> {
    if table.gate_raised {
        return Err(UpdateGateClosed);
    }
    Ok(table
        .worlds
        .entry(world_id.to_owned())
        .or_insert_with(|| Arc::new(RwLock::new(())))
        .clone())
}

pub fn world_write_permit(world_id: &str) -> Result<WorldWritePermit, UpdateGateClosed> {
    write_permit_sync(global_state(), world_id)
}

fn write_permit_sync(
    state: &Mutex<LockState>,
    world_id: &str,
) -> Result<WorldWritePermit, UpdateGateClosed> {
    // tokio 1.53 的 RwLock 沒有 blocking_read_owned。獨占只在轉換／還原／安裝清場時持有，
    // 這裡用 try_read 短睡等到它放開。每一圈先看旗標，開閘就不必再等。
    let lock = {
        let mut table = lock_state(state);
        lock_for(&mut table, world_id)?
    };
    loop {
        if gate_is_raised(state) {
            return Err(UpdateGateClosed);
        }
        match lock.clone().try_read_owned() {
            Ok(guard) => {
                if gate_is_raised(state) {
                    drop(guard);
                    return Err(UpdateGateClosed);
                }
                return Ok(WorldWritePermit { _guard: guard });
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}

pub async fn world_write_permit_async(
    world_id: &str,
) -> Result<WorldWritePermit, UpdateGateClosed> {
    write_permit_async(global_state(), world_id).await
}

// 非同步等待者進 select 前先登記。測試靠這個數知道人已經停在 select 裡，再來開閘。
#[cfg(test)]
thread_local! {
    static PARKED: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
struct ParkToken;

#[cfg(test)]
impl Drop for ParkToken {
    fn drop(&mut self) {
        PARKED.with(|cell| cell.set(cell.get() - 1));
    }
}

async fn write_permit_async(
    state: &Mutex<LockState>,
    world_id: &str,
) -> Result<WorldWritePermit, UpdateGateClosed> {
    loop {
        let notify = lock_state(state).notify.clone();
        let notified = notify.notified();
        tokio::pin!(notified);
        let lock = {
            let mut table = lock_state(state);
            // Notified 要 enable 才算登記。notify_waiters 不存許可，必須在持鎖時 enable，
            // 否則開閘的通知會在登記前丟失。enable 若吃到舊許可，下面的 select 會立刻走到
            // 通知分支，不會空轉。
            let _registered = notified.as_mut().enable();
            lock_for(&mut table, world_id)?
        };
        #[cfg(test)]
        PARKED.with(|cell| cell.set(cell.get() + 1));
        #[cfg(test)]
        let _parked = ParkToken;
        tokio::select! {
            biased;
            _ = &mut notified => {
                if gate_is_raised(state) {
                    return Err(UpdateGateClosed);
                }
                // 通知到了但旗標已放下（安裝失敗放閘）：這次不算被拒，再等一次。
            }
            guard = lock.read_owned() => {
                if gate_is_raised(state) {
                    drop(guard);
                    return Err(UpdateGateClosed);
                }
                return Ok(WorldWritePermit { _guard: guard });
            }
        }
    }
}

pub struct WorldExclusive {
    _guard: OwnedRwLockWriteGuard<()>,
}

/// 有共用許可、另一個獨占，或閘門已開，就回 None。呼叫端沿用原本的忙碌路徑，不改成閘門那句話。
pub fn try_world_exclusive(world_id: &str) -> Option<WorldExclusive> {
    try_exclusive(global_state(), world_id)
}

fn try_exclusive(state: &Mutex<LockState>, world_id: &str) -> Option<WorldExclusive> {
    let lock = {
        let mut table = lock_state(state);
        if table.gate_raised {
            return None;
        }
        table
            .worlds
            .entry(world_id.to_owned())
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone()
    };
    let guard = lock.try_write_owned().ok()?;
    // 拿到獨占之後旗標可能剛升起。放掉，讓安裝的 write_owned 等得到，也避免自己跨過閘門去寫。
    if gate_is_raised(state) {
        drop(guard);
        return None;
    }
    Some(WorldExclusive { _guard: guard })
}

/// 開閘：設旗標、取出當下所有鎖、叫醒等待中的許可。之後新許可與新桌一律拒絕。
fn raise(state: &Mutex<LockState>) -> Vec<Arc<RwLock<()>>> {
    let mut table = lock_state(state);
    table.gate_raised = true;
    let snapshot: Vec<_> = table.worlds.values().cloned().collect();
    table.notify.notify_waiters();
    snapshot
}

fn lower(state: &Mutex<LockState>) {
    lock_state(state).gate_raised = false;
}

async fn wait_exclusive(snapshot: Vec<Arc<RwLock<()>>>) -> Vec<OwnedRwLockWriteGuard<()>> {
    let mut guards = Vec::with_capacity(snapshot.len());
    for lock in snapshot {
        guards.push(lock.write_owned().await);
    }
    guards
}

/// 安裝進行中。跟閘門分開：第二次呼叫在開閘前就回「已在安裝」，避免兩次清場互相等。
#[derive(Debug)]
pub(crate) struct InstallLock {
    running: AtomicBool,
}

impl InstallLock {
    pub(crate) fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
        }
    }

    pub(crate) fn try_begin(&self) -> Result<InstallTicket<'_>, String> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("已在安裝".to_owned());
        }
        Ok(InstallTicket {
            lock: self,
            active: true,
        })
    }
}

#[derive(Debug)]
pub(crate) struct InstallTicket<'a> {
    lock: &'a InstallLock,
    active: bool,
}

impl InstallTicket<'_> {
    /// 安裝已跨過不歸點。Drop 不再清旗標；程序退出前的第二次呼叫維持「已在安裝」。
    /// `process::exit` 與 `restart() -> !` 不會跑 Drop，不呼叫 commit 也一樣留著。
    fn commit(mut self) {
        self.active = false;
    }
}

impl Drop for InstallTicket<'_> {
    fn drop(&mut self) {
        if self.active {
            self.lock.running.store(false, Ordering::SeqCst);
        }
    }
}

fn global_install_lock() -> &'static InstallLock {
    static LOCK: OnceLock<InstallLock> = OnceLock::new();
    LOCK.get_or_init(InstallLock::new)
}

/// 序列化安裝：先重驗，重驗通過後、開閘前跑 `before_raise`（清略過鍵），
/// 再開閘、等在途許可、拿設定鎖，再跑安裝本體。
/// `prepare` 或 `before_raise` 失敗時閘維持放下。安裝本體失敗才放閘。
/// 成功則把獨占與設定鎖留到程序結束。不要在握著設定 mutex 時 await。
async fn run_gated<T>(
    world: &Mutex<LockState>,
    config: &Mutex<()>,
    install_lock: &InstallLock,
    prepare: impl FnOnce() -> Result<T, String>,
    before_raise: impl FnOnce() -> Result<(), String>,
    install: impl FnOnce(T) -> Result<(), String>,
) -> Result<(), String> {
    let ticket = install_lock.try_begin()?;
    let prepared = prepare()?;
    before_raise()?;
    let snapshot = raise(world);
    let exclusives = wait_exclusive(snapshot).await;
    let config_guard = config.lock().unwrap_or_else(|poison| poison.into_inner());
    match install(prepared) {
        Ok(()) => {
            std::mem::forget(exclusives);
            std::mem::forget(config_guard);
            ticket.commit();
            Ok(())
        }
        Err(error) => {
            drop(exclusives);
            drop(config_guard);
            lower(world);
            Err(error)
        }
    }
}

pub(crate) async fn install_guarded<T>(
    prepare: impl FnOnce() -> Result<T, String>,
    before_raise: impl FnOnce() -> Result<(), String>,
    install: impl FnOnce(T) -> Result<(), String>,
) -> Result<(), String> {
    run_gated(
        global_state(),
        super::config::config_mutex(),
        global_install_lock(),
        prepare,
        before_raise,
        install,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::time::Duration;

    fn local() -> Mutex<LockState> {
        Mutex::new(LockState::new())
    }

    #[test]
    fn raised_gate_rejects_a_new_permit_and_does_not_insert_the_world() {
        let state = local();
        let _snapshot = raise(&state);
        let error = write_permit_sync(&state, "new").unwrap_err();
        assert_eq!(error.to_string(), UPDATE_GATE_MESSAGE);
        assert!(!lock_state(&state).worlds.contains_key("new"));
    }

    #[test]
    fn raised_gate_rejects_a_new_permit_on_an_existing_world() {
        let state = local();
        let permit = write_permit_sync(&state, "desk").unwrap();
        drop(permit);
        let _snapshot = raise(&state);
        assert!(write_permit_sync(&state, "desk").is_err());
        assert!(lock_state(&state).worlds.contains_key("desk"));
    }

    #[test]
    fn raised_gate_makes_exclusive_busy_without_inserting_a_new_world() {
        let state = local();
        let _snapshot = raise(&state);
        assert!(try_exclusive(&state, "new").is_none());
        assert!(!lock_state(&state).worlds.contains_key("new"));
    }

    #[test]
    fn sync_spinner_returns_because_the_gate_opened_while_exclusive_is_held() {
        let state = Arc::new(local());
        let exclusive = try_exclusive(&state, "desk").unwrap();
        let spinner = {
            let state = Arc::clone(&state);
            std::thread::spawn(move || write_permit_sync(&state, "desk"))
        };
        std::thread::sleep(Duration::from_millis(40));
        let _snapshot = raise(&state);
        let error = spinner.join().unwrap().unwrap_err();
        assert_eq!(error.to_string(), UPDATE_GATE_MESSAGE);
        drop(exclusive);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn async_waiters_return_because_the_gate_opened() {
        let state = Arc::new(local());
        let exclusive = try_exclusive(&state, "desk").unwrap();
        PARKED.with(|cell| cell.set(0));
        let mut handles = Vec::new();
        for _ in 0..3 {
            let state = Arc::clone(&state);
            handles.push(tokio::spawn(async move {
                write_permit_async(&state, "desk").await
            }));
        }
        for _ in 0..100 {
            if PARKED.with(|cell| cell.get()) == 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(PARKED.with(|cell| cell.get()), 3, "等待者還沒停在 select");
        let _snapshot = raise(&state);
        for handle in handles {
            let error = handle.await.unwrap().unwrap_err();
            assert_eq!(error.to_string(), UPDATE_GATE_MESSAGE);
        }
        drop(exclusive);
        assert!(write_permit_sync(&state, "desk").is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn install_waits_until_the_inflight_permit_drops() {
        let world = Arc::new(local());
        let config = Arc::new(Mutex::new(()));
        let install_lock = Arc::new(InstallLock::new());
        let permit = write_permit_sync(&world, "desk").unwrap();
        let phase = Arc::new(AtomicUsize::new(0));
        let world_task = Arc::clone(&world);
        let config_task = Arc::clone(&config);
        let lock_task = Arc::clone(&install_lock);
        let phase_task = Arc::clone(&phase);
        let handle = tokio::spawn(async move {
            run_gated(
                &world_task,
                &config_task,
                &lock_task,
                || Ok(()),
                || Ok(()),
                |_| {
                    phase_task.store(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .await
        });
        for _ in 0..50 {
            tokio::task::yield_now().await;
            assert_eq!(phase.load(Ordering::SeqCst), 0, "許可還在，安裝不該跑");
        }
        assert!(gate_is_raised(&world));
        drop(permit);
        handle.await.unwrap().unwrap();
        assert_eq!(phase.load(Ordering::SeqCst), 1);
        assert!(gate_is_raised(&world));
        assert_eq!(
            install_lock.try_begin().unwrap_err(),
            "已在安裝",
            "成功路徑把安裝旗標留到程序結束"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepare_failure_leaves_the_gate_down_without_clearing() {
        let world = local();
        let config = Mutex::new(());
        let install_lock = InstallLock::new();
        let permit = write_permit_sync(&world, "desk").unwrap();
        let cleared = AtomicBool::new(false);
        let error = run_gated(
            &world,
            &config,
            &install_lock,
            || Err::<(), String>("重驗失敗".to_owned()),
            || {
                cleared.store(true, Ordering::SeqCst);
                Ok(())
            },
            |_: ()| panic!("重驗失敗不該安裝"),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "重驗失敗");
        assert!(!cleared.load(Ordering::SeqCst), "重驗失敗不該清略過鍵");
        assert!(!gate_is_raised(&world));
        drop(permit);
        assert!(write_permit_sync(&world, "later").is_ok());
        assert!(install_lock.try_begin().is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn install_failure_lowers_the_gate_and_releases_the_config_lock() {
        let world = local();
        let config = Mutex::new(());
        let install_lock = InstallLock::new();
        let error = run_gated(
            &world,
            &config,
            &install_lock,
            || Ok(()),
            || Ok(()),
            |_| {
                assert!(config.try_lock().is_err(), "安裝本體執行時設定鎖應在手上");
                Err("無法自動替換".to_owned())
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error, "無法自動替換");
        assert!(!gate_is_raised(&world));
        assert!(config.try_lock().is_ok());
        assert!(write_permit_sync(&world, "desk").is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_second_install_is_rejected_while_the_first_is_clearing() {
        let world = Arc::new(local());
        let config = Arc::new(Mutex::new(()));
        let install_lock = Arc::new(InstallLock::new());
        let permit = write_permit_sync(&world, "desk").unwrap();
        let world_task = Arc::clone(&world);
        let config_task = Arc::clone(&config);
        let lock_task = Arc::clone(&install_lock);
        let handle = tokio::spawn(async move {
            run_gated(
                &world_task,
                &config_task,
                &lock_task,
                || Ok(()),
                || Ok(()),
                |_| Ok(()),
            )
            .await
        });
        for _ in 0..50 {
            if gate_is_raised(&world) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(gate_is_raised(&world));
        let error = run_gated(
            &world,
            &config,
            &install_lock,
            || Ok(()),
            || Ok(()),
            |_| Ok(()),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "已在安裝");
        drop(permit);
        handle.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn clear_runs_after_prepare_and_before_the_gate_rises() {
        let world = Arc::new(local());
        let config = Mutex::new(());
        let install_lock = InstallLock::new();
        let order = Arc::new(Mutex::new(Vec::new()));
        let record = |order: &Arc<Mutex<Vec<&str>>>, world: &Arc<Mutex<LockState>>, step| {
            order.lock().unwrap().push(step);
            gate_is_raised(world)
        };
        run_gated(
            &world,
            &config,
            &install_lock,
            {
                let order = Arc::clone(&order);
                let world = Arc::clone(&world);
                move || {
                    assert!(!record(&order, &world, "prepare"));
                    Ok(())
                }
            },
            {
                let order = Arc::clone(&order);
                let world = Arc::clone(&world);
                move || {
                    assert!(!record(&order, &world, "clear"));
                    Ok(())
                }
            },
            {
                let order = Arc::clone(&order);
                let world = Arc::clone(&world);
                move |_| {
                    assert!(record(&order, &world, "install"));
                    Ok(())
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(
            order.lock().unwrap().as_slice(),
            ["prepare", "clear", "install"]
        );
    }

    #[test]
    fn dropping_the_ticket_allows_another_install() {
        let install_lock = InstallLock::new();
        let ticket = install_lock.try_begin().unwrap();
        assert_eq!(install_lock.try_begin().unwrap_err(), "已在安裝");
        drop(ticket);
        assert!(install_lock.try_begin().is_ok());
    }
}
