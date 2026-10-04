> 結案 2026-10-04：Sol 第 4 輪複驗通過，本機真卡驗收全過，進 main。

# mvu-replace-numeric — 未重構 MVU 卡數字欄 replace 照上游接受

規格、拍板與各輪修正見 [plans/mvu-replace-numeric.md](../../plans/mvu-replace-numeric.md)。

## 結果
- 沒重構的 MVU 卡：數字欄 replace 照上游 MagVarUpdate 收（型別照上游、不夾值、轉不成數字寫 null）；重構過的卡維持強制 delta〔作者裁決 2026-10-04〕。
- 改動：`mechanism/policy.rs` 判定策略、`apply.rs` `replace_existing` 放行、`mechanism/upstream_set.rs` 批內完整型別視圖與上游 set、`gm_materials`／turn.commit／`append_opening` 帶同一份策略、`transport/context.rs` 協定分版；前後端比對案例 `src/shared/contracts/mvu-replace-parity.json`。
- verify 全綠：vitest 926（＋3 skipped）、cargo 913、harness 28。

## 本機真卡驗收（2026-10-04，測試通道，零真實派送）
做法：開場白用 `post_opening` 貼自帶 `<UpdateVariable>` replace 的文字；GM 回合把 `preferences.base_url` 指到本機假 OpenAI 相容端點（回固定 GM 回覆、記錄請求本文），按「GM 旁白」走真的 GM 提交路徑。
- bcd368（根源重塑app，不重構）：開場白 `资金 "99000"`→99000、`名称` 維持字串、`积分 "abc"`→null、`年龄 20`；GM 兩輪 `资金 50000`、`等级 "2"`→2、`资金 "70000"`→70000。面板、樹、提交樓 `stat_data` 一致，mechanism-log 無拒收；重啟 app 值仍在；下一輪 GM 提示的目前狀態是新值，系統提示是上游版（「也可以用 replace 直接寫新值」）。
- DongeonMaster（迷宫之主，不重構）：開場白 `带出财宝计算 "1500"`→1500、`已覆灭小队数 "x"`→null、`时间` 字串；GM 兩輪 3000、`"4200"`→4200；其餘同上全過，重啟後 4200。
- 對照組：bcd368 另開一桌，關 app 把 `state.json` 的 `refactor_mode` 設成 `interface`（模擬重構標記，未真的跑 AI 重構；TestCards 既有重構產物都是非 MVU 卡）。開場白三個數字欄 replace 全擋（「請用增減量（delta）」）、字串欄照收；GM 回合系統提示是原規則版（「給絕對值會被系統擋下」），资金 replace 被擋、下一輪提示帶擋下說明。
- `[值, 說明]` 只改第 0 項：兩張卡沒有這種欄位，由單元測試與比對案例涵蓋。
