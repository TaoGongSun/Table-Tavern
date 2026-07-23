# Task
Task-ID: i18n-more-languages
Title: 介面擴充多語系（十國語言，AI 產字典）
Status: todo
Created: 2026-07-24T00:05:00+08:00
Updated: 2026-07-24T00:05:00+08:00

## Summary
2026-07-23 使用者提議：本產品主打 AI 生成、模型本就能回覆多國語言，介面字典也可用 AI 產生十國語言，降低外語使用者首開門檻。現行 i18n 架構（i18n.ts 逐鍵字典＋LANGUAGE_OPTIONS）已可直接擴充；範例桌內容（create_sample_world）與後端 LANGUAGE_RULE 也要跟上同一批語系。

## Next action
- 先定目標語系清單與品質驗證流程（AI 產初稿＋抽查？母語者驗？），再一次擴 i18n.ts／LANGUAGE_OPTIONS／create_sample_world／LANGUAGE_RULE；首開下拉與設定外觀頁自動跟上

## Constraints
字典品質要有驗證關卡，不能 AI 產完直接上；每加一語系，範例桌內容與 LANGUAGE_RULE 需同步，缺一不上該語系。
