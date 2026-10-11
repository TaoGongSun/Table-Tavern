# 重構初判失敗不留痕：方案

交接檔：[refactor-probe-failure-hidden](../handoffs/archive/refactor-probe-failure-hidden.md)。

## 根因

- `src/features/refactor/useRefactorWorkflow.ts:295-300`（`runAiRefactor` 初判 catch）：除 `refactor-aborted` 外**所有**錯誤一律落到「無建議、展開、預選介面」的二選一，`reason` 丟掉不顯示。
- 後端 `refactor_recommend`（`src-tauri/src/commands/refactor.rs:57-118`）的失敗分三種，前端全混成一種：
  1. **沒送出**：更新閘門、缺 `cli_risk_accepted`、找不到 CLI、agy 太舊（`transport/dispatch.rs:144-163`、`:447-466`）、檔位沒設模型（`transport/client.rs:75` `resolve_model`）、缺 OpenRouter key（`client.rs:521` 擋下，`dispatch.rs:424` 包成 `AI_CALL_FAILED:`）、智慧免費沒有可用模型與今日額度用完（`smart_free/mod.rs:154-170`）。未知 transport 實際先被 `:449` 的 `cli_not_found` 擋下（`:636` 那條會被 `:642` 包殼）。
  2. **呼叫失敗**：送出後失敗。API 路經 `ai_call_failure`（`dispatch.rs:199`）帶 `AI_*:` 碼；claude lane 的 `refactor_ai/session.rs:20-28` `open_stage` **沒包**，`cli_reply_error`／`cli_crashed`／`cli_no_reply` 等以裸 `TTMSG:` 回來，跟第 1 類長得一樣。
  3. **回話解析不出建議**：`refactor-recommend-unparsable`。

順查同函式其他派送前失敗：
- `useRefactorWorkflow.ts:257-259`：`card_interfaces` 失敗被 `.catch(() => [])` 吞成「無介面卡」，三態判成 `none`，**靜默直跑角色線**。
- `useRefactorWorkflow.ts:205`（`checkRerun`）：`refactor_rerun_status` 失敗沒 catch，按鈕按了沒反應；匯入重構卡路徑（`:663`）也呼叫它。
- 第二段 `startRefactorRun`、單條展開、套用都有把原因送到狀態列或失敗清單，不在本案範圍。

## 拍板

沒送出的初判失敗不開二選一、狀態列直接報原因；送出後失敗／解析不出照舊開二選一（不偽造證據、預選介面）並附原因。〔作者裁決 2026-10-12〕

## 修法

1. **後端**：`session.rs` `open_stage` 的 `run_stage(...).await` 補 `.map_err(ai_call_failure)`（比照 `commands/chat.rs:228`），讓 claude lane 送出後的失敗都帶 `AI_CALL_FAILED:`（已帶碼的原樣放行）；前面的 `refuse_if_updating` 不包。`resume_stage` 失敗本來就降級重送，不動。
2. **分類**：`refactor-mode.ts` 新增 `probeFailureKind(reason)`，回 `aborted | not_sent | failed | unparsable`：
   - `refactor-aborted` → `aborted`；`refactor-recommend-unparsable` → `unparsable`。
   - 正向白名單：`backendCode(reason)`（`shared/ui/backend-text.ts`）落在 `cli_risk_not_accepted`、`cli_not_found`、`agy_too_old`、`unknown_transport`（保留裸碼無害）、`update_gate_closed`、`tier_model_missing`、`no_free_models`、`no_stable_free_model` → `not_sent`。
   - 剝殼特例兩條：`openrouter_api_key_missing`（裸碼或 `AI_CALL_FAILED:` 殼，沿用 `ai-error.ts` 既有 `isOpenRouterKeyMissing`，改 export）；`AI_HTTP_STATUS_429:` 殼內只認 `smart_free_daily_exhausted`。殼內非白名單錯誤一律走開二選一（不宣稱已送出）。
   - sentinel 去掉外層 `Error: ` 後精確比對，不用 `includes`。
   - 其餘一律（含認不出的）→ `failed`。
3. **初判 catch**（`useRefactorWorkflow.ts:295`），先檢查 `stale()`：
   - `aborted` → 靜默；`not_sent` → 剝掉開頭 `(Error: )?(AI_CALL_FAILED|AI_HTTP_STATUS_\d{3}):` 前綴後 `setStatusMessage`（狀態列只剩翻譯後人話），不開二選一。
   - `failed`／`unparsable` → 開二選一，`modeAsk` 新增 `failure: { kind: "failed" | "unparsable"; raw: string } | null`。初判成功開框前同樣先檢查 `stale()`。
