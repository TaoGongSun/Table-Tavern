# 更新與回退點下載沒有逾時

Status: todo

## Summary
api-request-header-timeout 排除項（不是 AI 請求，移出該案）：
- 回退點下載 `commands/update.rs` `fetch_release_asset`：送出與本文都無限等。reqwest `ClientBuilder::read_timeout` 對回應頭是單一期限、對本文是閒置期限，一行可同時管兩段且不殺大檔。
- 更新外掛 `plugin.check()`（`commands/update.rs:51`）：`UpdaterBuilder::timeout` 預設 None、只套 check，可一行補。`update.download()` 本文同樣無時限。

## Next action
未排程。
