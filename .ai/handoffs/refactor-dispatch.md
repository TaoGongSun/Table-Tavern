# Handoff: refactor-dispatch

## Current state
2026-08-11 包 1–3 實作完成，代理自驗＋主線複驗全綠（cargo **442**／vitest **94**／npm build 0／check:i18n 0），三個 commit：包 1 `cce182d`、包 2 `60c58f2`、包 3 見 git log。同日實機開跑 orc-cave：檔位與快取生效（P1／P3 綠），但總時長仍 ~24 分（P2 紅）且盤點分類不合格（P7 紅）——提速與品質改由 [refactor-survey-spans](refactor-survey-spans.md) 接手（設計與證據在該任務檔），本案僅剩取消類驗收（P4–P6、P8）等新案完成後合併驗。

## Completed（本次）
- **包 1 檔位下放**：[transport.rs](../../src-tauri/src/transport.rs) `refactor_expand_tier`（API 未設 balanced 退 GM，同 translate_opening 慣例）＋單元測試；三個展開 command 換檔位、survey 留 GM。
- **包 2 取消中止＋孤兒清理**：[inflight.rs](../../src-tauri/src/inflight.rs) 新檔（world 分組取消訊號＋子程序 PID 表＋`kill_all_children`＋測試 T1–T4）；[cli.rs](../../src-tauri/src/cli.rs) `run_cli` 加 `kill_on_drop(true)`＋PID 登記 guard；[lib.rs](../../src-tauri/src/lib.rs) 四個 refactor command 包 `tokio::select!`（survey 也可中止）、新 command `refactor_abort`、builder 改 build+run 在 `RunEvent::Exit` 殺全部子程序；中止錯誤 sentinel＝`refactor-aborted`。
- **包 3 前端 A 拓撲並行**：[refactor-run.ts](../../src/refactor-run.ts)（`runRefactorCalls` 首發建快取＋鏈/池兩線、`withRateLimitRetry` 限流單次退避；11 測試）；[App.tsx:2114](../../src/App.tsx#L2114) `runAiRefactor` 整段重推（人物佇列並行上限 4 ‖ 重寫→介面序列鏈、knownFields 只在鏈上、共用思考字尾、進度「整理中 完成 x/n」、取消分流）；[App.tsx:2270](../../src/App.tsx#L2270) 取消接 `refactor_abort`；i18n 十語系各 +1 鍵 `refactorParallelStep`。

## Verification
- 主線親跑：cargo test 442（基線 438＋inflight 4；另以 `--test-threads=16` 複驗高並行不誤殺）；vitest 94（基線 83＋refactor-run 11）；npm build exit 0；check:i18n exit 0（十語系）。
- 主線親讀：inflight.rs 全檔、cli.rs／lib.rs／lanes.rs diff 逐段、refactor-run.ts 全檔、App.tsx 2110–2274（knownFields 鏈上限定、介面 playable 判定、尾端組產物、取消雙路分流逐項核過）。
- 包 2 代理規格外增動（lanes.rs 三測試加 `lock_real_process_tests` 序列鎖）主線審過：`#[cfg(test)]` 專用、防全域 children 表在測試高並行下互殺，保留。

## 實機驗收結果（2026-08-11 orc-cave 實跑，prompt-cache.jsonl 實證）
- [x] P1 檔位下放 **✓**：survey＝claude-opus-4-7（GM）、展開＝claude-sonnet-4-6（balanced）×17。
- [x] P2 並行提速 **✗**：總時長 ~24 分（14:21–14:45）。人物段並行有動（六筆小輸出間隔 15–40 秒收束），但條目重寫→介面序列鏈每筆輸出 7–18k、共 ~82k tokens，獨佔 ~19 分（~60 tok/s 序列生成）。
- [x] P3 快取命中 **✓**：展開幾乎每筆 `cached_tokens=11,494`（hit_rate ~88%）。
- [x] P7 品質 **✗**（基線拍板 b＝絕對標準）：純設定條目（豺狼人／深藍狼／巨魔等）被整篇重寫；逐日機制「巴古克與古茲卡入侵劇情線」未接管。分類與產出策略缺陷，修法見 [refactor-survey-spans](refactor-survey-spans.md)。
- [ ] P4／P5／P6（取消／孤兒）、P8（API 退檔）：未測，refactor-survey-spans 完成後合併驗。

## 驗收中發現的待修（2026-08-11 事故揭露，另行拍板排程）
驗收開跑即撞 claude CLI 401：根因＝`~/.claude/settings.json` env 被寫入 cliproxy 代理（已備份後移除兩鍵、4.6 秒探針復通）。來源 2026-08-11 查明並全清：7/13 CLIProxyAPI Connect 安裝流程寫入 settings.json＋`~/.zshrc`；7/29 的檔案時間係 handoff 技能安裝整檔重寫所致（proxy 鍵原樣搬運）；當日斷線＝proxy 內 Claude OAuth 11:21 過期後刷新失敗。`.zshrc` 區塊與 proxy 的 claude 憑證已一併清除，恢復訂閱直連。過程揭露三個 app 缺陷：
1. CLI 認證類錯誤慢速重試 ~3 分鐘才報錯，期間零回饋（玩家以為卡死）——快速失敗＋即時提示。
2. 設定頁「正在安裝／重新驗證」進行中按鈕沒鎖，可重複開驗證視窗。
3. 驗證腳本探針用 `claude -p "ok"`（慢＋燒額度＋認證壞時掛 3 分鐘）——換 `claude auth status` 類即時指令（須先驗它能否分辨憑證失效）。

## Next action
先做 [refactor-survey-spans](refactor-survey-spans.md)（新對話）；完成後回本案補驗 P4–P6／P8，過即結案（HANDOFF.md 那行刪掉、本檔搬 handoffs/archive/）。

## Constraints
- survey／expand 共用 system 逐位元組相同的快取紅線照舊，system 組裝零觸碰（本次已驗：knownFields 等階段差異只在 user 訊息）。
- 機制重寫品質是下放最敏感點：P7 不過就單獨升回 best，不整包回滾。
- knownFields 欄位命名單一權威不得犧牲（並行拓撲 A 已固定此語意）。
- 並行上限 4；實測穩定前不上調。
