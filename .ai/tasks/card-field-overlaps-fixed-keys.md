# card-field-overlaps-fixed-keys — 卡欄位與 place／present 同義，GM 只報固定鍵

## Summary
clean-orc 重構後，卡自帶的「当前环境」「驻留/交涉角色」與 app 固定鍵 place／present 意思重疊。flat-field-reporting 實測三回合（Haiku），GM 只把同義內容寫進 place／present，這兩欄停在開場值。寫入通道是通的（有規則、會列給 GM），缺口在模型回報。

## 已知線索
- 出處：flat-field-reporting 交接檔真實流程一節。
- 可能方向：重構時把同義卡欄位併到 place／present、或在指示裡點名要兩邊都報、或介面顯示時由固定鍵帶出。
- 與 character-presence（名冊欄位接在場機制，延後到 VN 前〔作者裁決 2026-10-27〕）相關：「驻留/交涉角色」就是名冊欄位，開工前先看兩案要不要合併處理。

## Next action
未排程；等 flat-field-reporting 結案後開工，先提做法選項。

## 驗收方向
重構後多輪續聊，卡上同義欄位與 place／present 不會一邊更新、一邊凍住。
