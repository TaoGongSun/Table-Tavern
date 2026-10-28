> 結案 2026-10-28：Sol、opus-review 驗收 PASS，squash 進 main。

# narrator-prefix-echo — GM 把歷史裡 app 自己的「（旁白）」標籤照抄進正文

## 根因
續聊線把 GM 自己先前的旁白攤成 user 文字時標「（旁白）」，GM 當成回覆格式照寫在開頭，落檔後又留在 session 裡越抄越穩。

## 做法
GM 線旁白改標「GM：」（與單發攤平一致），歷史旁白開頭的舊標籤送出前剝掉；GM 回覆顯示開頭照抄的標籤由 `transport::strip_narrator_label` 剝除，剝過的回合下一輪續聊線刻意重開。規格、驗收數據與已知限制見 [plans](../../plans/narrator-prefix-echo.md)。
