# codex-skills-in-prompt — 使用者自己的 codex 技能清單被帶進跑團 prompt

## Summary
non-claude-real-cache 審查（2026-10-28）讀 codex rollout 發現：app 的 codex 單發第 1 輪就帶入使用者自己的 codex 技能清單（skills_instructions developer 訊息，約 4.9KB），續聊第 4 輪又重注入 19.5KB。等於把玩家個人的 codex 設定混進跑團請求，浪費 token、也可能影響旁白。grok 已用假 HOME 隔離同類問題（見 transport/dispatch.rs grok_profile）。

## Next action
未排程。先查 codex 讀技能清單的來源與可關閉的旗標／設定；評估用 CODEX_HOME 或 config 覆寫隔離（與 codex-lane-resume 的 session 位置決定一起看）。
