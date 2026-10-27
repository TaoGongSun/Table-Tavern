> 結案 2026-10-28：Sol、Grok 驗收 DONE，squash 進 main。

# card-field-overlaps-fixed-keys — 卡欄位與 place／present 同義，GM 只報固定鍵

分支 `card-field-overlaps-fixed-keys`。

## 問題
clean-orc characters 重構後，卡自帶的「当前环境」「驻留/交涉角色」搬進狀態樹，與 app 固定鍵 place／present 同義。flat-field-reporting 實測三回合（Haiku），GM 只在 ```state 圍欄寫 place／present，兩個卡欄位停在開場值；頂部狀態欄同時列「地點：（新）」與「当前环境：（舊）」，GM「目前狀態」也並列兩份。

## 根因
同一件事有兩個欄位、回報義務不對稱：`narrate_instruction`（transport/response.rs）要求圍欄每回合必報 time／place／present；`narration_update_note` 對其他欄位只要求「有變動時」另附更新區塊。重構依設計丟掉卡的「每回合重印狀態欄」規定（refactor_ai/prompt_common.rs 傳輸規定那段），卡欄位就只剩選報通道，模型報過 place 就當已說過。重構 AI 也不知道 app 有這三個固定鍵（同義禁令只管卡內欄位）。寫入通道本身是通的（圍欄同名鍵改道進樹、更新區塊都測過）。

## 範圍
- 只發生在收圍欄的桌（characters 重構、一般旁白開增量）。接管桌三鍵已清（takeover-fence-scene-keys），名冊欄位是唯一來源 → 屬 character-presence。
- TestCards 開場狀態欄為純文字、且有 table_key 認不得的同義鍵：clean-orc 一族（clean-orc-cave、orc-cave-copy）；其餘卡是 MVU／YAML 樹或用「地点」等已認得的鍵。

## 做法
方案 C（App 鏡像），三方共識，細節見 [plans/card-field-overlaps-fixed-keys.md](../../plans/card-field-overlaps-fixed-keys.md)。

## 真重構實測（2026-10-28，測試通道獨立 root、claude CLI claude-sonnet-5-5，正式目錄 hash 前後相同、設定副本不含金鑰且已刪）
- 第一輪（標記放在機制改寫提示）：產物沒有任何 `same_as`。characters 模式把狀態欄格式段零呼叫淘汰（rule 5），機制合組看不到兩欄定義 → 標記改到盤點。
- 第二輪（盤點 MIRRORS）：clean-orc 匯成角色卡、貼開場白 2、重構（characters）。
  - 盤點帶出標記：`## MIRRORS` 只有 `当前环境 = place`、`驻留/交涉角色 = present`，沒有誤標其他欄（`剧情阶段/要塞事件`、`驻守天数` 沒標）；匯出的產物 `mirrors` 同上。
  - 套用：「全部套用」成功（這次沒撞驻守天数衝突），`mechanism.mirrors` 收進兩欄、兩欄的規則被拿掉、平欄清空。
  - 多輪鏡像：兩回合 GM 只在圍欄報 place／present，兩欄跟著變（食堂／玩家 → 指挥塔作戰室／玩家、托伦）。GM 同時照開場白樣式重印了卡的 `<details>` 狀態欄（重構產物已淘汰該格式段，推測是模仿開場白），其中兩欄的不同寫法被固定鍵蓋回。

## 另案
- GM 照開場白重印已淘汰的 `<details>` 狀態欄 → gm-reprints-retired-statusbar。
- 兩條機制條目同寫 `驻守天数` 規則擋下整次套用（第二次實測未重現）→ refactor-rule-conflict-same-field。
- 名冊欄位接在場機制（接管桌）→ character-presence。
