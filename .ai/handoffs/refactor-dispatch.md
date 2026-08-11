# Handoff: refactor-dispatch

## Current state
2026-08-11 包 1–3 實作完成，代理自驗＋主線複驗全綠（cargo **442**／vitest **94**／npm build 0／check:i18n 0），三個 commit：包 1 `cce182d`、包 2 `60c58f2`、包 3 見 git log。剩包 4 實機驗收（下方 P 清單）。注意：實作先於品質基線（使用者 2026-08-11 指示直接開工），A/B 基線取法見「基線時序（待拍板）」。

## Completed（本次）
- **包 1 檔位下放**：[transport.rs](../../src-tauri/src/transport.rs) `refactor_expand_tier`（API 未設 balanced 退 GM，同 translate_opening 慣例）＋單元測試；三個展開 command 換檔位、survey 留 GM。
- **包 2 取消中止＋孤兒清理**：[inflight.rs](../../src-tauri/src/inflight.rs) 新檔（world 分組取消訊號＋子程序 PID 表＋`kill_all_children`＋測試 T1–T4）；[cli.rs](../../src-tauri/src/cli.rs) `run_cli` 加 `kill_on_drop(true)`＋PID 登記 guard；[lib.rs](../../src-tauri/src/lib.rs) 四個 refactor command 包 `tokio::select!`（survey 也可中止）、新 command `refactor_abort`、builder 改 build+run 在 `RunEvent::Exit` 殺全部子程序；中止錯誤 sentinel＝`refactor-aborted`。
- **包 3 前端 A 拓撲並行**：[refactor-run.ts](../../src/refactor-run.ts)（`runRefactorCalls` 首發建快取＋鏈/池兩線、`withRateLimitRetry` 限流單次退避；11 測試）；[App.tsx:2114](../../src/App.tsx#L2114) `runAiRefactor` 整段重推（人物佇列並行上限 4 ‖ 重寫→介面序列鏈、knownFields 只在鏈上、共用思考字尾、進度「整理中 完成 x/n」、取消分流）；[App.tsx:2270](../../src/App.tsx#L2270) 取消接 `refactor_abort`；i18n 十語系各 +1 鍵 `refactorParallelStep`。

## Verification
- 主線親跑：cargo test 442（基線 438＋inflight 4；另以 `--test-threads=16` 複驗高並行不誤殺）；vitest 94（基線 83＋refactor-run 11）；npm build exit 0；check:i18n exit 0（十語系）。
- 主線親讀：inflight.rs 全檔、cli.rs／lib.rs／lanes.rs diff 逐段、refactor-run.ts 全檔、App.tsx 2110–2274（knownFields 鏈上限定、介面 playable 判定、尾端組產物、取消雙路分流逐項核過）。
- 包 2 代理規格外增動（lanes.rs 三測試加 `lock_real_process_tests` 序列鎖）主線審過：`#[cfg(test)]` 專用、防全域 children 表在測試高並行下互殺，保留。

## 待實機測試清單（P1–P8，使用者操作，全過即結案）
前置：本機 claude CLI OAuth 已過期（2026-08-11 實測 401）——CLI 傳輸相關項目前先 `claude login`。
- [ ] P1 檔位下放：CLI 模式跑重構→資料目錄 `prompt-cache.jsonl`（或額度分頁）：survey 行 model 為 GM 檔（opus 級）、展開行為 balanced（sonnet 級）。
- [ ] P2 並行提速：多人物卡（orc-cave／Dark Wolf）跑重構→進度字「整理中 完成 x/n」遞增，總時長由分鐘級降到一分鐘上下。
- [ ] P3 快取命中：同輪展開呼叫 `cached_tokens>0`（balanced 首發多一次 cache write 屬預期，其餘命中）。
- [ ] P4 盤點中取消：survey 進行中按取消→進度立即收掉、該呼叫不燒完（額度分頁輸出中斷）。
- [ ] P5 並行中取消：展開中按取消→在途全部停止；已完成項照樣出結果卡；取消項不列入失敗訊息。
- [ ] P6 Cmd-Q 孤兒：重構跑一半 Cmd-Q→`ps aux | grep -i claude` 無殘留 CLI 子程序。
- [ ] P7 品質 A/B：orc-cave 新產物 vs 基線（取法見下節）——機制 RULES/TRIGGERS 可用、人物欄位不掉；不行→`refactor_expand_tier` 把 rewrite 升回 best 再比。
- [ ] P8（可選）API 直連未設 balanced 模型→重構照常可用（退 GM 檔）。

P2／P7 可與 [ai-card-refactor](../tasks/ai-card-refactor.md) 的 B 段真跑合併執行（同一次跑同時驗兩案，省額度）。

## 基線時序（待拍板）
原拍板「先立基線再優化」，實作已先行。P7 的「舊管線基線」兩個取法：
- **a. 嚴格 A/B**：`git checkout cce182d^` 舊管線跑一輪 orc-cave 留產物與耗時，回 main 再跑新版對比——證據硬，多花一輪額度。
- **b. 絕對標準**：只跑新版，用「RULES/TRIGGERS 實際可跑、欄位齊全」直接驗——省一輪額度，時間對比只剩體感。
兩案都由使用者自己觸發（不替玩家花錢紅線）。

## Next action
使用者：`claude login` → 拍板基線時序 a/b → 照 P1–P8 實測。全過→任務結案（狀態 completed、TASKS.md 行搬 DONE.md、本檔已驗收段搬 archive/）；有紅→帶現象回來修。

## Constraints
- survey／expand 共用 system 逐位元組相同的快取紅線照舊，system 組裝零觸碰（本次已驗：knownFields 等階段差異只在 user 訊息）。
- 機制重寫品質是下放最敏感點：P7 不過就單獨升回 best，不整包回滾。
- knownFields 欄位命名單一權威不得犧牲（並行拓撲 A 已固定此語意）。
- 並行上限 4；實測穩定前不上調。