4. **二選一框**（`RefactorRunDialogs.tsx:81`，判官句位置；`recommend === null && failure` 時）：
   - `failed`：比照 `TurnFailedDialog.tsx:64`，`<p>{t("refactorProbeFailed")}<br /><AiErrorText text={failure.raw} transport={transport} /></p>`；`transport` 由 `AppWorkspace` 既有值經 `MainView`→`WorldEditor`→`RefactorRunDialogs` 傳下，一路選填（不傳時中性文案）。
   - `unparsable`：只顯示 `t("refactorProbeUnparsable")`，不露內部代碼。
5. **`card_interfaces` 失敗**（`:257`）：拿掉 `.catch(() => [])`，成功或失敗都先檢查 `stale()`；失敗 `setStatusMessage(String(reason))` 結束。
6. **`checkRerun` 失敗**：在函式內 catch，`stale()` 為假才 `setStatusMessage(String(reason))`，回 `false`。`runAiRefactor` 的清狀態列提到 `checkRerun` 之前（匯入重構卡本來就先清）；兩個呼叫端拿到 `false` 直接 return，外層 catch 不會再報，只報一次。
7. i18n：`refactorProbeFailed`（「初判沒成功，請自己選玩法」）、`refactorProbeUnparsable`（「初判回覆看不懂，請自己選玩法」），十國語言。

## 測試

Rust（`refactor_ai/session.rs`，`fake_cli` 加 `FAKE_FAIL`、`FAKE_RUNAWAY` 改成開線也生效）：
- CLI 非零碼結束 → `open_stage` 錯誤以 `AI_CALL_FAILED:` 開頭；失控 → 保持 `AI_OUTPUT_RUNAWAY:` 不包殼。

前端 `refactor-mode.test.ts`（`probeFailureKind`）：
- `not_sent`：八個白名單碼帶齊必要參數（含 `Error: ` 包裝）、`openrouter_api_key_missing` 裸碼／`AI_CALL_FAILED:` 殼／`Error: AI_CALL_FAILED:` 殼、`AI_HTTP_STATUS_429: TTMSG:{smart_free_daily_exhausted}`。
- `failed`：`AI_CALL_FAILED: …`、`AI_HTTP_STATUS_429: …`、`AI_EMPTY_RESPONSE: …`、`AI_CALL_FAILED: TTMSG:{cli_reply_error}`（claude lane 包裝後的長相）、`AI_CALL_FAILED: TTMSG:{cli_not_found}`（殼內非 openrouter 碼不剝）、未知 `TTMSG` 碼、純文字。
- `unparsable`、`aborted` 各一。

前端 `refactor-workflow.test.tsx`（harness 的 `setStatusMessage` 改成記錄）：
- 初判回 `cli_risk_not_accepted`：不開 `modeAsk`、狀態列收到原文、`refactor_survey` 沒被呼叫。
- 初判回 `AI_HTTP_STATUS_429:`、`AI_CALL_FAILED` 包 `cli_reply_error`、未知碼：開二選一、`recommend === null`、`expanded`、預選介面、`failure.kind === "failed"`。
- 初判回 unparsable：開二選一、`failure.kind === "unparsable"`。
- 初判成功／送出後失敗／沒送出時已換桌：不開框、不寫狀態列。
- `card_interfaces` 丟錯：狀態列收到原文、`refactor_recommend`／`refactor_survey` 都沒被呼叫；換桌後丟錯不報。
- `refactor_rerun_status` 丟錯：`runAiRefactor` 與匯入重構卡兩條路徑各自狀態列只收到一次原文、不往下走；換桌後丟錯不報。

前端 `RefactorRunDialogs` 渲染：`failed` 顯示失敗句與 `AiErrorText`（429 走額度人話）；`unparsable` 只顯示那一句、沒有代碼字樣；兩者都不顯判官句。

實機（測試通道）：設定拿掉 `cli_risk_accepted`、對 supported 卡按重構，狀態列出現「尚未確認 CLI 訂閱模式的風險告知…」，不出二選一。

## 範圍外／已知限制〔模型判斷·未裁決〕

- 白名單外的送出前失敗（例如 `cli_workspace` 建資料夾失敗；claude lane `session.rs` 的 `PromptFile::create`、CLI spawn 失敗被包殼）照舊開二選一並附原因，罕見，不另分類。
- `import/interface.rs:60` 讀原卡失敗直接跳過（可能落 `none`），是介面面板既有契約，本案不改。
- `refactor_ai/context.rs:40` 略過讀不到的角色。
- `cancelAiRefactor` 的 `void invoke` 沒有錯誤處理。
- 狀態列改用 `AiErrorText`：牽動世界書區所有非 AI 訊息，另案再議。
