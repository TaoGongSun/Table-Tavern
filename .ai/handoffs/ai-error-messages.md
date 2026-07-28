# Handoff: ai-error-messages

## Current state
額度／未登入兩類的人話文案已實作，`npm run build` 綠、分類器 13 例樣本測試全對。GUI 未實跑（要真的打爆額度才會觸發），等使用者實機看一眼。

## Completed
- i18n zh/en 兩鍵：`errQuota`（額度用完，請換來源或等重置）、`errAuth`（未登入或憑證過期，到設定重新連線）——src/i18n.ts:279、src/i18n.ts:556
- 分類器 `explainAiError(raw)`：兩條正則比對，命中回鍵名、認不出來回 null 走原本文案——src/App.tsx:248
- 對話錯誤列改用 `<ErrorNote>` 元件（命中顯示人話＋原文小字，未命中原樣輸出），取代原本五處 `<p role="alert">{error}</p>`——src/App.tsx:255
- 生圖失敗主句改成 `t(explainAiError(aiGenError) ?? "aiGenFailed")`，原始錯誤小字不動——src/App.tsx:2178

## Verification
- `npm run build` exit 0（tsc + vite 皆綠）
- 分類器樣本測試 13 例全對：Grok「free Grok Build usage limit」／OpenRouter 402 Insufficient credits／Claude「usage limit reached」／Codex「hit your usage limit」／Gemini 429 RESOURCE_EXHAUSTED／OpenRouter 429 rate limit → 全判額度；「尚未設定 API key」／「Not logged in」／401 No auth credentials → 全判未登入；空名 invalid name／檔案 permission denied／CLI 逾時／內容政策拒繪 → 全走保底原文（零誤判）

## Remaining
- 使用者實機看一眼（Grok 免費額度目前已用完，重按一次 AI 生成即可看到新文案）
- 內容政策拒繪（「violates content policy」類）目前走保底原文，分流留給 sponsor-features 待討論議程拍板後再加一類

## Next action
- 等實機回報；若文案要調字面，只改 i18n 兩鍵即可

## Constraints
- 關鍵字比對只在前端顯示層，後端錯誤原文一律完整保留在小字。
- 不做自動換來源（花玩家自己帳號的錢，決定權留給玩家）；403 有歧義（既可能額度也可能權限）刻意不列入任一類。
