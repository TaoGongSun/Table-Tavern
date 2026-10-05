# API 串流沒有停滯逾時

## Summary
usage-cache-audit 實跑（2026-10-06）：OpenRouter 免費模型兩通串流卡住超過 5 分鐘，app 沒有逾時，只能手動按停止。CLI 通道有 120 秒斷流偵測，API 通道沒有。證據在 usage-cache-audit 計畫檔「四、對帳結果」。

## Next action
看 API 串流讀取路徑，比照 CLI 加停滯逾時，錯誤走既有人話路線。
