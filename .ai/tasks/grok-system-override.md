# grok 在 app 裡拿不到角色／GM 設定

Status: todo（排在 usage-cache-audit 前面）〔作者裁決 2026-10-06〕

## Summary
app 給 grok 的 system（扮演規則、公開設定、提進凍結 system 的私設）沒有生效：session 的 `system_prompt.txt` 是 grok 自己的 coding agent 提示，另外被塞約 1.1 萬字技能清單。角色只靠對話裡的「角色回歸」事件看到公開設定，私設完全沒送到。grok 內部旁白混進角色回覆（「先看一下這場酒館戲的角色設定……」）應是同一個病，修完一起看有沒有消失。

2026-10-06 重現（grok 1.0.46）：同一組 `-s <id> --system-prompt-override` 參數，`GROK_HOME` 指向 app 的獨立目錄（`<config>/grok-home`）就失效，用 `~/.grok` 就生效；`GROK_CONFIG`、`--output-format` 不影響。app 的 grok-home 比 `~/.grok` 少很多檔（`version.json`、`bundled`、`rules` 等），`config.toml` 只有 marketplace 幾行。

獨立 `HOME`／`GROK_HOME` 要保留：grok 會自動吃 `~/.claude` 的 hooks／skills／CLAUDE.md、官方無 opt-out（見 `cli/request.rs` 的 `grok_envs` 註解）。

## Next action
逐項比對兩個 grok 目錄，找出讓 override 失效的差異；補進 app 的 grok-home，或改用別的方式把 system 送進去。修完驗：角色與 GM 線的 `system_prompt.txt` 是 app 的設定、私設送到該角色、不再出現 agent 旁白、快取續聊照舊。聊天單發（`grok_args`）同一個旗標也要一起看。
