# grok-first-session-preamble-drop — 新 grok-home 第一通就丟線

## Summary
2026-10-28 實測第 14 項（grok 1.0.70、grok-4.6，測試 root 剛登入的 grok-home）：第一通回合後抹寫失敗，`chat_history 本輪有預期外的項目 Some("user")`（`src-tauri/src/lanes/grok_session.rs:369`），整條 lane 被丟、下一通重開；之後不再出現。grok 1.0.70 每個 session 開頭自己塞兩則 user（`<user_info>`、可用 skills 提醒），新 grok-home 第一通時才裝 bundled skills，推測與此有關，未證實（①的 session 已隨丟線刪除）。另觀察：冷啟動讀到的快取從 2026-08-21 的 10,368 掉到 128。證據在主線 scratchpad vq/14-grok-*。

## Next action
未排程。開工先在乾淨 grok-home 重現第一通，保留 session 檔看前導 item 的位置與角色，再決定抹寫的預期形狀要怎麼放寬。

## Constraints
- 不讀、不複製作者的 ~/.grok；用測試 root 的 grok-home。
