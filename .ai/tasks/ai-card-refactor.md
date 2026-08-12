# Task
Task-ID: ai-card-refactor
Title: AI 卡重構按鈕：整卡抽成機制格式＋介面本地化＋人物拆成角色卡
Status: in-progress
Created: 2026-08-13T00:29:59.829904+08:00
Updated: 2026-08-13T01:41:37.295590+08:00

## Summary
接 [state-values-mvu](state-values-mvu.md) 不做清單「下期按鈕」與 [card-import-flow](card-import-flow.md) 另案。按鈕＝AI 讀整張卡，依 [MECHANISM-FORMAT.md](../reference/MECHANISM-FORMAT.md) 一次產三類產物：機制殘渣抽成欄位規則＋觸發表、介面規則抽成狀態樹（app 本地渲染）、人物合集條目切成角色卡候選。產物一律人審後套用。

規格細節（拍板結論（2026-08-04–06 討論定案）、卡型分佈（前置產出，2026-08-06 完成，零額度）、順序）見 [plans/ai-card-refactor.md](../plans/ai-card-refactor.md)。

## Next action
- 七包實作完成，2026-08-10 起實機驗收，開跑抓到三 bug（假檔停在舊契約→展開細看白畫面、匯入無驗證、AI 產的 HTML 殼被前端丟掉）已全修（vitest 82／build／i18n 綠，未 commit）；實測順序改成先做 `refactor-outcome-export`，再從 B 段真跑 orc-cave 卡、產物存檔後回頭跑 A 段，全過與 `person-promote` 兩案一起結案

## Constraints
- 地基（皆已完成）：機制格式規範文件、未收編帳本（state-values-mvu 包 8）、匯入收據（card-import-flow 包 2）。
- 安全紅線沿用：卡片內容永遠當資料、永不執行；容錯＝抽不出就留在帳本，遊戲照常。
- 重構燒使用者自己的額度：按鈕必須主動按，不自動跑。
