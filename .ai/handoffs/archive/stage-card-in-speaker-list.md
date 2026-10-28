> 結案 2026-10-28：三包進 main，Sol、opus-review 驗收 PASS、三方共識。

# stage-card-in-speaker-list — 舞台卡（提供設定、不參與角色發言）

規格（拍板、效果、分包、驗收、已知限制）見 [plans/stage-card-in-speaker-list.md](../../plans/stage-card-in-speaker-list.md)。

## 現況
- 三包（後端核心、前端、重構自動判定）皆已進 main。

## 包 1、2 已定的結論（程式現況）
- 旗標：卡檔 frontmatter `stage: true`（只在開啟時寫）；`CharacterMeta.stage`、`CharacterCard.stage`（`skip_deserializing`：前端看得到、送回不收）；只能經 `data::set_character_stage`／指令 `set_character_stage` 改，開啟時清 `auto_hidden`、關閉不動。
- 分流唯一入口：後端 `CharacterMeta::cast_role()`（Speaker／Stage／Hidden／Archived）與 `chat_assembly::speakers`／`speaker_roster`；前端 `characters.speaking`、`firstSpeakerCandidate`、`effectiveSpeaker`（導覽 controller 對外的 `speaker` 已排除舞台，畫面與送出共用）。
- 舞台卡進提示詞但放「## 舞台」段（GM 含私設、角色線只公開）；不進點名名單、`pick_speaker` 候選、發言對象；角色回合後端拒收（`stage_card_no_turn`）；換幕結算與退回／分岔還原都跳過；同名狀態分支永遠送；「1 角色＋1 舞台」算單角色桌。
- 「讓 X 接話」候選是發言名單，`callAndReply` 寫點名前查最新 stage。
- 手動開關落在角色卡編輯頁 ⋯ 選單、「隱藏角色」正上方（「設為舞台卡／改回角色」）〔作者裁決 2026-10-28〕。側欄舞台卡帶「舞台」標籤、不可選為發言對象。
- 收尾句已改明指 ```state 圍欄。

## 包 3 程式現況（細節見計畫檔 A1）
- 盤點：`refactor_ai/survey.rs` characters 模式段要求 `## STAGE`（一行一張既有角色卡名），`survey_parse.rs` 解析、interface 清掉；`commands/refactor.rs` 收結果時用 `refactor/stage.rs` 濾成本桌對得到的名字。
- 套用：`refactor/stage.rs`（名字唯一對、只收可發言角色、寫入前快照、只認 characters 玩法）→ `apply.rs` 在存重構卡前寫旗標，前後值進 `ApplyProgress.stage_changes`。
- 撤銷：`receipts.rs` 收據 `stage_restores`，`data::restore_stage_flags` 兩旗標都等於套用後值才退回。
- 開重構卡：清回原卡之後用指令 `refactor_stage_cards` 照這一桌過濾舞台卡名（`useRefactorWorkflow`）。
- 前端：`RefactorResultDialog` 角色區下方「設為舞台卡」區＋摘要列卡名（>3 改張數），`refactor-review.ts` 判空含舞台；套用後發言對象照 App 既有規則撥回 GM（`refactor-stage-speaker.test.tsx`）。

## 留意
- 包 2 的觀察（選中的卡被標成舞台後畫面顯示「沒有對象」）只發生在編輯頁手動切換，維持現狀；重構套用那條由 App `refreshAfterRefactorApplied` 撥回 GM 涵蓋〔模型判斷·未裁決〕。
- 已知限制（計畫檔有）：匯出／再匯入與網頁版不帶舞台旗標。
- `commands/chat.rs`（約 1600 行）、`App.tsx`（約 1065 行）、`receipts.rs`（約 1220 行）本來就超過 1000 行，未拆。
