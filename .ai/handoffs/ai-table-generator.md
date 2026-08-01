# Handoff: ai-table-generator

## Current state
2026-08-02 開工（目標模式：主線指揮＋外包寫碼）。塊 1 後端已發包 codex（gpt-5.6-terra），實作中。

## Completed
（尚無——本檔隨各塊驗收即時更新）

## Verification
（各塊驗收後補：cargo test／npm build／check:i18n／實跑全流程）

## Remaining / Next action
1. 塊 1 後端：genesis.rs（outline/expand 提示組裝＋容錯解析＋materialize 落桌）＋lib.rs 兩指令 generate_table_outline／generate_table_expand——codex 實作中
2. 塊 2 前端：側欄「開新桌」下方加生成按鈕＋modal 生成視窗（輸入框＋題材 chips＋大綱預覽＋重骰＋開桌＋額度小字）——等塊 1 驗收後發包
3. 塊 3 十語系：塊 2 先填 zh-TW＋en，其餘八語系暫填英文佔位過 check:i18n，塊 3 機械檔補真翻譯
4. 主線終驗：實跑「輸入兩句→大綱→重骰→開桌→進桌看開場白」全流程

## Constraints
- 規格全文見 tasks/ai-table-generator.md（標記文字大綱、角色數模型自判不錨定數字、解析失敗不留半套桌、額度明示、免費功能）。
- 提示詞已由主線定稿並隨塊 1 發包（英文標記 ## WORLD / ## CHARACTER / ## OPENING，內容跟介面語言走）。
- 落桌走 create_sample_world 同路徑；開場白事件形狀照抄該函式。
