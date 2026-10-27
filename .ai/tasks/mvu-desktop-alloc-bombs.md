# mvu-desktop-alloc-bombs — 桌面 MVU 引擎補累加配置與 TypedArray 炸彈測項

## Summary
web-mvu-memory-cap 短驗證發現 wasm 版 quickjs-ng 0.12.1 的 setMemoryLimit 擋不住 ArrayBuffer／TypedArray 炸彈。桌面用原生 0.16.2，ArrayBuffer 走 js_malloc 會先對 malloc_limit 檢查，既有測項（`mvu_engine/tests.rs` 單次 512MB ArrayBuffer）判成 Memory；尚未驗的是多次小配置累加（例如 16MB 迴圈）與 TypedArray，以及 transform 觸限後整次不提交、下一次重建。不能先認定原生版有洞。

## Next action
未排程；等 mvu-memory-bomb-flaky 合併（同檔）後補測項，紅了才查產品。

## 驗收方向
累加與 TypedArray 炸彈在 64MB 上限內判 Memory、整次不提交、下次重建；負載下不偶發紅。
