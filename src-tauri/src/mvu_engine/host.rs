//! 執行緒與通道（規格 2.2 節）：一桌一條專用執行緒持有自己的 runtime，最多保留 `max_tables` 桌（LRU 淘汰）。
//! 呼叫端同步等待（P4 在 async 裡用 spawn_blocking 包）；外層逾時就作廢該執行緒、下次改開新的，
//! 執行中＋作廢未結束的執行緒數（全域名額）與作廢重開次數（每桌）都有上限，超過就整次失敗。被 LRU 淘汰但還在跑的執行緒，
//! 呼叫端手上仍握著它的追蹤（Ticket），逾時照樣作廢、計數。

use super::contract::{
    BatchOutput, DetailedResult, EngineFailure, FailureKind, InitOutput, Limits, Macros,
    SchemaScript, SeedBase, MAX_CHANGES, MAX_RESULTS, MESSAGE_BYTES, SCHEMA_SOURCE_BYTES,
    TABLE_BYTES,
};
use super::runtime::{EngineRuntime, Guard};
use serde_json::{Map, Value};
use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

pub(super) enum Op {
    Batch {
        message: String,
        old_json: String,
        macros_json: String,
    },
    Initialize {
        stat_json: String,
    },
    Seed {
        initvars_json: String,
        opening: String,
        macros_json: String,
        base_json: String,
    },
    /// 測試用：模擬卡在不受中斷的原生程式碼裡
    #[cfg(test)]
    Stall(std::time::Duration),
    /// 測試用：執行緒 panic
    #[cfg(test)]
    Panic,
    /// 測試用：在這桌的 runtime 裡求值一段 JS（回 JSON）
    #[cfg(test)]
    Eval(String),
}

struct Job {
    schema: Arc<SchemaScript>,
    op: Op,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<(String, u32), EngineFailure>>,
    /// 跟著請求走：執行緒做完（或請求沒被處理就丟掉）才歸還；卡住的執行緒一直握著
    _permit: Permit,
}

/// 「執行中＋作廢未結束」的執行緒名額（全域）。送出請求前取得，所以就算 worker 被 LRU 淘汰、之後才逾時，
/// 卡住的執行緒總數也不會超過上限
struct Permit(Arc<AtomicUsize>);

impl Permit {
    fn acquire(busy: &Arc<AtomicUsize>, max: usize) -> Option<Self> {
        busy.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |held| {
            (held < max).then_some(held + 1)
        })
        .ok()
        .map(|_| Self(Arc::clone(busy)))
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 執行緒生命狀態：作廢與結束誰先誰後都只計數一次
const LIVE: u8 = 0;
const ABANDONED: u8 = 1;
const EXITED: u8 = 2;

/// 一條執行緒的追蹤：池裡一份、送出請求的呼叫端一份（被淘汰後仍能作廢、計數）
#[derive(Clone)]
struct Ticket {
    id: u64,
    jobs: mpsc::Sender<Job>,
    guard: Guard,
    life: Arc<AtomicU8>,
}

struct Worker {
    table: String,
    ticket: Ticket,
}

#[derive(Default)]
struct Pool {
    /// 前面最久沒用
    workers: Vec<Worker>,
    /// 每桌作廢重開的時間（`respawn_window` 內的才算）
    abandons: HashMap<String, VecDeque<Instant>>,
    next_id: u64,
}

/// 桌面 MVU 引擎：zod 卡的 schema 與共用核心在隔離的 QuickJS 裡跑
pub struct MvuEngine {
    limits: Limits,
    pool: Mutex<Pool>,
    /// 作廢後還沒結束的執行緒數（執行緒結束時自己減）
    stuck: Arc<AtomicUsize>,
    /// 握著名額的請求數（執行中、排隊中、作廢未結束）
    busy: Arc<AtomicUsize>,
}

impl MvuEngine {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            pool: Mutex::new(Pool::default()),
            stuck: Arc::new(AtomicUsize::new(0)),
            busy: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 一次處理的期限：起算一次，同一次處理的所有請求（含後處理後的第二次驗證）共用，不重新起算
    pub fn deadline(&self) -> Instant {
        Instant::now() + self.limits.batch
    }

