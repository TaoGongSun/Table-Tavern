//! 一桌一個 QuickJS runtime：上限、中斷、模組 loader 白名單、bundle 與卡片 schema 載入（規格 1.5、2.2 節）。
//! 只在 host.rs 的專用執行緒上建立與使用（runtime 不是 Send）。

use super::contract::{EngineFailure, FailureKind, Limits, SchemaScript};
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{
    CatchResultExt, CaughtError, Context, Ctx, Error, Function, Module, Object, Persistent,
    Promise, Runtime, Value,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

/// `npm run build:mvu-bundle` 產生（quickjs-entry.ts）：IIFE，全域名 TTMvu
const BUNDLE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/gen/mvu-engine.js"));
const SHIM_NAME: &str = "tt:mvu_zod";
const SHIM_GLOBAL: &str = "__ttRegisterMvuSchema";
/// 卡片 import 的 mvu_zod 換成這個墊片：開機時抓住註冊函式、立刻從全域拿掉，之後只轉呼叫核心
const SHIM_SOURCE: &str = "const register = globalThis.__ttRegisterMvuSchema;\n\
delete globalThis.__ttRegisterMvuSchema;\n\
export function registerMvuSchema(input) { return register(input); }\n";
const CARD_MODULE: &str = "card-schema";

/// 卡片能 import 的唯一模組：StageDog/tavern_resource 的 mvu_zod.js（jsdelivr 各鏡像、可帶 @版本）。只對應到墊片，從不連網
/// 〔模型判斷·三方共識〕
pub(super) fn is_mvu_zod_url(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("https://") else {
        return false;
    };
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    if !matches!(
        host,
        "testingcf.jsdelivr.net" | "cdn.jsdelivr.net" | "fastly.jsdelivr.net"
    ) {
        return false;
    }
    let Some(path) = path.strip_prefix("gh/StageDog/tavern_resource") else {
        return false;
    };
    let path = match path.strip_prefix('@') {
        Some(versioned) => match versioned.split_once('/') {
            Some((version, tail))
                if !version.is_empty()
                    && version
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)) =>
            {
                tail
            }
            _ => return false,
        },
        None => match path.strip_prefix('/') {
            Some(tail) => tail,
            None => return false,
        },
    };
    path == "dist/util/mvu_zod.js"
}

struct ShimOnly;

impl Resolver for ShimOnly {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        _base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        if is_mvu_zod_url(name) {
            Ok(SHIM_NAME.into())
        } else {
            Err(Error::new_resolving("", name))
        }
    }
}

impl Loader for ShimOnly {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        if name == SHIM_NAME {
            Module::declare(ctx.clone(), name, SHIM_SOURCE)
        } else {
            Err(Error::new_loading(name))
        }
    }
}

/// 期限與中斷：中斷處理器、執行緒、外層逾時（作廢時把期限設成現在）共用
#[derive(Clone, Default)]
pub(super) struct Guard {
    deadline: Arc<Mutex<Option<Instant>>>,
    fired: Arc<AtomicBool>,
}

impl Guard {
    pub(super) fn arm(&self, deadline: Instant) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = Some(deadline);
        self.fired.store(false, Ordering::SeqCst);
    }
    pub(super) fn disarm(&self) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
    /// 作廢：讓還在跑的 JS 在下一次輪詢就中斷
    pub(super) fn expire_now(&self) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }
    pub(super) fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }
    fn should_interrupt(&self) -> bool {
        let due = matches!(*self.deadline.lock().unwrap_or_else(PoisonError::into_inner), Some(at) if Instant::now() >= at);
        if due {
            self.fired.store(true, Ordering::SeqCst);
        }
        due
    }
}

/// 一桌的 runtime：bundle、核心、卡片 schema 都已載入。欄位照釋放順序排：api 要在 runtime 之前放掉
pub(super) struct EngineRuntime {
    api: Persistent<Object<'static>>,
    ctx: Context,
    runtime: Runtime,
    pub(super) schema_hash: [u8; 32],
    pub(super) duplicates: u32,
}

