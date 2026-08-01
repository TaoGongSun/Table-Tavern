# Handoff: ai-table-generator

## Current state
2026-08-02 目標模式進行中。塊 1 後端完成並驗收；塊 2 前端發包中。

## Completed
- 塊 1 後端（codex gpt-5.6-terra 實作、主線審過）：genesis.rs 新檔——outline／expand 提示組裝（角色數零數字錨定）、容錯解析（標記 0–6 個 #、大小寫不拘、半全形冒號、缺 EMOJI→🎭、缺 PRIVATE→空、缺 OPENING 照樣成桌）、materialize 落桌（先解析成功才動磁碟；重名補「 2」「 3」；六色輪配；開場白事件形狀同 create_sample_world data.rs:422）。lib.rs 兩指令 generate_table_outline／generate_table_expand（src-tauri/src/lib.rs:1275、1310）已註冊，回傳 camelCase：`{parsed, raw}`／`{worldId, raw}`，解析失敗＝null＋raw 原文，Err 只留 API 錯誤。
- codex 順手 cargo fmt 掃到五個範圍外檔（cli/data/import/install/transport），純排版零語意，已全數退回。

## Verification
- 主線實跑 `cargo test`：133 passed; 0 failed（127 既有＋6 新增，見 genesis.rs:351-427）；`cargo check` 0 warning。codex 沙盒回報的 3 紅確認是 loopback 禁令誤傷，本機全綠。
- 主線逐行審 genesis.rs：提示詞與拍板規格逐字一致；materialize 動磁碟前必先解析成功。

## Remaining / Next action
1. 塊 2 前端（發包中）：側欄「開新桌」下方生成按鈕＋modal 生成視窗（輸入框＋題材 chips＋大綱預覽＋重骰＋開桌＋額度小字）；zh-TW＋en 字串、其餘八語系英文佔位
2. 塊 3 十語系：機械檔補八語系真翻譯
3. 主線終驗：實跑「輸入兩句→大綱→重骰→開桌→進桌看開場白」全流程

## Constraints
- 規格全文見 tasks/ai-table-generator.md（標記文字大綱、角色數模型自判不錨定數字、解析失敗不留半套桌、額度明示、免費功能）。
- 提示詞已由主線定稿並隨塊 1 發包（英文標記 ## WORLD / ## CHARACTER / ## OPENING，內容跟介面語言走）。
- 落桌走 create_sample_world 同路徑；開場白事件形狀照抄該函式。