    /// parseMessageDetailed：完整新表＋逐條結果＋變動。`table` 是桌的識別（每桌各自的 runtime）
    pub fn run_batch(
        &self,
        table: &str,
        schema: &Arc<SchemaScript>,
        message: &str,
        old: &Map<String, Value>,
        macros: &Macros,
        deadline: Instant,
    ) -> Result<BatchOutput, EngineFailure> {
        check_schema(schema)?;
        if message.len() > MESSAGE_BYTES {
            return Err(EngineFailure::new(
                FailureKind::InputTooLarge,
                "訊息超過 256 KB",
            ));
        }
        let old_json = table_json(old)?;
        let macros_json = serde_json::to_string(macros)
            .map_err(|error| EngineFailure::new(FailureKind::InputTooLarge, error.to_string()))?;
        let op = Op::Batch {
            message: message.to_owned(),
            old_json,
            macros_json,
        };
        let (json, _) = self.dispatch(table, schema, op, deadline)?;
        let detailed: DetailedResult = serde_json::from_str(&json)
            .map_err(|error| bad_output(format!("回傳不符契約：{error}")))?;
        if detailed.results.len() > MAX_RESULTS || detailed.changes.len() > MAX_CHANGES {
            return Err(bad_output("逐條結果或變動筆數超過上限".into()));
        }
        // 核心自己會擋，但卡片腳本與核心同一個 realm：在 Rust 再驗一次表的大小
        table_json(&detailed.table).map_err(|failure| bad_output(failure.detail))?;
        check_deadline(deadline)?;
        Ok(BatchOutput { detailed })
    }

    /// 開局：initvar 的 stat_data 先 cleanUpMetadata 再過 schema
    pub fn initialize(
        &self,
        table: &str,
        schema: &Arc<SchemaScript>,
        stat_data: &Value,
        deadline: Instant,
    ) -> Result<InitOutput, EngineFailure> {
        check_schema(schema)?;
        let stat_json = serde_json::to_string(stat_data)
            .map_err(|error| EngineFailure::new(FailureKind::InputTooLarge, error.to_string()))?;
        if stat_json.len() > TABLE_BYTES {
            return Err(EngineFailure::new(
                FailureKind::InputTooLarge,
                "變數表超過 2 MB",
            ));
        }
        let (json, duplicates) =
            self.dispatch(table, schema, Op::Initialize { stat_json }, deadline)?;
        init_output(&json, duplicates, deadline)
    }

    /// 種子（MVU initCheck）：卡內各條 `[initvar]` 原文依序在引擎裡保序解析深合併（卡上沒有 initvar 時改以狀態樹
    /// 物化的 stat_data 為底），開場白的 `<initvar>` 區塊整份取代，再同 `initialize`。任一條 initvar 解析失敗＝整次失敗
    pub fn seed(
        &self,
        table: &str,
        schema: &Arc<SchemaScript>,
        base: SeedBase<'_>,
        opening: &str,
        macros: &Macros,
        deadline: Instant,
    ) -> Result<InitOutput, EngineFailure> {
        check_schema(schema)?;
        let too_large = |error: serde_json::Error| {
            EngineFailure::new(FailureKind::InputTooLarge, error.to_string())
        };
        let (initvars_json, base_json) = match base {
            SeedBase::Initvars(initvars) => (
                serde_json::to_string(initvars).map_err(too_large)?,
                "null".to_owned(),
            ),
            SeedBase::Tree(stat) => (
                "[]".to_owned(),
                serde_json::to_string(stat).map_err(too_large)?,
            ),
        };
        if initvars_json.len() + base_json.len() + opening.len() > TABLE_BYTES {
            return Err(EngineFailure::new(
                FailureKind::InputTooLarge,
                "initvar 與開場白超過 2 MB",
            ));
        }
        let macros_json = serde_json::to_string(macros)
            .map_err(|error| EngineFailure::new(FailureKind::InputTooLarge, error.to_string()))?;
        let op = Op::Seed {
            initvars_json,
            opening: opening.to_owned(),
            macros_json,
            base_json,
        };
        let (json, duplicates) = self.dispatch(table, schema, op, deadline)?;
        init_output(&json, duplicates, deadline)
    }

