# 世界編輯器關鍵字分隔符沒跟語系

Status: 已結案（合併 main）

## 狀態
- 玩家看得到的清單串接一律走 i18n 鍵 `listSeparator`（zh-TW／zh-CN／ja 為「、」，其餘七語系為 `, `）：世界書列表與編輯表單關鍵字欄、撤銷匯入訊息、免費模型橫幅、重構結果對話框與流程訊息。
- model 層保持純函式：`draftFromEntry`／`rebaseDraft`／`sourceEntryTitles` 的分隔符由呼叫端傳入；關鍵字解析仍 `,` 與 `、` 都收。

## Next action
無。
