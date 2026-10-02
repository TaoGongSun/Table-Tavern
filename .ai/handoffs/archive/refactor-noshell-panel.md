# refactor-noshell-panel — 重構沒產殼的桌不給卡片介面

## 現象
2026-10-02 跑五卡矩陣（[refactor-mode-split](../refactor-mode-split.md)）時，bcd368、NorthHall-structure 重構判 playable: no、沒建殼，但桌子 refactor_mode=interface，卡片介面鈕照樣出現。面板退回原卡 HTML：空桌停在「加载中...」，跑一回合後按鈕消失。NorthHall 型卡的頁面底部還多出字面 ```。

## 裁決
重構判定不接管介面（沒有重構殼）時不給面板，狀態看頂部狀態欄〔作者裁決 2026-10-02〕。理由：原卡 HTML 靠 TavernHelper 的 getChatMessages／getCurrentMessageId 讀訊息，app 沒提供，畫不出值。大改前建立的桌不必相容〔作者裁決 2026-10-02〕。

## 狀態（已結案）
- 已進 main。做法、接受的限制與測試清單見 [plans/refactor-noshell-panel.md](../../plans/refactor-noshell-panel.md)；Sol 審查兩輪、驗收通過。
- `npm run verify` 全綠：vitest 460、cargo 731（本案 Rust 14 案）、harness 28。
- GUI 補驗（test-harness，零 AI 派送，正式目錄 hash 不變）：
  - WestFantsy 桌套用 wf1 產物後介面鈕出現、面板顯示選角開場。
  - 面板狀態開著時套用 nhs（沒殼）產物：面板與介面鈕消失，沒有字面 ```。
  - 按復原後介面鈕回來，面板不自己跳開，頂部狀態欄換回 wf1 的樹。
  - 沒重構的 NorthHall-structure 桌，卡片介面底部不再有字面 ```。
- 沒重構過的桌（mode=null）依裁決維持現狀：NorthHall 型卡打開卡片介面仍停在「加载中...」。