    fn dispatch(
        &self,
        table: &str,
        schema: &Arc<SchemaScript>,
        op: Op,
        deadline: Instant,
    ) -> Result<(String, u32), EngineFailure> {
        check_deadline(deadline)?;
        let permit =
            Permit::acquire(&self.busy, self.limits.max_busy_threads).ok_or_else(|| {
                EngineFailure::new(
                    FailureKind::Busy,
                    "引擎執行緒（執行中與作廢未結束）已達上限",
                )
            })?;
        let (reply, answer) = mpsc::sync_channel(1);
        let mut job = Job {
            schema: Arc::clone(schema),
            op,
            deadline,
            reply,
            _permit: permit,
        };
        // 執行緒閒置過久自己結束、或剛被淘汰時送不出去：換一條新的再送一次
        let mut sent = None;
        for _ in 0..2 {
            let ticket = self.worker_for(table)?;
            match ticket.jobs.send(job) {
                Ok(()) => {
                    sent = Some(ticket);
                    break;
                }
                Err(mpsc::SendError(returned)) => {
                    job = returned;
                    self.remove(ticket.id);
                }
            }
        }
        let Some(ticket) = sent else {
            return Err(EngineFailure::new(
                FailureKind::Crashed,
                "引擎執行緒無法接收請求",
            ));
        };
        let wait = deadline.saturating_duration_since(Instant::now()) + self.limits.outer_grace;
        match answer.recv_timeout(wait) {
            Ok(result) => {
                let reply = result?;
                check_deadline(deadline)?;
                Ok(reply)
            }
            Err(RecvTimeoutError::Timeout) => {
                self.abandon(table, &ticket);
                Err(EngineFailure::new(
                    FailureKind::Deadline,
                    "引擎沒有在期限內回應，已作廢該執行緒",
                ))
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.remove(ticket.id);
                Err(EngineFailure::new(
                    FailureKind::Crashed,
                    "引擎執行緒意外結束",
                ))
            }
        }
    }

    /// 取這桌的執行緒（移到最近使用）；沒有就開新的，滿了先淘汰最久沒用的
    fn worker_for(&self, table: &str) -> Result<Ticket, EngineFailure> {
        let mut pool = self.pool.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = pool.workers.iter().position(|worker| worker.table == table) {
            let worker = pool.workers.remove(at);
            let found = worker.ticket.clone();
            pool.workers.push(worker);
            return Ok(found);
        }
        let now = Instant::now();
        let window = self.limits.respawn_window;
        pool.abandons.retain(|_, times| {
            while times
                .front()
                .is_some_and(|at| now.duration_since(*at) >= window)
            {
                times.pop_front();
            }
            !times.is_empty()
        });
        if pool.abandons.get(table).map_or(0, VecDeque::len) >= self.limits.max_respawns {
            return Err(EngineFailure::new(
                FailureKind::Busy,
                "這桌的引擎逾時重開次數已達上限",
            ));
        }
        while pool.workers.len() >= self.limits.max_tables.max(1) {
            // 丟掉池裡那份通道：執行緒做完手上的請求就結束；還在跑的由呼叫端的 Ticket 繼續追蹤
            pool.workers.remove(0);
        }
        pool.next_id += 1;
        let ticket = self.spawn(pool.next_id)?;
        pool.workers.push(Worker {
            table: table.to_owned(),
            ticket: ticket.clone(),
        });
        Ok(ticket)
    }

