> 結案 2026-10-04：進 main；Sol 複驗通過、本機 verify 全綠、真卡驗收過。

# gm-format-directive-missing-target — GM 導演指示指向不存在的格式條目

Status: done

規格與拍板：[plans/gm-format-directive-missing-target.md](../../plans/gm-format-directive-missing-target.md)

## Summary

- 起因（card-mvu-shim 實機驗收 2026-10-03）：導演指示要求「照卡片規定格式」，但格式條目被匯入收編停用或根本不存在，模型回「未附上介面規格」或亂抄範例路徑。
- 匯入：`[mvu_update]` 只有抽得出欄位規則（`import::is_field_rule_table`）才強制停用，其餘照原卡啟停。
- 判定：`transport::gm_prompt_full_entries` 給出全文進本輪提示的條目（已登場人物取登場事件當時的本文），`card_format_entry` 只從裡面找；`transport::card_format_turn` 同時決定指示與收尾句，找不到就用中性版 `GmTurnFormat::CardFormatAbsent`。scaffold_baseline 三份含 closing 兩態。

## 真卡驗收（2026-10-04，測試通道新匯入，ai-log 零派送）

- 世界書啟停：過。
  - bcd368：格式說明（原卡 id 13/14）啟用，規則表停用（app 抽出 3 條規則），`[initvar]` 與「前端」（id 7）停用。
  - DongeonMaster：格式說明（id 21/22）啟用，`[initvar]` 停用；`[mvu_update]变量更新规则`（id 20）是 TypeScript 型別寫法，app 抽不出規則，照原卡維持啟用。
- 提示組裝（API 單發 messages 與 CLI lane system／tail 都看過）：中性版過。格式條目停用或 keyword 沒命中時用中性指示與收尾句，不出現「卡片已經規定了回覆的輸出格式」。
- 點名版在這兩張卡走不到：`format_tags` 抽不到中文標籤、自閉合標籤和 regex 群組標籤，所以格式全文在提示裡時也走中性版。接受這個結果，不另立案〔作者裁決 2026-10-04〕。
- 模型實際的狀態更新品質要另外實聊才看得出來。

## 後續相依

- 與 mvu-replace-numeric 合併相依：已寫進 [tasks/mvu-replace-numeric.md](../../tasks/mvu-replace-numeric.md)。
