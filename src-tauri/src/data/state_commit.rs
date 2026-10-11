//! 每桌一把短提交鎖（計畫 8.4）：逐字稿、狀態快取、卡片變數控制檔、回合紀錄的讀改寫都在這把鎖內做完，
//! 不等模型。鎖順序：世界許可（共用或獨占）→ 短提交鎖 → 逐字稿同檔鎖。
//!
//! 鎖內的函式收 `&CommitTx`；同一條執行緒已持有這桌的鎖時再進來不重取（可重入），所以投影入口
//! `read_state` 與各公開寫入函式鎖內鎖外都能叫。
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// 持有某桌短提交鎖的憑證。只能由 [`with_commit`] 產生。
pub struct CommitTx<'a> {
    pub root: &'a Path,
    pub world_id: &'a str,
    _private: (),
}

fn locks() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    static HELD: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

struct HeldMark(String);

impl Drop for HeldMark {
    fn drop(&mut self) {
        HELD.with(|held| {
            held.borrow_mut().remove(&self.0);
        });
    }
}

/// 拿這桌的短提交鎖做事。鎖是行程內的；鍵含資料根，測試各自的暫存根互不干擾。
pub fn with_commit<T>(root: &Path, world_id: &str, work: impl FnOnce(&CommitTx<'_>) -> T) -> T {
    let key = format!("{}\u{0}{world_id}", root.display());
    let reentered = HELD.with(|held| !held.borrow_mut().insert(key.clone()));
    if reentered {
        return work(&CommitTx {
            root,
            world_id,
            _private: (),
        });
    }
    let _mark = HeldMark(key.clone());
    let lock = locks()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(key)
        .or_default()
        .clone();
    let _held = match lock.try_lock() {
        Ok(held) => held,
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => {
            #[cfg(test)]
            contention_hook::fire(world_id);
            lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        }
    };
    work(&CommitTx {
        root,
        world_id,
        _private: (),
    })
}

/// 測試用：某桌的短提交鎖被占著、這條執行緒正要排隊時呼叫，交錯測試據此確認對方真的在等鎖。
/// 全域的，用到它的測試要彼此排隊（與 `write_hook::TESTS` 共用）。
#[cfg(test)]
pub(crate) mod contention_hook {
    use std::sync::{Arc, Mutex};

    type Hook = Arc<dyn Fn() + Send + Sync>;

    static HOOK: Mutex<Option<(String, Hook)>> = Mutex::new(None);

    pub(crate) struct HookGuard;

    impl Drop for HookGuard {
        fn drop(&mut self) {
            *HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        }
    }

    pub(crate) fn install(world_id: &str, hook: impl Fn() + Send + Sync + 'static) -> HookGuard {
        *HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some((world_id.to_owned(), Arc::new(hook)));
        HookGuard
    }

    pub(super) fn fire(world_id: &str) {
        let hook = HOOK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .filter(|(target, _)| target == world_id)
            .map(|(_, hook)| hook.clone());
        if let Some(hook) = hook {
            hook();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_commit_on_same_world_is_reentrant() {
        let root = std::env::temp_dir();
        let value = with_commit(&root, "nested-lock", |_| {
            with_commit(&root, "nested-lock", |_| 7)
        });
        assert_eq!(value, 7);
        // 外層放掉之後別的執行緒拿得到
        let root2 = root.clone();
        std::thread::spawn(move || with_commit(&root2, "nested-lock", |_| ()))
            .join()
            .unwrap();
        with_commit(&root, "a", |_| with_commit(&root, "b", |_| ()));
    }

    #[test]
    fn commits_on_the_same_world_are_serialized() {
        let root = std::env::temp_dir();
        let counter = Arc::new(Mutex::new((0u32, 0u32)));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let counter = counter.clone();
                let root = root.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        with_commit(&root, "serial-lock", |_| {
                            let mut guard = counter.lock().unwrap();
                            guard.0 += 1;
                            guard.1 = guard.1.max(guard.0);
                            drop(guard);
                            std::thread::yield_now();
                            counter.lock().unwrap().0 -= 1;
                        });
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(counter.lock().unwrap().1, 1);
    }
}
