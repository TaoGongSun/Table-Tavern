# Handoff: state-values-mvu

## Current state
2026-08-04：包 1（標籤放寬＋`<maintext>` 剝除）完成，cargo test 222 綠、五張樣本卡自驗過。下一步＝包 3（`[initvar]` 匯入）或包 2（機制格式核心），依對話額度挑。

## Completed
- 包 1 標籤放寬（主線 Opus 5 直寫，小包不外包）：
  - `transport::find_state_tag`（transport.rs:748）：掃描器取代原本寫死的 `["status", "updatevariable"]` 逐字比對。標籤名走**前綴**比對——`<StatusData>`（donass）、`<Status_block>`（鎮北王府）都認；開閉標籤同名才配對，`<combatStatus>` 這種「名字裡有 status 但不是開頭」的卡片自訂欄位不會被誤剝。收不收欄位由 `collect` 旗標決定：status 系收、`<updatevariable>` 只剝不收（JSON patch 歸包 4）。沒有配對收尾的標籤整段留著不動，寧可讓玩家看到半截標籤也不吞後面的旁白。
  - `<maintext>` 剝殼（transport.rs `extract_state_block` 內）：只拆掉開閉標籤、內容原樣留在正文（不是「只取 maintext 內容」——模型在殼外多寫的東西不能丟）。大小寫不敏感，順帶蓋掉勇者卡 CloverArchive 的 `<mainText>`。單獨出現（沒有狀態區塊）時一樣要拆，所以 `removed` 也要跟著設。
  - `data::STATE_BAR_MARKERS`（data.rs:831）補 ```` ```status ````，11→12 詞。原本 ```` ```state ```` 涵蓋不到 ```` ```status ```` 圍欄，而 `extract_state_block` 的圍欄比對認它——這是這次對齊補的唯一缺口；`<status` 本來就是子字串比對，前綴放寬後自動涵蓋各家標籤名。
  - 新增 5 個測試：status 前綴變體（兩張卡標籤混在同一則）、未閉合標籤原樣返回、`<combatStatus>` 不誤剝、`<maintext>` 拆正文（含只有殼沒有狀態區塊）。

## Verification
- `cargo test` 222 passed（217→222）；clippy 無新警告（既有 9 個與本包無關）；`cargo fmt --check` 新增區段乾淨（檔案其餘 fmt 差異是既有的，未動）。
- 五張樣本卡真實輸出跑過一次性煙霧測試（跑完即刪，不留在 repo）：donass `<StatusData>` 收 10 欄、鎮北王府 `<maintext>`＋`<Status_block>` 收 19 欄且正文完整、orc-cave `<details>` 收 4 欄、勇者與根源重塑 `<UpdateVariable>` 剝除不收欄。五張的顯示文字都不含裸露的 `<status`／`<updatevariable`／`<maintext`／`<details`。
- 鎮北王府的 19 欄是巢狀 YAML 被平面解析的結果（兩個人的欄位混在一層）——預期如此，樹狀結構歸包 2。

## Remaining
包 2–8 全部未開工，內容見 [tasks/state-values-mvu.md](../tasks/state-values-mvu.md) 分包段。順序建議：包 3（中）→ 包 2（大）→ 包 4（大，核心）→ 包 5 → 包 7 → 包 6／包 8（小，可併）。包 2／4／7 各自吃滿一次對話。

## Notes
- 樣本卡在 TestCards/（gitignore）。從 PNG 取卡片 JSON：讀 tEXt/zTXt chunk 的 `chara`／`ccv3`（base64 JSON）。
- 勇者卡另有一組 `<CloverArchive>` 全回應 XML（PlotModule／PlayerModule／HeroModule…，`<mainText>` 包正文）——那是 ST 前端渲染用的另一套殼，跟 `<UpdateVariable>` 不同機制。本期不處理，未來若要接是包 2 樹的另一個來源格式。
