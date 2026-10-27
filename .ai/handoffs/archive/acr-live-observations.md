# acr-live-observations — 重構案實測看到的兩件事

> 結案 2026-10-27：第 1 件併入 [present-name-interface-lang](present-name-interface-lang.md)、第 2 件由 [flat-field-reporting](flat-field-reporting.md) 結案。

## Summary
ai-card-refactor 實測（測試通道，2026-10-27）看到三個現象。第一件（在場名單字形）已拆成 [present-name-interface-lang](present-name-interface-lang.md)；本檔留下另外兩件，兩條互相獨立，可拆案。

1. **人物進在場名單太晚**：人物條目已出場說話，卻沒進在場名單，登場事件因此晚到。**已結：併入 [present-name-interface-lang](present-name-interface-lang.md) 結案（2026-10-27）。**
2. **clean-orc 平欄劇情欄位不更新**：平欄的「剧情阶段」「当前环境」不在狀態樹上，增量被拒，永遠停在開場值。

## 1. 人物進在場名單太晚
根因：
- `transport/response.rs:53-54` 沒要求把名冊（`transport/worldbook.rs:13`）上的人物列進 present，加上「沒變化的欄位照抄目前值」，模型就沿用上一輪的名單。
- `transport/arrivals.rs:24-25、42-48`：只要 present 存在，就停用正文比對。
- A/B 實測（每組 n=3）：現行措辭 1/3 會列；改成「列出此刻在現場的所有人，含名冊上的人物」3/3，但會多列沒有條目的人。

修法方向〔模型判斷·未裁決〕：指示點明名冊上的人；或 present 沒列到時，用正文命中名冊名字補上。

## 2. clean-orc 平欄劇情欄位不更新
**已結：拆成 [flat-field-reporting](flat-field-reporting.md) 結案（2026-10-27）。**

根因：
- 「剧情阶段」：AI 規則的路徑是「劇情階段」，平欄的鍵是「剧情阶段/要塞事件」。`align_path`（`refactor/apply_mechanism.rs` 的 `namespace()`）只折疊簡繁，接不上後綴，所以 `open_rule_reporting`（約 :312）沒把它搬進樹，`mechanism/apply.rs:431-437` 就以「路徑不存在」拒收。鍵名裡的「/」也和 JSONPatch 的分隔符衝突。
- 「当前环境」：沒有規則讀它，所以從來沒搬進樹。
- 平欄的鍵列在給 GM 的「目前狀態」裡（`state_view.rs:54-66`），指示又叫 GM 用更新區塊回報（`response.rs:218-219`），但更新區塊只寫得進樹。
- 第二輪 GM 改用 insert，結果樹上新長出一份，和平欄並存成兩份。

修法方向〔模型判斷·未裁決〕：開啟增量時，把平欄的非固定鍵都搬進樹（要處理「/」）；或讓單層路徑在樹上找不到時回落到平欄。業界參照：酒館 MVU 寫入不存在的路徑時會自動建立。

## Next action
無，已結案。

## 驗收方向
1 以登場事件不晚於人物首次開口、且不多列沒有條目的人為界；2 以劇情欄位隨回合更新、樹上不並存兩份為界（暫定）。
