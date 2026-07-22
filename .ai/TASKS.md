# Project tasks

## In progress
- [mvp-7-packaging](tasks/mvp-7-packaging.md) — MVP 切片 7：打包 DMG＋README — 下一步：使用者以乾淨情境（另一台 Mac 或 AirDrop 補 quarantine）雙擊驗證 Gatekeeper 流程與 README 相符，通過後結案
- [mvp-6-onboarding](tasks/mvp-6-onboarding.md) — MVP 切片 6：Onboarding（BYOK 引導） — 下一步：使用者 UI 實測（搬走 ~/Documents/TableTavern 看首開範例桌；設定切 API 直連看引導面板），通過後結案
- [mvp-4-director](tasks/mvp-4-director.md) — MVP 切片 4：簡易導演（GM） — 下一步：使用者開 App 實測「GM 旁白」與「GM 推進」兩鈕，通過後 handoff complete＋task complete

## Todo
- [post-mvp-i18n-language-rule](tasks/post-mvp-i18n-language-rule.md) — MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入 — 下一步：等多語系功能開工時處理；屆時把 LANGUAGE_RULE 的注入改為依使用者語系設定條件化（設定檔需先有語系欄位）
- [post-mvp-character-archive](tasks/post-mvp-character-archive.md) — MVP 後：角色卡隱藏區（軟刪除）＋真刪除警告 — 下一步：等 MVP 驗收後開工；先定儲存形式（frontmatter 旗標 vs archived/ 子目錄），再依序做收起／還原／真刪除＋確認框
- [post-mvp-more-cli-providers](tasks/post-mvp-more-cli-providers.md) — MVP 後：擴充 CLI 訂閱供應商（gemini／grok，依偵測到的 CLI 決定） — 下一步：等 MVP 驗收後開工；第一步查證 gemini CLI（agy）與 grok CLI 的 headless 單發介面與模型列表取得方式（grok 先抄 Build-Collab-Board 的做法）
- [post-mvp-scene-summary](tasks/post-mvp-scene-summary.md) — MVP 後：場景切換＋場景摘要 — 下一步：等 MVP 驗收後開工；先實作換場鈕＋摘要生成單發呼叫，摘要存 world 目錄並在組裝時注入
- [post-mvp-st-import](tasks/post-mvp-st-import.md) — MVP 後第一優先：SillyTavern 角色卡匯入 — 下一步：等 MVP 切片 1–7 驗收後再開工；屆時先解析 V2 card spec 並寫欄位對應

## Blocked
- None.