    fn spawn(&self, id: u64) -> Result<Ticket, EngineFailure> {
        let (jobs, inbox) = mpsc::channel();
        let guard = Guard::default();
        let life = Arc::new(AtomicU8::new(LIVE));
        let (thread_guard, thread_life, stuck, limits) = (
            guard.clone(),
            Arc::clone(&life),
            Arc::clone(&self.stuck),
            self.limits.clone(),
        );
        thread::Builder::new()
            .name("mvu-engine".into())
            .stack_size(self.limits.thread_stack_bytes)
            .spawn(move || {
                worker_loop(inbox, &thread_guard, &limits);
                if thread_life.swap(EXITED, Ordering::SeqCst) == ABANDONED {
                    stuck.fetch_sub(1, Ordering::SeqCst);
                }
            })
            .map_err(|error| {
                EngineFailure::new(FailureKind::Crashed, format!("開不了引擎執行緒：{error}"))
            })?;
        Ok(Ticket {
            id,
            jobs,
            guard,
            life,
        })
    }

    fn remove(&self, id: u64) {
        let mut pool = self.pool.lock().unwrap_or_else(PoisonError::into_inner);
        pool.workers.retain(|worker| worker.ticket.id != id);
    }

    /// 外層逾時：執行緒標成作廢（還沒結束才計數）、催它中斷、移出池（已被淘汰也照做），記入這桌的重開次數
    fn abandon(&self, table: &str, ticket: &Ticket) {
        let mut pool = self.pool.lock().unwrap_or_else(PoisonError::into_inner);
        // 先加再改狀態：執行緒若同時結束，看到 ABANDONED 才減，不會先減後加
        self.stuck.fetch_add(1, Ordering::SeqCst);
        if ticket
            .life
            .compare_exchange(LIVE, ABANDONED, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.stuck.fetch_sub(1, Ordering::SeqCst);
        }
        ticket.guard.expire_now();
        pool.workers.retain(|worker| worker.ticket.id != ticket.id);
        pool.abandons
            .entry(table.to_owned())
            .or_default()
            .push_back(Instant::now());
    }

    #[cfg(test)]
    pub(super) fn run_op(
        &self,
        table: &str,
        schema: &Arc<SchemaScript>,
        op: Op,
        deadline: Instant,
    ) -> Result<String, EngineFailure> {
        self.dispatch(table, schema, op, deadline)
            .map(|(json, _)| json)
    }

