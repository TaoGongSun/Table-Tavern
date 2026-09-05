//! AI 卡重構套用：AI 讀整張匯入卡，把內容拆成角色／介面／機制三類產物（RefactorOutcome），
//! 玩家人審勾選（RefactorSelection）後套用落檔，可一鍵倒退。AI 呼叫是下一包的事，這裡只管
//! 「已經有一份 RefactorOutcome，怎麼套用、怎麼復原」——手寫 JSON 餵進 apply() 就能驗證整條路。

mod apply;
mod interface;
mod types;

pub use apply::apply;
pub use types::{
    normalize_stored_mode, RefactorApplySummary, RefactorCharacter, RefactorInterface,
    RefactorOutcome, RefactorSelection,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
