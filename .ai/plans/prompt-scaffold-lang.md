# prompt-scaffold-lang 計畫

結論：已結案進 main（2026-10-03，Sol 審查與驗收同意）。Sol 三項非阻擋建議不做〔作者裁決 2026-10-03〕。

拍板〔作者裁決 2026-10-03〕：送 AI 的固定骨架 zh*（zh-TW、zh-CN、其他 zh 開頭）送繁中，其餘全部送英文；輸出語言仍由 `language_rule(lang)`（各語系母語書寫）決定。

## 共用判斷
`data/scene/marker.rs` `prompt_lang` 改為 `zh* → "zh-TW"、其餘 → "en"`；`transport/messages.rs` `scaffold_en(lang)` 包它，所有送 AI 的語系分岔（含 state_view 欄位名的 tuple match）都走它。不在函式開頭把 `lang` 覆寫成骨架語系：`language_rule`、`player_fallback_name` 一律吃原始 `lang`（點名玩家無名時的稱呼也照原始語系補）。`lang_key`、匯出／匯入段標、範例桌、`leftover_title_suffix` 不動。
英文骨架的「名字：」前綴、旁白／系統行、狀態欄冒號與變動標記括號一律半形；`transport::speaker_prefix` 同時供事件行、共線歷史、`cli::flatten_messages` 的 assistant 前綴與 commands/chat.rs lane 回覆補前綴使用，同一條 session 的歷史逐字銜接。

## 盤點

### A. 既有 `"en"` 二選一分岔（全部是送 AI，全部改走 `prompt_lang`）
| 位置 | 內容 |
|---|---|
| transport/response.rs:38、55 | `narrate_instruction` 導演指示＋`Next:` 點名 |
| transport/response.rs:115、130 | `takeover_instruction` 導演指示＋點名 |
| transport/response.rs:152 | `card_format_instruction` |
| transport/context.rs:129 | `mechanism_protocol` 狀態更新協定 |
| transport/context.rs:188、209 | `table_update_example` 佔位值與標題 |
| transport/context.rs:224 | `interface_owned_notice` |
| transport/turns.rs:270 | `summary_messages` 摘要指令 |
| transport/state_view.rs:60-62 | 狀態欄位名 Time／Place／Present |
| data/scene/marker.rs:50 | `prompt_lang` 本體（逐字稿事件標頭經 `transport/arrivals.rs:122` `prompt_text`） |

非送 AI、不動：`openrouter_oauth.rs`、`data/config.rs`、`data/world.rs`（範例桌）、`import/card.rs`、`data/scene/export.rs`、`translate.rs:24` 註解。

注意：`summary_messages` 英文版寫死 “in English”，改成接 `language_rule(lang)`，否則 ja 等會收到「摘要用英文」。

### B1. 只有中文、本案補英文版（遊玩主路徑）
英文版照現有中文逐段忠實翻成精簡自然的英文，不另做優化；所有新分岔都走 `prompt_lang`。
- transport/context.rs:44-94 `gm_system_prompt` 開頭身分說明與段標（世界設定、世界書、登場角色、公開／私有設定、玩家角色）；context.rs:268 `split_person_roster`（加 `lang` 參數）
- transport/state_view.rs:25、53、55、84、98 動態區段標；state_view.rs:68、151-163 全形「：」「（）」英文版改半形；state_view.rs:350 `character_state_block`（加 `lang`）
- transport/turns.rs:35-38 `lane_event_line`（旁白）（系統）前綴；turns.rs:56-98 `chars_lane_system`；turns.rs:154-198 `chars_lane_turn` 段標與本輪指示
- transport/assemble.rs:65-68、122-124 逐字稿行前綴
- commands/chat.rs:267-284 GM 收尾句；commands/scene.rs:255、330 摘要收尾句
- cli/request.rs:34 `flatten_messages`「以下是到目前為止的對話紀錄」（呼叫端 transport/dispatch.rs:349 以 `ui_language(config)` 傳入）；lanes/mod.rs:408 同句
- lanes/snapshot_patch.rs:26-49 設定更新段（呼叫端 lanes/mod.rs:372 傳 `input.lang`）

### 不在本案
- 機制拒收說明與變動標記（mechanism/apply.rs，套用時以中文存進 `state.notes`／changes）：不做也不立案，出問題再說〔作者裁決 2026-10-03〕
- refactor_ai/ 全部骨架、transport/translate.rs 開場白翻譯 system：不做也不立案，出問題再說〔作者裁決 2026-10-03〕
- lanes/mod.rs:797 `PING_PROMPT` 維持中文（理由見下）〔模型判斷·未裁決〕
- 已是英文骨架、不動：genesis.rs、commands/refactor.rs／scene.rs:101 CLI 收尾、image.rs

## 快取與凍結快照
- zh*（含 zh-CN）骨架逐字不變（基準檔守住）；en 的 A 類除摘要語言句外逐字不變，B1 由中文改英文。
- en 與其他非中文語系的既有桌：GM／角色線的凍結 system 改變後，Claude 快取有效時走補丁（`render_patch`，英文版）、過期時換新 system 續聊（rebase）；Grok／Agy 因 system 改變重開（`SystemChanged`）；API 路徑是前綴快取失效一次。之後同語系骨架逐字穩定。
- 已送段指紋（`events_fingerprint`）只算原事件、不算 `lane_event_line`／`build_prompt` 的渲染前綴，前綴改字不觸發重開。不另加骨架版本判斷；發佈前不保舊桌。
- `PING_PROMPT` 維持中文：維持現有定位契約、避免無關改動〔模型判斷·未裁決〕。

## 解析
回覆解析兩套都認，不需改：`extract_next_speaker`（下一位／next）、`extract_scene_title`（標題：／Title:）、state 圍欄鍵別名。

## 測試清單
- 基準：`src-tauri/src/lanes/scaffold_tests.rs` 把 zh-TW／zh-CN／zh-HK 的 GM／角色 system、回合尾動態塊、共線與 GM 組裝、摘要、導演指示、收尾句、事件行、CLI 攤平、開線 prompt、設定補丁全部攤開，與改動前產出的 `lanes/scaffold_baseline/<lang>.txt` 逐字比對。
- `prompt_lang`：zh-TW、zh-CN、zh-HK → zh-TW；en、ja、ko、es、pt-BR、de、fr、ru、未知 → en。
- ko／es／pt-BR／de／fr／ru 的整份輸出（語言規則句與玩家稱呼換佔位後）與 ja 相同；不含任何中文骨架字；未知語系與 en 相同；不含 “in English”。
- ja／fr／ru 的 GM、角色 system 與摘要指令帶該語系母語規則；摘要保留 `Title:` 契約。
- 逐字稿標頭：zh-CN 繁中、ja／ru／fr 同 en；點名無名玩家照原始語系稱呼。
- 英文共線：換角色除尾端外逐字相同；單角色私設上提進 system；機密段整段在 tail、抹除後不留私設與限定條目。
- 英文設定補丁：新增、刪除、重複標題、開頭指示。
- lane 升級：舊（繁中）→新（英文）骨架，Claude 快取有效走補丁、過期 rebase，Grok／Agy 重開，追上後第二輪續聊無補丁。
- 解析：ja／fr／ru 回覆含 state 圍欄＋`Next: __PLAYER__`、母語幕名＋`Title:` 都解析得到；既有 `下一位：`／`標題：` 測試保留。
