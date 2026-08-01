# Project tasks

## In progress
- [sponsor-features](tasks/sponsor-features.md) — 贊助三件組：作者頁（贊助連結）＋5 主題配色＋AI 生成角色圖 — 下一步：三項＋生成歷史圖庫全部實作並實測通過（圖庫 2026-07-28 驗收）；匯入贊助包入口已由 release-4 補上（2026-07-28），另待拍板：提示詞標籤、Ko-fi 導購歧義
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：商品頁已上線（2026-07-30，連結已進 app）；剩確認 .ttpack 掛檔發貨、說明文案定稿、首筆提領實測
- [release-4-theme-pack](tasks/release-4-theme-pack.md) — 發佈 4：佈景主題引擎＋贊助包（回禮內容） — 下一步：.ttpack 驗收通過、商品頁連結已換（2026-07-30）；接著主題引擎：先拍板主題檔格式（預留元件裝飾 schema）
- [ui-overhaul](tasks/ui-overhaul.md) — UI 全面改版：Emblem 設計系統（桌遊組件卡＋playbill 對話＋token 化） — 下一步：第一輪實作完成（npm build 綠＋淺色實跑驗證），淺色主題去黃調色已驗收定案，等英文合作者實機驗收深色模式與實聊 playbill
- [cli-install-windows](tasks/cli-install-windows.md) — CLI 一鍵安裝 Windows 支援（PowerShell 分支） — 下一步：07-31 系統代理自動下傳＋兩條失敗白話提示已實作自驗綠（Grok 回報者兩案：鎖檔＋牆內空白登入視窗），等 CI verify→重打包→回報者只開系統代理走安裝→登入→聊天全流程
- [test-build-cross-platform](tasks/test-build-cross-platform.md) — 測試版打包：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（CI 未簽章） — 下一步：2026-08-01 以 HEAD 68a28ea 重打兩平台並自驗通過，等使用者實機驗收（MacBook Air 測 DMG、真 Windows 機裝 artifact）後結案

- [worldbook-card-import](tasks/worldbook-card-import.md) — 世界書卡（PNG）匯入＋條目就地展開編輯＋純世界書開局 — 下一步：四項實作皆自驗全綠（cargo test 117、真卡 17 條煙霧、tsc＋i18n 檢查），等使用者實機驗收（含零角色匯入自動選 GM、條目切換自動存）後結案
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：十語系三處全部上齊、npm build 與 cargo test 116 全綠，等實機逐語系看畫面驗收；另有四件待拍板（日文世界書用詞、範例桌地名處理不統一等）
- [undo-last-message](tasks/undo-last-message.md) — 收回上一句（一次一則、可連按往回收；復原同樣可連按逐則倒回） — 下一步：實作＋三項自驗全綠（cargo test 127、npm build、check:i18n），等使用者實機驗收六項後結案

## Done

見 [DONE.md](DONE.md)（25 項）。

## Todo
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成且自驗綠，但本機無 DeepSeek／GLM／Kimi 訂閱可測，暫掛；等有相容端點的訂閱或協力者時再實測結案
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [theme-pack-component-skins](tasks/theme-pack-component-skins.md) — 主題包元件裝飾：華麗發言送出鈕（贊助包 v2 回購誘因） — 下一步：等 release-4-theme-pack 主題檔格式定案時預留「元件裝飾」schema，v1 不實作
- [cli-auto-connect](tasks/cli-auto-connect.md) — CLI 自動連接：背景偵測＋登入跳轉自動回 — 下一步：查證 claude／codex CLI 的登入觸發與完成偵知介面，再定 UX 流程（風險告知仍前置）
- [easy-pay-onboarding](tasks/easy-pay-onboarding.md) — 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值 — 下一步：遠期構想，等 BYOK 版初步測試後先做第一階段 OAuth；完整路線圖與合規前提見任務檔

## Blocked
- None.