    #[cfg(test)]
    pub(super) fn stuck_threads(&self) -> usize {
        self.stuck.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(super) fn tables(&self) -> Vec<String> {
        let pool = self.pool.lock().unwrap_or_else(PoisonError::into_inner);
        pool.workers
            .iter()
            .map(|worker| worker.table.clone())
            .collect()
    }
}

/// 開局與種子的回覆：表大小再驗一次、拆出驗證錯誤
fn init_output(
    json: &str,
    duplicates: u32,
    deadline: Instant,
) -> Result<InitOutput, EngineFailure> {
    let mut reply: Value = serde_json::from_str(json)
        .map_err(|error| bad_output(format!("回傳不是 JSON：{error}")))?;
    let stat_data = reply
        .get_mut("stat_data")
        .map(Value::take)
        .unwrap_or(Value::Null);
    if serde_json::to_string(&stat_data).map_or(true, |text| text.len() > TABLE_BYTES) {
        return Err(bad_output("開局結果超過 2 MB".into()));
    }
    let outcome = &reply["outcome"];
    let invalid = match outcome["ok"].as_bool() {
        Some(true) => None,
        Some(false) => {
            let issues = outcome["issues"]
                .as_array()
                .map(|issues| {
                    issues
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Some((issues, outcome["issueCount"].as_u64().unwrap_or(0) as usize))
        }
        None => return Err(bad_output("開局回傳缺 outcome".into())),
    };
    check_deadline(deadline)?;
    Ok(InitOutput {
        stat_data,
        invalid,
        duplicates,
    })
}

/// 期限一到就整次失敗：回成功前、輸出驗證後都要再對一次（中斷只在 JS 執行中輪詢，原生程式與 Rust 的處理看不到）
fn check_deadline(deadline: Instant) -> Result<(), EngineFailure> {
    if Instant::now() >= deadline {
        return Err(EngineFailure::new(
            FailureKind::Deadline,
            "整次處理超過期限",
        ));
    }
    Ok(())
}

fn check_schema(schema: &SchemaScript) -> Result<(), EngineFailure> {
    if schema.source.len() > SCHEMA_SOURCE_BYTES {
        return Err(EngineFailure::new(
            FailureKind::InputTooLarge,
            "卡片 schema 腳本超過 256 KB",
        ));
    }
    Ok(())
}

fn table_json(table: &Map<String, Value>) -> Result<String, EngineFailure> {
    let json = serde_json::to_string(table)
        .map_err(|error| EngineFailure::new(FailureKind::InputTooLarge, error.to_string()))?;
    if json.len() > TABLE_BYTES {
        return Err(EngineFailure::new(
            FailureKind::InputTooLarge,
            "變數表超過 2 MB",
        ));
    }
    Ok(json)
}

fn bad_output(detail: String) -> EngineFailure {
    EngineFailure::new(FailureKind::BadOutput, detail)
}

/// 執行緒主迴圈：閒置過久丟掉 runtime（下次重建）；通道關閉就結束
fn worker_loop(inbox: mpsc::Receiver<Job>, guard: &Guard, limits: &Limits) {
    let mut runtime: Option<EngineRuntime> = None;
    loop {
        let job = match inbox.recv_timeout(limits.idle) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => {
                runtime = None;
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let result = match catch_unwind(AssertUnwindSafe(|| {
            process(&mut runtime, &job, guard, limits)
        })) {
            Ok(result) => result,
            Err(_) => {
                // panic 後 runtime 狀態不明：不釋放（釋放可能觸發 QuickJS 的斷言讓整個 app 中止），直接放掉
                if let Some(broken) = runtime.take() {
                    std::mem::forget(broken);
                }
                Err(EngineFailure::new(FailureKind::Crashed, "引擎 panic"))
            }
        };
        guard.disarm();
        let _ = job.reply.send(result);
    }
}

/// 一個請求：期限已過就不跑；schema 變了或上次出錯就重建 runtime；任何失敗都丟掉 runtime（下次重建）
fn process(
    runtime: &mut Option<EngineRuntime>,
    job: &Job,
    guard: &Guard,
    limits: &Limits,
) -> Result<(String, u32), EngineFailure> {
    check_deadline(job.deadline)?;
    guard.arm(job.deadline);
    if runtime
        .as_ref()
        .is_some_and(|current| current.schema_hash != job.schema.hash)
    {
        *runtime = None;
    }
    if runtime.is_none() {
        *runtime = Some(EngineRuntime::build(&job.schema, limits, guard)?);
    }
    let Some(current) = runtime.as_ref() else {
        return Err(EngineFailure::new(FailureKind::Crashed, "runtime 不存在"));
    };
    let result = match &job.op {
        Op::Batch {
            message,
            old_json,
            macros_json,
        } => current.parse(message, old_json, macros_json, guard),
        Op::Initialize { stat_json } => current.initialize(stat_json, guard),
        Op::Seed {
            initvars_json,
            opening,
            macros_json,
            base_json,
        } => current.seed(initvars_json, opening, macros_json, base_json, guard),
        #[cfg(test)]
        Op::Stall(duration) => {
            thread::sleep(*duration);
            Ok("null".into())
        }
        #[cfg(test)]
        Op::Panic => panic!("測試用 panic"),
        #[cfg(test)]
        Op::Eval(code) => current.eval_for_test(code, guard),
    };
    // 中斷不可被 catch：觸發過就算 JS 照常回了結果也不認
    let result = if guard.fired() {
        Err(EngineFailure::new(
            FailureKind::Deadline,
            "整次處理超過期限",
        ))
    } else {
        result
    };
    match result {
        Ok(json) => {
            current.collect_garbage();
            // 跑完（含 GC）已過期限：不認結果；runtime 本身沒壞，留著
            check_deadline(job.deadline)?;
            Ok((json, current.duplicates))
        }
        Err(failure) => {
            *runtime = None;
            Err(failure)
        }
    }
}
