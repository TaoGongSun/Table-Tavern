//! 每桌一把讀寫鎖。寫入 command 持共用許可直到最後一次落檔（可跨 await）；
//! 轉換、改用備份、刪桌取獨占，拿不到就回 busy，不跟在途寫入排隊。
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

fn locks() -> &'static Mutex<HashMap<String, std::sync::Arc<RwLock<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Arc<RwLock<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_for(world_id: &str) -> std::sync::Arc<RwLock<()>> {
    let mut map = locks().lock().unwrap_or_else(|poison| poison.into_inner());
    map.entry(world_id.to_owned())
        .or_insert_with(|| std::sync::Arc::new(RwLock::new(())))
        .clone()
}

/// 共用許可。丟掉才放鎖，所以 command 要把它留到函式結束。
pub struct WorldWritePermit {
    _guard: OwnedRwLockReadGuard<()>,
}

pub fn world_write_permit(world_id: &str) -> WorldWritePermit {
    // tokio 1.53 的 RwLock 沒有 blocking_read_owned。獨占只在轉換／還原的同步檔案
    // 操作期間持有，這裡用 try_read 短睡等到它放開；try_write 失敗的那條不在這裡等。
    let lock = lock_for(world_id);
    loop {
        match lock.clone().try_read_owned() {
            Ok(guard) => return WorldWritePermit { _guard: guard },
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}

pub async fn world_write_permit_async(world_id: &str) -> WorldWritePermit {
    WorldWritePermit {
        _guard: lock_for(world_id).read_owned().await,
    }
}

pub struct WorldExclusive {
    _guard: OwnedRwLockWriteGuard<()>,
}

/// 有共用許可或另一個獨占還在就回 None（呼叫端改回 busy，不排隊）。
pub fn try_world_exclusive(world_id: &str) -> Option<WorldExclusive> {
    lock_for(world_id)
        .try_write_owned()
        .ok()
        .map(|guard| WorldExclusive { _guard: guard })
}
