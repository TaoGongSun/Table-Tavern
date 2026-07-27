# Project tasks

## In progress
- [drag-reorder-lists](tasks/drag-reorder-lists.md) — 角色卡與世界書條目改拖曳排序（GM 固定最上、刪 ↑↓ 鈕、列不可反白） — 下一步：三件實作完成（cargo 96 綠＋npm build 綠），禁反白已實測通過，等使用者實測拖曳手感後結案
- [cli-detect-state](tasks/cli-detect-state.md) — CLI 偵測空窗期：加「正在偵測…」三態＋四支並行＋結果快取 — 下一步：實作完成（cargo 93 綠＋npm build 綠＋並行實測 2.8 倍），等使用者實測設定頁重開不再閃「一鍵安裝」後結案
- [character-card-avatar-issues](tasks/character-card-avatar-issues.md) — 角色卡回饋修訂＋建卡流程改版＋刪除入口＋GM 卡 — 下一步：全數實作完成且使用者實測通過（GM 卡書皮方案七主題驗收，Codex 生圖取消），移除全身圖警告已補（d871f55）；剩改名提示複驗＋圖庫路徑 bug 擱置等 stable-id-storage 拍板
- [sponsor-features](tasks/sponsor-features.md) — 贊助三件組：作者頁（贊助連結）＋5 主題配色＋AI 生成角色圖 — 下一步：三項＋生成歷史圖庫實作完成（cargo 91 綠＋build 綠），等實測圖庫；待拍板：提示詞標籤、Ko-fi 導購歧義
- [cli-connected-badge](tasks/cli-connected-badge.md) — CLI 已連結狀態記憶：按鈕換「已連結 ✓」＋不重發登入 — 下一步：實作完成（cargo 77 綠＋npm build 綠），等使用者以新 DMG 實測後結案
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成（cargo 77 綠＋npm build 綠），等使用者實測設定頁進階區＋實聊後結案
- [ui-overhaul](tasks/ui-overhaul.md) — UI 全面改版：Emblem 設計系統（桌遊組件卡＋playbill 對話＋token 化） — 下一步：第一輪實作完成（npm build 綠＋淺色實跑驗證），淺色主題去黃調色已驗收定案，等英文合作者實機驗收深色模式與實聊 playbill
- [cli-install-windows](tasks/cli-install-windows.md) — CLI 一鍵安裝 Windows 支援（PowerShell 分支） — 下一步：verify 綠（run 30165056516）＋打包出爐（run 30165448004，commit 194fb86 含四項 UX 修正），Mac DMG 同步重打（00:22 版），等轉交測試者回報
- [cli-install-all-providers](tasks/cli-install-all-providers.md) — CLI 一鍵安裝擴充到 claude／codex／grok（比照 agy） — 下一步：實作完成（cargo 70 綠＋npm build 綠），等使用者實測安裝鈕後結案，再重打包
- [test-build-cross-platform](tasks/test-build-cross-platform.md) — 測試版打包：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（CI 未簽章） — 下一步：兩平台產物已出爐並自驗通過，等使用者實機驗收（MacBook Air 測 DMG、真 Windows 機裝 artifact）後結案

## Done

見 [DONE.md](DONE.md)（16 項）。

## Todo
- [stable-id-storage](tasks/stable-id-storage.md) — 儲存改用穩定代碼定址（桌與角色的路徑是代碼，顯示名是欄位） — 下一步：**等 fable／使用者回答任務檔裡的四題**（劇情紀錄發言者存什麼／範圍／代碼格式／舊資料處理），再寫實作計畫；採用的話「生成圖庫目錄放錯層」那個 bug 不用先修
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 下一步：等發佈產線（release-1、release-2）打通後開工：先定主題檔格式與載入引擎＋基礎白色主題，再產五套贊助包資產與自選桌布功能；AI 產生主題（prompt 模板＋BYOK 產圖）排最後，v1 可不上
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：使用者操作：PayPal 升級商業帳戶 → 開 Ko-fi 帳號並「切至 Free 檔」（新帳號預設 Contributor 檔抽 5%）→ 建 Shop 商品（10 美元主題包，檔案自動發貨）→ 提領設 USD 進玉山外匯戶自行換匯。商品檔案本身等 release-4-theme-pack 產出
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：定目標語系清單與字典品質驗證流程，再一次擴 i18n／範例桌／LANGUAGE_RULE
- [cli-auto-connect](tasks/cli-auto-connect.md) — CLI 自動連接：背景偵測＋登入跳轉自動回 — 下一步：查證 claude／codex CLI 的登入觸發與完成偵知介面，再定 UX 流程（風險告知仍前置）

## Blocked
- None.
