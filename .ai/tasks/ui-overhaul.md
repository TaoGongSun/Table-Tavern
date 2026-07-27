# Task
Task-ID: ui-overhaul
Title: UI 全面改版：Emblem 設計系統（桌遊組件卡＋playbill 對話＋token 化）
Status: in_progress
Created: 2026-07-25T00:30:00+08:00
Updated: 2026-07-25T01:20:00+08:00

## Summary
英文合作者（computerhead13）主導的 UI 改版：先做三面向對抗式設計檢討（可讀性／美感／獨特性），再經五輪候選稿收斂成「Emblem」設計系統，已拍板定案並完成第一輪實作（App.css 全面 token 化重寫＋App.tsx 結構調整＋en 按鈕文案 title case）。

設計規則（一頁講完，token 都在 App.css 頂部）：
- **wedge（斜切名牌）＝名字**：角色卡名牌、對話發言名牌、桌名（中性色）三處同一塊組件；全 app 只有「名字」戴 wedge。
- **虛線琥珀＝機密**：資訊邊界（世界書可見範圍徽章）統一虛線琥珀記號；琥珀色只做這件事。
- **角色色固定五處**：卡圖窗染底、名牌、檔位寶石、發言名牌、composer 目標晶片。
- **serif＝故事文字**（對話、旁白、輸入框——打字所見即故事字樣）；**sans＝系統文字**。
- **控制項一律同高**（--control-h）；每畫面同時間只有一顆實色主要鈕（submit）。
- 深色 token 全過 WCAG AA（正文 AAA）；淺色是同結構石板紙色系。

設計過程紀錄（claude.ai artifacts，英文）：
- 檢討報告：https://claude.ai/code/artifact/3cff3ad8-ab20-4e62-8deb-a93b45ea1ce0
- 定案設計稿：https://claude.ai/code/artifact/a1efda5d-a8fd-427b-a67c-cbf11faa55f7

實作原則（使用者拍板）：只重現實際存在的系統功能，不發明 UI——設計稿裡的「Say 模式選單」「訊息內機密徽章」「角色 epithet 副標」皆確認非既有功能而剔除；檔位寶石（tier 是既有欄位）、目標晶片（既有「點側欄選發言對象」狀態的可見化）、幕書籤（既有換幕／前幕結構）皆對得上系統。

## Next action
- 使用者（英文合作者）實機驗收深色模式（系統切深色主題看 token 對應）與長對話 playbill 呈現（需 API key 實聊才有 dialogue 事件）
- 驗收後評估殘留項：舊 .container／first-run 畫面樣式僅粗略 token 化；zh-TW 按鈕文案無 casing 問題不動
- 淺色主題調色已由使用者 2026-07-27 逐輪驗收定案（去黃→中性灰，GM 書皮改石板灰帶一點暖），詳見交接檔

## Constraints
- 不發明 UI：任何新視覺元素都要對應既有系統功能或狀態，實作前先對 App.tsx／後端指令查證。
- serif 只給故事文字；琥珀只給資訊邊界；wedge 只給名字——三條規則是這套系統的骨架，改版時不得稀釋。
- en 文案按鈕一律 Title Case；zh-TW 不受影響。角色名一律原樣（title case 不轉大寫）。
