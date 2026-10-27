# refactor-player-card-user-macro — 重構後玩家卡名稱顯示字面 {{user}}

## Summary
clean-orc 重構後，玩家卡名稱顯示成字面的 `{{user}}`。st-ecosystem-upgrades 第二項（巨集替換）已結案，名稱欄位這條路徑未套替換。

## 已知線索
- 巨集替換設計見 [st-ecosystem-upgrades](st-ecosystem-upgrades.md) 第二項。
- 其餘未查（可能是重構產出的玩家卡名稱沒過替換，或把 AI 認出的 `{{user}}` 直接當名稱存）。

## Next action
未排程；開工先重現，找名稱從重構產物到畫面的路徑。

## 驗收方向
重構後玩家卡名稱顯示實際玩家名，不出現 `{{user}}`。
