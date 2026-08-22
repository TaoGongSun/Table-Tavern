# grok-tool-schema-overhead — Grok 工具定義佔 context

## 結論（2026-08-22 實測，grok 1.0.5／grok-4.6）
聊天單發加 `--disallowed-tools <內建工具全集>`：CLI 內部 log 的 `tool_count` 由 24 歸 0、
`available_commands` 空陣列、input tokens 12604 → 3602、單次 cost $0.0267 → $0.0086。
`--deny *` 保留當第二層（它只擋執行，不擋 schema 注入）。生圖通道不設，仍用 image_gen。

## 過程中踩到的兩個坑
- **顯示名 ≠ 工具 ID**：串流事件 `available_commands` 報 `run_terminal_command`，但過濾旗標認
  README 的 `run_terminal_cmd`。只寫顯示名會靜默無效，shell 沒被移除、tool_count 停在 1，
  而 token 仍大降（12604→3905），光看 token 會誤判成功。常數兩個名字都列。
- **`--tools` allowlist 走不通**（三組反例）：空字串等同沒設（24 個工具原封不動）；
  錯 ID 時 tool_count 21；正確 ID `--tools run_terminal_cmd` 也只換成 `search_tool`／`use_tool`
  兩個元工具（tool_count 2）。1.0.5 的行為與文件「停用預設注入」的描述不符。

## 維護
CLI 升版後跑一次廉價文字 smoke test，確認 `available_commands: []` 與 `tool_count: 0`；
漏網工具會回到 toolset，模型挑它去用、被 `--deny *` 擋下後那一輪就沒了（`--max-turns 1`），
玩家看到的是一次空回應。不需要動態解析機制（Sol 2026-08-22 驗收意見）。
