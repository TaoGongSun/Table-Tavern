# MVU 墊片事件測試在負載下偶發失敗

Status: todo〔作者裁決 2026-10-10：立案，再遇到才處理〕

## Summary
`src/features/card-interface/mvu/card-mvu-shim.test.ts` 的「async 監聽器依序等完；連續兩次推送排隊處理、不交錯」靠 `setTimeout(60)` 等事件處理完。2026-10-10 主線在背景同時跑多個 cargo test 時，verify 第 5 步這支失敗一次（只收到前 5 筆紀錄），單獨重跑 3 次全過。

## Next action
再遇到才處理：改成等待明確的完成訊號，不靠固定毫秒。
