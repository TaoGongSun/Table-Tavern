# Task
Task-ID: i18n-more-languages
Title: 介面擴充多語系（十國語言，AI 產字典）
Status: in-progress
Created: 2026-07-24T00:05:00+08:00
Updated: 2026-07-30T00:00:00+08:00

## Summary
十國語言（繁中、简中、英、日、韓、西、葡巴、德、法、俄）的介面字典、首開範例桌內容、AI 輸出語言規範三處已全部上齊。架構同步改造：字典拆成每語言一檔（src/i18n/）、範例桌文本抽成資料檔（src-tauri/samples/），新增語系不必動程式邏輯。品質關卡＝Gemini 產初稿 → Opus 逐語系審校 → 機械量測按鈕寬度 → 退回縮短；體檢腳本留在 repo（npm run check:i18n）。驗證：npm run build 綠、cargo test 116 passed（含兩個新語系測試）、四處一致性檢查十語系全齊。

## Next action
- 實機驗收：真 app 逐語系切過去看畫面（德俄按鈕最長、日韓字形、首開十選項排版），這是唯一擋結案的項目
- 四件待拍板見 handoffs/i18n-more-languages.md「待拍板」段（日文世界書用詞、日文角色名「狐」、範例桌地名創譯或音譯不統一、角色性別留白）

## Constraints
字典品質要有驗證關卡，不能 AI 產完直接上；每加一語系，範例桌內容與 LANGUAGE_RULE 需同步，缺一不上該語系。
