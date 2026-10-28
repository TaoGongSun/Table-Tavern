> 結案 2026-10-28：Sol、opus-review 驗收 PASS，squash 進 main。

# greeting-context-st-parity — 開場白進上下文的方式對齊 ST

規格（拍板結論、已知限制、驗收）：[plans/greeting-context-st-parity.md](../../plans/greeting-context-st-parity.md)。三方共識（主線、Sol、opus-review 暫代 Grok）2026-10-28。

## 做在哪
- 政策：`chat_assembly/greeting.rs`（`policy`、`strip_card_greetings`）；`GmMaterials` 帶 `strip_greetings` 與 GM 送出層 `opening`，角色回合在入口算一次；卡清單、角色回合、回歸事件都走 `prompt_card(card, strip)`。
- GM 送出層：`transport/gm_opening.rs`（`GmOpening`、範例段）；`Side::GmOpeningRaw` 在 `arrivals::prompt_text` 讓開場事件送 `raw`，只由 `assemble_gm_messages_with` 與 lane `build_prompt_as`（`lanes/prompt.rs`，`TurnInput.opening_raw`）使用；世界書掃描、巨集、角色線照 `Side::Gm`／`Character`。
- 換幕往回找：`data::branch_opening`（lifecycle.rs，沿 `scene_label().parent`，原始位元組沒有開場旗標就不解析整幕）。
- 量測：`scene_budget/measure.rs` GM lane 走同一個 `lane_side`。
- 測試：`chat_assembly/greeting/tests.rs`（政策、往回找、範例不進掃描、API 逐字、接管／退場例外）、`lanes/tests/opening_raw.rs`（假 claude CLI 實送）、`scene_budget/tests/greeting.rs`（量測＝實送）。

## 實測（2026-10-28，scratchpad `greet/`）
- 卡有常駐格式條目時，舊（A）與新（N）Sonnet 4/4、codex 3/3 全同（含 4 欄）。
- 拿掉格式條目只剩開場示範：本幕有開場 A 4/4→N 4/4（codex 3/3→3/3）；換幕後首回合 A 0/4→N 4/4、codex 0/3→3/3（舊版卡在換幕時被自動隱藏，system 本來就沒開場白段）；世界書匯入桌 A 0/4→N 3/4、codex 0/3→3/3。換幕後狀態值都寫第 2 天，沒照抄範例的第 1 天。
- 掃描：NorthHall、hero-roster（MVU 卡）、clean-orc 新舊 GM 輸入的世界書段逐字相同，差異只有第 0 則原文。
- 不貼開場的桌新舊提示詞相同（測試證明政策回預設）。

## 已知限制
- promptOnly 剝除卡（NorthHall、MVU）會多送 ST 不送的區塊（後端套 promptOnly 另案 D）。
- 角色卡桌 system 變動 → lane 一次性重開，重開後舊 GM 回覆示範被壓成 text；世界書桌 system 不變不重開，舊 session 第 0 則停在 text。換幕本來就重開 GM lane（plan.rs SceneChanged），範例不多一次重開。
- 換幕往回找沿 parent（分岔當下所在幕）：從較早的非開場幕分岔、且當下所在線貼過不同開場白時，範例取到當下那條線的〔模型判斷·未裁決〕。
- Haiku 在無格式條目時三方案皆 0/5，效果未證實。
- 已寫入的回歸事件裡的開場白段收不回來。
