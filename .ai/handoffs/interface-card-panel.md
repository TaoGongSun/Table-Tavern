# Handoff: interface-card-panel

v1（2026-08-04 實機驗收全過）與 v2（多卡分頁＋腳本按卡隔離、破圖收斂、介面底部輸入列；2026-10-11 三方驗收 PASS）都已進 main，首發必含〔作者裁決 2026-10-04〕。程式無待做，剩真視窗實測（實測佇列梯 1 第 4f）。規格：v1 [plans/interface-card-panel.md](../plans/interface-card-panel.md)、v2 [plans/interface-card-panel-v2.md](../plans/interface-card-panel-v2.md)。

## 現在的行為
- 讀取：`src-tauri/src/import/interface.rs` 的 `read_card_interfaces` 讀每張未封存角色的原始卡檔（`<id>.png`／`<id>.import.json`）與世界書卡原始檔，抽 `extensions.regex_scripts` 的顯示腳本（未停用、非 promptOnly、placement 含 2）。偵測 SCrypt（DRM）與雲端載入器就清空腳本並回原因，不解不繞。這桌不允許卡內 regex（網頁存檔匯入時）就整組清空。
- 選殼：`src/features/card-interface/card-shell-route.ts` 的 `pickCardShell`（最新 GM 樓→近 10 樓非玩家樓→空桌時各卡開場白；指定卡時掃整幕）＋`interface-card.ts` 的 `findShell`（腳本按卡隔離、候選帶來源：台詞樓給說話者的卡、開場白給所屬卡、GM 樓世界書卡先；抽 ```` ```html ```` 圍欄或裸頁面）。角色優先桌（`characters`）一律不顯示；有重構骨架時走骨架。規則細節見 v2 plan。
- 多卡分頁：畫得出的卡 ≥2 張時覆蓋層工具列出現卡名分頁；預設跟著最新，點了就固定，直到再切、離開這桌或那張卡畫不出殼。
- 顯示：全螢幕覆蓋層 `CardInterfaceOverlay.tsx`，單 iframe `sandbox="allow-scripts"`＋srcdoc，key＝殼指紋，殼一換整支重掛。沙盒內建 jQuery／lodash／errorCatched、讀訊息墊片、MVU 墊片、storage 墊片（設定存回宿主、一桌一份）、IME 守衛、Esc 回報、破圖收斂（抓不到的圖藏起來保留位置）。
- 送出：誘餌 `#send_textarea`、`triggerSlash('/send …')`、`/trigger` 一律 postMessage `kind: "input"` → `submitText` **直接送出**，畫面留在介面裡等回覆（左上「正在打字」）。`/trigger` 單獨＝送空字串、請對象直接接話。不擋卡片自己觸發回合（ST 也不擋）。
- 介面底部輸入列：與聊天輸入框共用草稿、送出路徑與停用條件，介面開著也能打自由行動。
- 純世界書桌沒有角色時發言對象預設 GM。
- 有可渲染的卡片介面時，GM 回合尾改成要求照卡片格式輸出（`chat_assembly.rs` `gm_turn_instruction`＋`card_format_entry` 自動找世界書裡教格式的條目）；沒有介面的桌逐字不變。
- 匯入完畫得出殼就自動打開一次（`openIfDrawable`）；匯入提示依結果分流（有介面／加密／雲端載入器／只有非顯示腳本）。
- 已拍板不做：聊天區把 XML 收成一行提示（關掉介面才看得到、也不省額度）。

## 回歸檢查（v1 實機清單）
1. 開新桌匯入 `TestCards/WestFantsy.png` → 判成世界書卡只給確認／取消 → 38 條進世界書 → 問開場白 → 介面自動打開、標題列有「卡片介面」鈕。
2. 開場就是選角畫面（零 AI）；選角色按「踏入命运」→ 直接送出、介面留著；Esc／✕ 關得掉。
3. 關介面照常玩；沒帶介面的桌看不到那顆鈕。
4. `SCrypted_WestFantasy.png` → 提示介面被加密保護、其餘照常。
5. `TrainEmperor.png` → 提示介面要連作者網站、不支援。
測試通道回歸：`node scripts/harness-e2e.mjs <root> TestCards/WestFantsy.png <截圖目錄>`。

## 等實機驗收
實測佇列梯 1 第 4f：真視窗輸入列注音／拼音選字（Enter 選字不誤送、字上得去）、多卡分頁點選。零 AI 部分已用測試通道代測（西幻＋donass、DongeonMaster＋bcd368、NorthHall＋寻道太虚三組多卡；假端點輸入列送出與失敗放回；單卡西幻 e2e 回歸）。

## Constraints
- 卡內腳本一律沙盒、無任何 app API；沙盒對宿主只有 postMessage 的固定幾種訊息（送字、存設定、關閉、MVU 讀寫），宿主一律核 token。
- 不碰 DRM（不解密、不繞驗證）。
- 面板為選配，任何失敗一律回退純文字對話。
- 酒馆助手（JS-Slash-Runner）**只當規格書讀，禁抄碼**（Aladdin 授權禁商業散布，本 app 有贊助包銷售）。
