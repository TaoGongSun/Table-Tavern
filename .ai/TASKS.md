# Project tasks

## In progress
- [sponsor-features](tasks/sponsor-features.md) — 贊助三件組：作者頁（贊助連結）＋5 主題配色＋AI 生成角色圖 — 下一步：三項＋生成歷史圖庫全部實作並實測通過（圖庫 2026-07-28 驗收）；剩「匯入贊助包入口」等贊助包格式定案，另待拍板：提示詞標籤、Ko-fi 導購歧義
- [ui-overhaul](tasks/ui-overhaul.md) — UI 全面改版：Emblem 設計系統（桌遊組件卡＋playbill 對話＋token 化） — 下一步：第一輪實作完成（npm build 綠＋淺色實跑驗證），淺色主題去黃調色已驗收定案，等英文合作者實機驗收深色模式與實聊 playbill
- [cli-install-windows](tasks/cli-install-windows.md) — CLI 一鍵安裝 Windows 支援（PowerShell 分支） — 下一步：verify 綠（run 30165056516）＋打包出爐（run 30165448004，commit 194fb86 含四項 UX 修正），Mac DMG 同步重打（00:22 版），等轉交測試者回報；併驗 cli-connected-badge 移交過來的 grok 新探針與 pre_probe
- [test-build-cross-platform](tasks/test-build-cross-platform.md) — 測試版打包：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（CI 未簽章） — 下一步：兩平台產物已出爐並自驗通過，等使用者實機驗收（MacBook Air 測 DMG、真 Windows 機裝 artifact）後結案

## Done

見 [DONE.md](DONE.md)（23 項）。

## Todo
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成且自驗綠，但本機無 DeepSeek／GLM／Kimi 訂閱可測，暫掛；等有相容端點的訂閱或協力者時再實測結案
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 下一步：使用者要求提前做「贊助解鎖檔案」——定贊助包檔案格式與 app 認證方式，取代 sponsor-features 現用的手改旗標並補上匯入入口；之後才做主題引擎與五套資產
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：使用者操作：PayPal 升級商業帳戶 → 開 Ko-fi 帳號並「切至 Free 檔」（新帳號預設 Contributor 檔抽 5%）→ 建 Shop 商品（10 美元主題包，檔案自動發貨）→ 提領設 USD 進玉山外匯戶自行換匯。商品檔案本身等 release-4-theme-pack 產出
- [theme-pack-component-skins](tasks/theme-pack-component-skins.md) — 主題包元件裝飾：華麗發言送出鈕（贊助包 v2 回購誘因） — 下一步：等 release-4-theme-pack 主題檔格式定案時預留「元件裝飾」schema，v1 不實作
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：定目標語系清單與字典品質驗證流程，再一次擴 i18n／範例桌／LANGUAGE_RULE
- [cli-auto-connect](tasks/cli-auto-connect.md) — CLI 自動連接：背景偵測＋登入跳轉自動回 — 下一步：查證 claude／codex CLI 的登入觸發與完成偵知介面，再定 UX 流程（風險告知仍前置）

## Blocked
- None.
