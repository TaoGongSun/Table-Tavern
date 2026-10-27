> 結案 2026-10-28：Sol、Grok 驗收 DONE，squash 進 main。

# gm-reprints-retired-statusbar — 重構淘汰狀態欄後，GM 仍照卡重印 <details> 狀態欄

## 根因
不是逐字稿裡的開場白（存檔時 `<details>` 已剝進 `raw`）。`import/card.rs` 把 first_mes 放公開設定「### 開場白」、全部 alternate_greetings 放私設「### 備用開場白 n」，每回合原樣送進 GM system；clean-orc 三份 `<details>` 範例被照抄。重印的 `<details>` 還被當狀態收（絕對值拒收、鏡像欄先寫錯再被蓋回）。

## 做法（三方共識，主線裁決）
- 標記：characters 重構實際淘汰狀態欄格式（非 playable 介面條目、statusbar 段，`RefactorDroppedEntry.statusbar`）→ 套用寫 `WorldState::statusbar_retired`，撤銷由收據還原。
- 提示詞：這類桌送進 GM、角色線、登場事件的卡以 `import::strip_greetings` 依段頭拿掉兩種開場白段；卡檔、匯出不動。選中的開場白已以逐字稿第 0 則進上下文。
- 回覆端 `BlockScope::RetiredCardBlocks`：卡片狀態區塊（`<details>` 狀態欄、`<status…>`）不收並回饋，圍欄與 `<UpdateVariable>` 照收（`StateBlock.fenced` 只記圍欄來源）。
- 沒重構桌、接管桌不動。

## 實測（2026-10-28）
- 重播第一回合（Sonnet n=4、codex n=3）：原樣 4/4、3/3 重印；剝開場白段後 0/4、0/3。
- 測試通道真重構 clean-orc：Sonnet 新 lane 4 回合 0 重印、codex（gpt-5.6-terra）2 回合 0 重印；跳到第 7 天時剧情阶段經 UpdateVariable 更新；沒重構桌 Sonnet 2/2 照舊重印；無繁體重複鍵。
- 既有 lane：系統提示一變 lane 自動開新 session，舊 session 裡的原始重印不帶過去。

## 已知限制
- 本案之前已套用的重構桌沒有標記，要重新重構套用才退場。
- 回饋句只有中文（同接管桌狀態區塊回饋）。

## 另案
- 歷史裡「（系統）狀態更新」事件被 GM 照抄進正文 → state-update-echo-in-story。
- 沒重構桌開場白進上下文對齊 ST → greeting-context-st-parity；舞台卡進點名名單 → stage-card-in-speaker-list。