impl EngineRuntime {
    /// 建 runtime、載 bundle、載卡片 schema；任何一步失敗都算 schema 載入失敗（規格 1.5 節）
    pub(super) fn build(
        schema: &SchemaScript,
        limits: &Limits,
        guard: &Guard,
    ) -> Result<Self, EngineFailure> {
        let load_failed = |detail: String| EngineFailure::new(FailureKind::SchemaLoad, detail);
        let runtime =
            Runtime::new().map_err(|error| load_failed(format!("建立 runtime 失敗：{error}")))?;
        runtime.set_memory_limit(limits.memory_bytes);
        runtime.set_max_stack_size(limits.stack_bytes);
        let interrupt = guard.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || interrupt.should_interrupt())));
        runtime.set_loader(ShimOnly, ShimOnly);
        let ctx = Context::full(&runtime)
            .map_err(|error| load_failed(format!("建立 context 失敗：{error}")))?;
        let loaded = ctx.with(
            |ctx| -> Result<(Persistent<Object<'static>>, u32), EngineFailure> {
                let caught = |error: CaughtError<'_>| describe(error, guard);
                ctx.eval::<(), _>(BUNDLE)
                    .catch(&ctx)
                    .map_err(|error| load_failed(format!("引擎載入失敗：{}", caught(error))))?;
                let api = (|| -> rquickjs::Result<Object<'_>> {
                    let namespace: Object = ctx.globals().get("TTMvu")?;
                    let boot: Function = namespace.get("boot")?;
                    let api: Object = boot.call(())?;
                    ctx.globals()
                        .set("TTMvu", Value::new_undefined(ctx.clone()))?;
                    let register: Function = api.get("register")?;
                    ctx.globals().set(SHIM_GLOBAL, register)?;
                    Ok(api)
                })()
                .catch(&ctx)
                .map_err(|error| load_failed(format!("引擎開機失敗：{}", caught(error))))?;
                let evaluated = Module::evaluate(ctx.clone(), CARD_MODULE, schema.source.as_str())
                    .and_then(|promise| promise.finish::<()>())
                    .catch(&ctx);
                // 卡片沒 import 墊片時全域還留著：一律清掉
                let _ = ctx.globals().remove(SHIM_GLOBAL);
                evaluated.map_err(|error| {
                    load_failed(format!("卡片 schema 腳本執行失敗：{}", caught(error)))
                })?;
                let status = call_string(&ctx, &api, "status", ())
                    .map_err(|error| load_failed(format!("讀不到註冊狀態：{}", caught(error))))?;
                let status: serde_json::Value = serde_json::from_str(&status)
                    .map_err(|error| load_failed(format!("註冊狀態格式錯誤：{error}")))?;
                if status["registered"] != serde_json::Value::Bool(true) {
                    return Err(load_failed(
                        "卡片 schema 腳本沒有呼叫 registerMvuSchema".into(),
                    ));
                }
                let duplicates = status["duplicates"]
                    .as_u64()
                    .unwrap_or(0)
                    .min(u32::MAX as u64) as u32;
                Ok((Persistent::save(&ctx, api), duplicates))
            },
        );
        // 載入途中觸發中斷：歸期限（可重試），不歸載入失敗
        if guard.fired() {
            return Err(EngineFailure::new(
                FailureKind::Deadline,
                "載入卡片 schema 時超過期限",
            ));
        }
        let (api, duplicates) = loaded?;
        Ok(Self {
            api,
            ctx,
            runtime,
            schema_hash: schema.hash,
            duplicates,
        })
    }

    /// parseMessageDetailed：回契約 JSON
    pub(super) fn parse(
        &self,
        message: &str,
        old_json: &str,
        macros_json: &str,
        guard: &Guard,
    ) -> Result<String, EngineFailure> {
        self.ctx.with(|ctx| {
            let api = self
                .api
                .clone()
                .restore(&ctx)
                .map_err(|error| EngineFailure::new(FailureKind::Crashed, error.to_string()))?;
            (|| -> rquickjs::Result<String> {
                let parse: Function = api.get("parse")?;
                let promise: Promise = parse.call((message, old_json, macros_json))?;
                promise.finish::<String>()
            })()
            .catch(&ctx)
            .map_err(|error| classify(error, guard))
        })
    }

    /// 開局：回 `{ outcome, stat_data }` JSON
    pub(super) fn initialize(
        &self,
        stat_json: &str,
        guard: &Guard,
    ) -> Result<String, EngineFailure> {
        self.ctx.with(|ctx| {
            let api = self
                .api
                .clone()
                .restore(&ctx)
                .map_err(|error| EngineFailure::new(FailureKind::Crashed, error.to_string()))?;
            call_string(&ctx, &api, "initialize", (stat_json,))
                .map_err(|error| classify(error, guard))
        })
    }

    pub(super) fn collect_garbage(&self) {
        self.runtime.run_gc();
    }

    #[cfg(test)]
    pub(super) fn eval_for_test(&self, code: &str, guard: &Guard) -> Result<String, EngineFailure> {
        self.ctx.with(|ctx| {
            (|| -> rquickjs::Result<String> {
                let value: Value = ctx.eval(code)?;
                match value.as_promise() {
                    Some(promise) => promise.finish::<String>(),
                    None => value.get::<String>(),
                }
            })()
            .catch(&ctx)
            .map_err(|error| classify(error, guard))
        })
    }
}

