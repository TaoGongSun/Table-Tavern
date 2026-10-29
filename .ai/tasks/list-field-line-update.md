# 清單欄逐行更新

Status: todo

## Summary
重構接管後清單欄（如 Transfur 神經狀態 `neuro|疲勞|20|…`）改一行也要整欄 replace。想讓 GM 只報變動那幾行：重構時替每個 `|` 記錄式清單欄宣告鍵欄位（FieldRule 選填 `line_key`），新增 `upsert`（依鍵整行替換或附加）與依鍵 `remove`；沒宣告鍵的清單維持只收 replace。動到資料格式、重構提示詞、套用程式、GM 提示詞四處。
「固定幾行、只有數字在變」的欄，重構時直接拆成獨立欄位交給 app 畫可能更好，開工時一併評估。
背景與最小規格見 [takeover-text-field-delta](../plans/takeover-text-field-delta.md) 的「不做逐行合併」段。〔作者裁決 2026-10-11：立案進待辦，之後排〕

## Next action
未排程；其他接管問題處理完再評估。
2026-10-29 輕防重審：縮小〔模型判斷·未裁決〕（作者裁決只管立案）。最小版＝重構時把「固定幾行、只有數字在變」的清單拆成獨立欄位交給 app 畫；不新增 upsert／remove 協定。
刪掉的部分成已知限制：行數會變的清單欄仍是整欄 replace。
