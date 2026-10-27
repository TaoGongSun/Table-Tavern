# 桌面 MVU 引擎累加配置與 TypedArray 記憶體炸彈

已結案進 main。結論：原生 quickjs-ng 0.16.2 的 ArrayBuffer／TypedArray／不可成長 SharedArrayBuffer 配置走 `js_calloc_rt`、resize／transfer 走 `js_realloc_rt`，都先對 malloc_limit 檢查（rquickjs 0.14 沒裝 SAB 配置掛鉤）；網頁版 wasm 0.12.1 擋不住的那類炸彈在桌面擋得住。產品碼不動。

- 測項（`src-tauri/src/mvu_engine/tests/limits.rs`）：累加 16MB×最多 64 塊（ArrayBuffer、Uint8Array、Float64Array）在正式 64MB 判 Memory；SharedArrayBuffer 累加、resize 每次長 16MB、單次大配置（Uint8Array、Int32Array、SharedArrayBuffer、resize、transfer 擴容，前置先驗成立）在 32MB 判 Memory；對照組 4MB 用完即丟 ×64 在 64MB、32MB 都跑滿回傳。
- 重建證據：同桌同 schema 的模組層計數器觸限前 1→2、觸限後重新 1→2（換 schema 會因雜湊重建，分不出來）；既有 memory_bombs 同樣補上。整次不提交以引擎層回 Err(Memory) 為證。
- 不加接線層測項〔模型判斷·三方共識〕：Err 之後接線共用同一條失敗處理，已有測項覆蓋。
- 將來升級 rquickjs／quickjs-ng 時這組測項是防線（例如改裝外部 SAB 配置器就會紅）。
- 驗證：負載 1 分鐘平均 ≥ 50 下 mvu_engine 模組 30 有效跑 0 紅（負載 52–174）；verify 12 步綠；Sol、Grok 審查與驗收 DONE。