fn call_string<'js, A: rquickjs::function::IntoArgs<'js>>(
    ctx: &Ctx<'js>,
    api: &Object<'js>,
    name: &str,
    args: A,
) -> Result<String, CaughtError<'js>> {
    (|| -> rquickjs::Result<String> {
        let function: Function = api.get(name)?;
        function.call(args)
    })()
    .catch(ctx)
}

/// 錯誤分類：中斷處理器觸發過一律算期限（中斷不可被 catch，JS 拿不到結果）；記憶體、堆疊照訊息分；
/// 核心的 parseFailure 帶 code
fn classify(error: CaughtError<'_>, guard: &Guard) -> EngineFailure {
    if guard.fired() {
        return EngineFailure::new(FailureKind::Deadline, "整次處理超過期限");
    }
    let (kind, detail) = match error {
        CaughtError::Exception(exception) => {
            let message = exception.message().unwrap_or_default();
            let object = exception.as_object();
            let code: Option<String> = object.get::<_, Option<String>>("code").ok().flatten();
            let name: Option<String> = object.get::<_, Option<String>>("name").ok().flatten();
            let kind = if message.contains("out of memory") || message.contains("string too long") {
                FailureKind::Memory
            } else if message.contains("call stack") {
                FailureKind::Stack
            } else if code.as_deref() == Some("deadline") {
                FailureKind::Deadline
            } else {
                FailureKind::Script { code }
            };
            (
                kind,
                name.map_or_else(|| message.clone(), |name| format!("{name}: {message}")),
            )
        }
        // 記憶體耗盡到連錯誤物件都建不出來時，QuickJS 丟的是 null
        CaughtError::Value(value) if value.is_null() || value.is_undefined() => {
            (FailureKind::Memory, "丟出 null（記憶體耗盡）".into())
        }
        CaughtError::Value(value) => (
            FailureKind::Script { code: None },
            format!("丟出非錯誤值：{value:?}"),
        ),
        // 等不到結果的 Promise（沒有待跑的工作卻還沒結算）
        CaughtError::Error(Error::WouldBlock) => (
            FailureKind::Script {
                code: Some("never-settled".into()),
            },
            "Promise 永遠不會結算".into(),
        ),
        CaughtError::Error(error) => (FailureKind::Script { code: None }, error.to_string()),
    };
    EngineFailure::new(kind, clip(&detail))
}

/// 給人看的錯誤文字（載入失敗用）
fn describe(error: CaughtError<'_>, guard: &Guard) -> String {
    let failure = classify(error, guard);
    match failure.kind {
        FailureKind::Deadline => "逾時中斷".into(),
        FailureKind::Memory => format!("記憶體超過上限（{}）", failure.detail),
        FailureKind::Stack => format!("堆疊超過上限（{}）", failure.detail),
        _ => failure.detail,
    }
}

fn clip(text: &str) -> String {
    text.chars().take(500).collect()
}
