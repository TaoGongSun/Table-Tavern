# lobby-table-badges — 大廳不分單人／多人：房間釘頂、桌多才出篩選籤、入口卡重排

Status: 已合併 main（2026-11-07；Sol 施工 1 包、opus-review 驗收 1 輪 PASS、verify 17 步綠、截圖主線親看、主線 Fable 自做常識審）
Created: 2026-11-07
案別：中案，核心約 75 行（Lobby／TableCard／CSS／STRUCTURE.md），測試 139、i18n 約 60。規格與已知限制見 [plans/lobby-table-badges.md](../plans/lobby-table-badges.md)。

## Summary
多人是桌的狀態、跑團是桌的能力，都不是桌種，大廳不分區。做了：房間開著的桌排第一並帶「房間中・房間碼」色帶；8 張桌以上且至少兩個類別有桌才出「全部／聊天／VN／開過房」篩選籤（空類別不出籤）；入口卡改成開新桌、一句話開桌、視覺小說、和朋友一起玩；擴充規則寫進 docs/STRUCTURE.md。〔作者裁決 2026-11-07，採納主線建議〕

## 下一步
- 作者實機看：房間開著時回大廳的色帶、8 張桌以上的籤（排實測佇列梯 1）。
- 跑團徽章與建桌流程「帶規則書」選項等 [ttrpg-rules-system](../tasks/ttrpg-rules-system.md) v1 有偵測結果再接。
