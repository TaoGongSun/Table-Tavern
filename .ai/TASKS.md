# Project tasks

## In progress
- [sample-world-i18n](tasks/sample-world-i18n.md) — 範例桌內容依語系產生（首開先選語言） — 下一步：程式碼完成（首開語言選擇畫面＋en 範例桌，cargo test 41 綠＋npm build 綠），剩使用者模擬首開實測即結案
- [ui-settings-panel](tasks/ui-settings-panel.md) — 設定視窗：單一入口內分頁（外觀預設／AI 連線） — 下一步：程式碼完成（modal 兩分頁＋文字大小偏好，npm build 綠），剩使用者視覺驗收即結案

## Done
- [ui-layout-rework](tasks/ui-layout-rework.md) — 版面重構：角色卡移左側欄＋桌列表可摺疊（NewPlan §9.4） — 2026-07-23 使用者視覺驗收通過，結案
- [transcript-export](tasks/transcript-export.md) — 一鍵下載跑團紀錄（劇情歷史匯出） — 2026-07-23 使用者實測另存對話框存檔通過，結案
- [post-mvp-st-import](tasks/post-mvp-st-import.md) — MVP 後第一優先：SillyTavern 角色卡匯入（含存 PNG＋角色圖顯示/隱藏） — 2026-07-23 使用者實測匯入範例卡＋角色圖開關通過（附截圖），結案
- [ui-i18n-switch](tasks/ui-i18n-switch.md) — UI 語系切換（zh-TW／en） — 2026-07-23 前端 i18n 字典＋語言下拉、後端 LANGUAGE_RULE 依語系注入，npm build＋cargo test 全綠，結案
- [post-mvp-i18n-language-rule](tasks/post-mvp-i18n-language-rule.md) — MVP 後：多語系時 LANGUAGE_RULE 改依使用者語系注入 — 2026-07-23 隨 ui-i18n-switch 完成，結案
- [mvp-7-packaging](tasks/mvp-7-packaging.md) — MVP 切片 7：打包 DMG＋README — 2026-07-22 實測 Gatekeeper，修掉 linker-signed 被判「已損毀」的缺陷＋README 步驟更新，結案（公證移交 release-1）
- [mvp-6-onboarding](tasks/mvp-6-onboarding.md) — MVP 切片 6：Onboarding（BYOK 引導） — 2026-07-22 使用者實測首開範例桌＋BYOK 面板通過，另修冪等／幣別文案／按鈕間距，結案
- [mvp-4-director](tasks/mvp-4-director.md) — MVP 切片 4：簡易導演（GM） — 2026-07-22 使用者實測 world.md 編輯／GM 旁白／GM 推進全通過，結案

## Todo
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，本機驗證通過後把憑證交接給 release-2 的 CI secrets
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 下一步：等發佈產線（release-1、release-2）打通後開工：先定主題檔格式與載入引擎＋基礎白色主題，再產五套贊助包資產與自選桌布功能；AI 產生主題（prompt 模板＋BYOK 產圖）排最後，v1 可不上
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：使用者操作：PayPal 升級商業帳戶 → 開 Ko-fi 帳號並「切至 Free 檔」（新帳號預設 Contributor 檔抽 5%）→ 建 Shop 商品（10 美元主題包，檔案自動發貨）→ 提領設 USD 進玉山外匯戶自行換匯。商品檔案本身等 release-4-theme-pack 產出
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：等 release-1-mac-signing 的憑證就緒後開工：寫 tauri-action workflow，Developer ID .p12 與公證 API key 進 CI secrets；Windows 產物由協力者在乾淨 Windows 機驗收（下載→安裝→啟動，記錄 SmartScreen 實況）
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：定目標語系清單與字典品質驗證流程，再一次擴 i18n／範例桌／LANGUAGE_RULE
- [cli-auto-connect](tasks/cli-auto-connect.md) — CLI 自動連接：背景偵測＋登入跳轉自動回 — 下一步：查證 claude／codex CLI 的登入觸發與完成偵知介面，再定 UX 流程（風險告知仍前置）
- [post-mvp-scene-summary](tasks/post-mvp-scene-summary.md) — MVP 後：場景切換＋場景摘要 — 下一步：先實作換場鈕＋摘要生成單發呼叫，摘要存 world 目錄並在組裝時注入
- [post-mvp-character-archive](tasks/post-mvp-character-archive.md) — MVP 後：角色卡隱藏區（軟刪除）＋真刪除警告 — 下一步：先定儲存形式（frontmatter 旗標 vs archived/ 子目錄），再依序做收起／還原／真刪除＋確認框
- [post-mvp-more-cli-providers](tasks/post-mvp-more-cli-providers.md) — MVP 後：擴充 CLI 訂閱供應商（gemini／grok，依偵測到的 CLI 決定） — 下一步：查證 gemini CLI（agy）與 grok CLI 的 headless 單發介面與模型列表取得方式（grok 先抄 Build-Collab-Board 的做法）

## Blocked
- None.
