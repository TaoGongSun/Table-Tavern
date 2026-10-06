> 結案 2026-10-06：已進 main，Sol、Grok 驗收通過。

# grok 在 app 裡拿不到角色／GM 設定

Status: done

## Summary
app 呼叫 grok 一律帶 `GROK_CAMPAIGNS=0`（`cli/request.rs` 的 `grok_envs`），角色線、GM 線、聊天單發的 `--system-prompt-override` 都會生效：session 的 `system_prompt.txt` 是 app 的設定，角色線只含自己的私設，不再出現 agent 旁白。

根因（grok 1.0.46）：遠端 campaign（例：`grok-4.7-launch` 把預設模型改成 4.7）在 app 的 grok-home 沒被 dismiss，`-m` 不是 campaign 預設模型時 override 會被丟掉。`~/.grok` 正常是因為已 dismiss。只用環境變數，不寫 config.toml、不寫 dismiss 檔。

獨立 `HOME`／`GROK_HOME` 照舊保留：`[compat.claude]` 開關不保證涵蓋全部相容項，且登入態要與使用者的 `~/.grok` 分開。

## 驗收（2026-10-06 測試通道，grok-4.6，全新開線）
角色（狐狸、騎士）與 GM 線 system 是 app 設定、私設辨識碼不串線、GM 拿得到私設原文、回覆非空且無旁白；`-r` 續聊 session 與 system 不變，狐狸第 3–4 輪命中 94%；聊天單發、`grok models` 探針、grok 生圖冒煙正常。GM 線前 5 輪幾乎不命中、第 6 輪 96%，此現象歸 usage-cache-audit 追查，未證實原因。

## 未驗
- 騎士線只跑開線一輪，續聊未驗。
- Windows 未測。

## 不處理〔模型判斷·未裁決：Opus／Sol／Grok 共識〕
- session 第 2 則約 11060 字的內建技能清單（`--disallowed-tools` 管不到；只有寫 grok-home 的 `config.toml` `skills.disabled` 能去掉）。
- 剩下的 `send_feedback` 工具。
