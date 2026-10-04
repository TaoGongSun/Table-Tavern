# gm-format-directive-missing-target 拍板與分包

原則：沒重構的卡照酒館怎麼做就怎麼做。依據：Sol 審查共識（審查串 codex exec session 01a104b2-86be-79d2-9aa5-7eb86dd4cb41）。

## 拍板

1. **匯入分類（根因一）**：`[mvu_update]` 前綴只在「本 app 真的從內文抽得出欄位規則」時才算機制鷹架、強制停用；抽不出規則的同前綴條目（变量输出格式／强调等格式說明）照原卡啟停。判定共用 `import/mechanism.rs` 的規則表解析（`collect_field_rules`），不另寫近似規則。`[initvar]`、EJS `<%`、`{{format_message_variable::` 維持停用。原卡本來停用的條目（bcd368 id 7「前端」）維持停用。不全面解除停用。
2. **格式條目判定（根因二）**：`card_format_entry` 只從「全文實際進入本輪 GM 提示」的條目裡找。這組條目由 `transport::gm_prompt_full_entries` 給出，與提示組裝共用同一套選取：`render_for_prompt(Side::Gm)` 後的 `active_worldbook_entries`；constant 條目扣掉只進名冊的人物條目（`split_person_roster` 同一個判定）；keyword 條目全文進回合尾段。已登場的人物條目（登場事件帶全文）也算。
3. **導演指示與收尾句**：同一個判定（`transport::card_format_turn`）同時決定指示與 `gm_closing`。找得到格式條目→現行點名文案；找不到→中性版（`GmTurnFormat::CardFormatAbsent`）：推進劇情、遵循上下文已有的卡片格式與狀態更新協定、沒額外格式就直接續寫、不加格式說明或要求補規格。不寫「只輸出正文」、不禁狀態欄或更新區塊、不退回 Narration。
4. 影響 gm_narrate 的 API 單發與 CLI lane（closing 接尾段）；角色發言線、換幕摘要不動。只做 zh／en（scaffold_en）。
5. 舊桌停用狀態不遷移（發佈前舊桌不用管〔作者裁決 2026-10-02〕）。
6. 卡片顯示腳本抽不到標籤時（`format_tags` 不認中文、自閉合、regex 群組標籤，例如 bcd368、DongeonMaster），格式全文就算在提示裡也走中性版，不另立案。理由：全文已在提示裡，中性版又要求遵循上下文格式，比較接近酒館不插導演指示的做法〔作者裁決 2026-10-04〕。

## 分包

單包施工（規模小）：匯入分類＋判定共用＋文案＋基準快照＋測試。

## 驗收

- 自動：`npm run verify` 全綠；單元測試涵蓋有格式／無格式／未停用但 keyword 未命中／停用／constant 人物條目只剩名冊／keyword 命中時帶格式全文；匯入分類正反例（啟用中的格式條目保留、停用的前端條目維持停用、規則表仍停用）；scaffold_baseline zh-TW／zh-CN／zh-HK 三份含 closing 兩態。
- 真卡（本機，2026-10-04）：世界書啟停過；提示組裝中性版過；點名版在這兩張卡走不到（見拍板 6）。細節見[交接檔](../handoffs/archive/gm-format-directive-missing-target.md)。
