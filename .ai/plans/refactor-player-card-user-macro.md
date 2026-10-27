# refactor-player-card-user-macro — 做法（已結案）

## 重現
暫時測試（未提交）：產物角色名 `{{user}}`、勾成玩家卡套用 → 落檔 `card.name == "{{user}}"`、`player_card_id` 指向它；
後端注入 `replace_st_macros(public_md, card.name)` 得到原樣 `{{user}}`。clean-orc 第 10 條標題 `{{USER}}`、內文
`Name: {{user}}`，盤點照抄成人名。本輪未叫模型，花費 0。

## 根因
本 app 沒有獨立的玩家名設定，`{{user}}` 的值就是玩家卡 name；name 本身是巨集就自我代換。
- `src-tauri/src/refactor_assemble.rs:537`（clean 組卡）與前端人物展開路徑：人名照抄盤點的 `{{user}}`。
- `src-tauri/src/refactor/apply.rs:273`：`name: character.name.clone()` 原樣寫進角色卡；`:288-289` 設成玩家卡。
- 下游三處都拿 `player.name` 當 user_name：`commands/chat.rs:517,602`（提示詞注入、狀態套用）、
  `transport/turns.rs:247,387`（各 lane 注入）、`commands/chat.rs:723`（`next_speaker` 比對說話者）。
  所以不只畫面，模型收到的 `{{user}}` 也全是字面。

## 做法（Opus、Sol、Grok 三方共識：送審第 1 輪同意，驗收第 2 輪 DONE）
- **套用建卡時解析，不在顯示時替換**：顯示替換修不了上面三處後端用途，而且 name 是巨集時沒有東西可換。
- **不改產物（outcome）**：`apply.rs` 把 outcome 原樣寫進 refactor-outcome.json，匯出卡與重構卡匯入都吃這份；
  留 `{{user}}` 才能在別桌、別語系各自解析。重構卡匯入也走 `apply_with_assets → apply_into`，一處兩路都蓋。
- **卡沒給名字時玩家卡先用語系預設名，不加「套用時問名字」**〔作者裁決 2026-10-27〕。預設名沿用 app 既有的
  `player_fallback_name`（`data/scene/marker.rs:33`），與無玩家卡時的注入、點名、匯出同一套字。
- **玩家名先定、再統一解析**：先算本次指定玩家卡的最終名稱（它自己的巨集換成預設名）；沒指定就用桌上既有玩家卡名
  （`apply.rs` 允許已有玩家時只新增 NPC）；兩者皆無才用預設名。所有新卡名與沒勾成卡的恆定人物條目標題都用這個
  玩家名過 `replace_st_macros`（本次玩家卡自己直接用算好的玩家名，不再代換第二次）（大小寫不分，與後端注入同一支），結果與角色排列順序無關。例：玩家卡叫「阿明」時
  `{{user}}的妹妹` 存成「阿明的妹妹」。
- lang 由 `refactor_apply` 命令讀 config＋`ui_language`，經 `apply_to_table → apply_and_record → apply_with_assets
  → apply_into` 傳下去；cfg(test) 的 `apply` 固定 zh-TW。
- 改名：預設名只是初始名；之後玩家改名，注入、StateBar 都即時讀 player.name，自動跟上。卡內文 `Name: {{user}}`
  保持原文，跟著現名代換。

## 範圍與已知限制
- 只保證無內部空白的 `{{user}}`（大小寫不分）；`{{ user }}` 前端 StateBar 容許、後端不容許的差異不在本案〔模型判斷·未裁決〕。
- 名字只有部分是巨集（如 `{{user}}的妹妹`）在套用當下固定，之後玩家改名不跟。
- 變數模式啟用時存的 `{{user}}` 代換值（`data/message_vars/control.rs:32`）改名不回溯——既有行為，本案不動。
- 已套用的舊桌玩家卡仍叫 `{{user}}`（發佈前不顧舊桌），玩家可手動改名。
- `RefactorResultDialog.tsx` 套用前預覽仍顯示 `{{user}}`（動畫面，要另拍現況截圖再議）〔模型判斷·未裁決〕。

## 改動
後端 `refactor/apply.rs`（`resolved_names`）、`apply_record.rs`、`reset.rs`、`commands/refactor.rs` 加 lang 參數；
回歸測試 `refactor/tests/user_macro_names.rs`。不碰 StateBar.tsx／AppWorkspace.tsx／card-interface controller／mvu shim。

## 驗收
`user_macro_names` 七支：`{{USER}}` 混大小寫→預設名、正文與產物檔留巨集、落檔後注入正文換成實名；en→Player；
同批具名玩家＋NPC 排前、普通名不變、`{{char}}` 原樣、改名後 NPC 名固定；既有玩家＋只新增 NPC；沒勾成卡的
人物條目標題（具名玩家與預設名兩例）；部分巨集玩家名（`{{user}}的化身`）＋NPC 排前，玩家名只展開一次、NPC 用同一個
最終名；匯出重構卡留巨集，在 en 桌走 open_card 匯入套用，玩家卡名 Player。`npm run verify` 綠。

## 業界做法與差距
SillyTavern 的 `{{user}}` 取自使用者自己設定的 Persona 名稱，巨集只在組提示詞時代換內容，描述玩家的世界書條目
不會升格成角色卡。本 app 把玩家卡 name 當玩家名，所以落檔名不能留巨集；卡沒給名字時沿用 app 既有的
`player_fallback_name`。差距是玩家沒在套用當下被問名字（作者裁決不做）。
