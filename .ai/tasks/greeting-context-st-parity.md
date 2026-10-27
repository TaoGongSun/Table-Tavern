# greeting-context-st-parity — 開場白進上下文的方式對齊 ST

## Summary
gm-reprints-retired-statusbar 查證：我們把 first_mes 放公開設定「### 開場白」、全部 alternate_greetings 放私有設定「### 備用開場白 n」，每回合原樣送進 GM system（main_furry 30 則約 2.7 萬字每次呼叫都送）；逐字稿第 0 則送的卻是剝過狀態欄的 text。ST 是：開場白只當聊天第 0 則（選中那則原文）與 swipe，從不放進人設。

## 已知線索
- 重構桌已在 gm-reprints-retired-statusbar 處理（限 characters 且淘汰狀態欄）；本案管沒重構的桌。
- 方向（Sol／Grok 共識）：第 0 則送選中開場白的 raw 候選（不是所有事件改送 raw，免得剝掉的控制內容回來）、system 不送開場白與未選備用開場白；要確認 API／CLI、選開場白、續聊與 lane 快取。
- 匯入不能剝（export.rs 靠段頭反解 alternate_greetings）。

## Next action
未排程；等 gm-reprints-retired-statusbar 結案後開工，先重播比對格式遵守度。

## 驗收方向
沒重構桌照卡格式回覆的遵守度不下降，GM 輸入少掉未選開場白。
