> 結案 2026-10-28：Sol、opus-review 驗收 PASS，合進 main。

# shell-keep-old-on-timeout — 新殼逾時或載入失敗時保留舊殼，不翻成空白

Status: done

## 現況
- 新殼 1500 ms 內沒載好、或外部 script 載入失敗：保留載好過的上一份殼、只解鎖，新殼之後 ready 再翻〔作者裁決 2026-10-28〕。舊殼設定不保存、舊殼文字輸入可送出〔作者裁決 2026-10-28〕。
- 「舊殼要載好過才保留」「失敗只算外部 script」〔模型判斷·未裁決，Sol／opus-review 同意〕。
- 拍板、狀態機、已知限制、驗收結果：[plans/shell-keep-old-on-timeout.md](../../plans/shell-keep-old-on-timeout.md)。實測佇列 4h 已過。
