# Task
Task-ID: post-mvp-i18n-language-rule
Title: MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入
Status: completed
Created: 2026-07-20T10:50:13.434298+08:00
Updated: 2026-07-23T00:15:00+08:00

## Summary
transport.rs 的 `LANGUAGE_RULE` 原本無條件寫死繁中規範。2026-07-23 隨 ui-i18n-switch 完成：改為 `language_rule(lang)` 依 `config.preferences.language` 注入——zh 系維持繁中台灣用語規範，en 注入英文輸出規範（transport.rs:12-21，測試 `language_rule_follows_ui_language`）。

## Next action
- 無，已結案（隨 ui-i18n-switch 完成）。

## Constraints
在多語系功能存在前不動現狀（現在寫死繁中是正確行為）。
