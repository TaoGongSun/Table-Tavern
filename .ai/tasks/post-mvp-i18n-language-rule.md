# Task
Task-ID: post-mvp-i18n-language-rule
Title: MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入
Status: todo
Created: 2026-07-20T10:50:13.434298+08:00
Updated: 2026-07-20T10:50:13.434298+08:00

## Summary
transport.rs 的 `LANGUAGE_RULE`（繁體中文＋台灣慣用語規範，注入角色與 GM 兩個 system prompt）目前是無條件寫死。未來做多語系（i18n）時，這段提示詞應只在使用者語系選擇繁中（zh-TW）時注入；其他語系換成對應語言規範或不注入。緣起：2026-07-20 修正角色語氣飄成中國用語的問題時新增此常數。

## Next action
- 等多語系功能開工時處理；屆時把 LANGUAGE_RULE 的注入改為依使用者語系設定條件化（設定檔需先有語系欄位）

## Constraints
在多語系功能存在前不動現狀（現在寫死繁中是正確行為）。
