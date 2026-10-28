# cli-version-gate — CLI 升版改輸出格式只在執行期才發現

Status: 已結案（2026-10-29 合併 main，包 1、包 2 各一筆）

包 1：四家都要收尾事件、認不得的格式明確報錯並附上次正常版本、真實夾具契約測試（`cli/stream/fixtures/`、`contract_tests.rs`；重錄夾具後要更新 `EXPECTED` 表）。包 2：spawn 注入關閉自動更新 env 並過更新擋板；app 內更新鈕（已測版／最新版、程序群組與 Job Object 殺整組並確認停止、`find_binary` 那支 `--version` 驗證、四家還原契約、分 CLI 啟動復原標記、停不下來時擋板留到重開）；首次安裝裝已測版。方案、已知限制（含兩條〔模型判斷·未裁決〕待作者裁決）見 [plans/cli-version-gate.md](../../plans/cli-version-gate.md)；實機待驗見實測佇列 27、29。
