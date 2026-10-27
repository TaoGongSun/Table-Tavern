# mvu_engine 測試在負載下偶發 Deadline（記憶體炸彈等）

已結案進 main。根因是測試配置：全模組引擎用 3 秒期限，建 runtime（閒置約 0.3 秒、負載衝高時超過 5 秒）和受測動作擠在同一個期限裡；炸彈時間大多花在不輪詢中斷的 QuickJS 原生程式，超過期限＋寬限就被作廢報 Deadline。產品端不改：中斷觸發過報 Deadline、Memory 在寬限內送回照報 Memory，照設計運作。

- 測試（安全網逾時與受測期限分離）：引擎一律用 `roomy()`（期限 30 秒只當安全網），給預期不是 Deadline 的呼叫；預期 Deadline 的用 `measured()`（3 秒）或自訂期限；量時間的先用同桌同 schema 的無害值暖機，觸限拆掉 runtime 後下一次受測前再暖機。
- 會炸記憶體的引擎 `bomb_engine()` 上限 32 MB（觸頂快一倍），產品預設維持 64 MB。本案新增斷言：字串炸彈走 InternalError（out of memory）、小物件炸彈走丟出 null，兩條 OOM 分類路徑都驗到。
- 拆檔：`mvu_engine/tests.rs` 改成 `tests/mod.rs`，上限類測試拆到 `tests/limits.rs`，收集數不變（30 支）。
- 驗證：負載 1 分鐘平均 ≥ 50 下 mvu_engine 模組 30 有效跑 0 紅（負載 61–170，其中 12 跑 ≥ 100；改前負載 52–83 起就有紅）；verify 12 步綠；Sol、Grok 審查與驗收 DONE。
