# Project tasks

## In progress
- [mvp-7-packaging](tasks/mvp-7-packaging.md) — MVP 切片 7：打包 DMG＋README — 下一步：使用者以乾淨情境（另一台 Mac 或 AirDrop 補 quarantine）雙擊驗證 Gatekeeper 流程與 README 相符，通過後結案
- [mvp-6-onboarding](tasks/mvp-6-onboarding.md) — MVP 切片 6：Onboarding（BYOK 引導） — 下一步：使用者 UI 實測（搬走 ~/Documents/TableTavern 看首開範例桌；設定切 API 直連看引導面板），通過後結案
- [mvp-4-director](tasks/mvp-4-director.md) — MVP 切片 4：簡易導演（GM） — 下一步：使用者開 App 實測「GM 旁白」與「GM 推進」兩鈕，通過後 handoff complete＋task complete

## Todo
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，本機驗證通過後把憑證交接給 release-2 的 CI secrets
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 下一步：等發佈產線（release-1、release-2）打通後開工：先定主題檔格式與載入引擎＋基礎白色主題，再產五套贊助包資產與自選桌布功能；AI 產生主題（prompt 模板＋BYOK 產圖）排最後，v1 可不上
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：使用者操作：PayPal 升級商業帳戶 → 開 Ko-fi 帳號並「切至 Free 檔」（新帳號預設 Contributor 檔抽 5%）→ 建 Shop 商品（10 美元主題包，檔案自動發貨）→ 提領設 USD 進玉山外匯戶自行換匯。商品檔案本身等 release-4-theme-pack 產出
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：等 release-1-mac-signing 的憑證就緒後開工：寫 tauri-action workflow，Developer ID .p12 與公證 API key 進 CI secrets；Windows 產物由協力者在乾淨 Windows 機驗收（下載→安裝→啟動，記錄 SmartScreen 實況）
- [post-mvp-i18n-language-rule](tasks/post-mvp-i18n-language-rule.md) — MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入 — 下一步：等多語系功能開工時處理；屆時把 LANGUAGE_RULE 的注入改為依使用者語系設定條件化（設定檔需先有語系欄位）
- [post-mvp-character-archive](tasks/post-mvp-character-archive.md) — MVP 後：角色卡隱藏區（軟刪除）＋真刪除警告 — 下一步：等 MVP 驗收後開工；先定儲存形式（frontmatter 旗標 vs archived/ 子目錄），再依序做收起／還原／真刪除＋確認框
- [post-mvp-more-cli-providers](tasks/post-mvp-more-cli-providers.md) — MVP 後：擴充 CLI 訂閱供應商（gemini／grok，依偵測到的 CLI 決定） — 下一步：等 MVP 驗收後開工；第一步查證 gemini CLI（agy）與 grok CLI 的 headless 單發介面與模型列表取得方式（grok 先抄 Build-Collab-Board 的做法）
- [post-mvp-scene-summary](tasks/post-mvp-scene-summary.md) — MVP 後：場景切換＋場景摘要 — 下一步：等 MVP 驗收後開工；先實作換場鈕＋摘要生成單發呼叫，摘要存 world 目錄並在組裝時注入
- [post-mvp-st-import](tasks/post-mvp-st-import.md) — MVP 後第一優先：SillyTavern 角色卡匯入 — 下一步：等 MVP 切片 1–7 驗收後再開工；屆時先解析 V2 card spec 並寫欄位對應

## Blocked
- None.
