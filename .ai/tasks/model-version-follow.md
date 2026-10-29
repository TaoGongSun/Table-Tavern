# 模型新版推出時跟上

Status: todo〔作者裁決 2026-10-06：立案，CLI 與 OpenRouter 兩件併一案〕

## Summary
兩件事：

1. **CLI 手動選過型號的人收不到新版**：檔位選「CLI 預設」時 app 不帶 `-m`，CLI 自己的預設模型由供應商遠端決定，本來就自動跟上（grok 4.7 即如此）。選了具體型號（例：三檔都存成 `grok-4.6`）就一直停在那版。做法：讀 CLI 模型清單時，發現 CLI 預設比玩家選的新，提醒一次「新版推出了，要換嗎？」，一鍵換或忽略；不自動替玩家換掉手選的型號。可沿用 `SmartFreeNewModelBanner` 那套提示。grok 的 `grok models` 有 `(default)` 標記；其他 CLI 的清單有沒有預設標記要查。
2. **OpenRouter 檔位預填寫死**：`src/features/settings/SettingsForm.tsx` 的 `SUGGESTED_TIER_MODELS`（claude-opus-4.8／claude-sonnet-5／gemini-3.5-flash）要發版才會變，玩家一存檔就固定。改成 app 執行時從官方來源決定（見 memory「智慧選模＝分數排行」：不靠人工清單、不寫死型號），已存的手選值同樣走第 1 點的提醒。

## Next action
2026-10-29 輕防重審：縮小〔模型判斷·未裁決〕（作者裁決只管立案與兩件併一案）。最小版＝OpenRouter 預填改成執行時從官方來源決定（不寫死型號）；CLI 提醒先只做 grok（`grok models` 有 `(default)` 標記）。開工先查 OpenRouter 官方 API 能否判斷同系列新版與付費檔位排行，再拍板提醒的位置與頻率。
刪掉的部分成已知限制：清單分不出預設版的 CLI 不提醒換新版。
