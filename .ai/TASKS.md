# Project tasks

## In progress
- [sponsor-features](tasks/sponsor-features.md) — 贊助三件組：作者頁（贊助連結）＋5 主題配色＋AI 生成角色圖 — 下一步：三項＋生成歷史圖庫全部實作並實測通過（圖庫 2026-07-28 驗收）；匯入贊助包入口已由 release-4 補上（2026-07-28）；待討論議程三項全數結案（提示詞標籤 07-30、生圖失敗訊息分流 08-01、Ko-fi 導購歧義 08-03 連結已直指商品頁），唯一剩餘＝使用者實測三項
- [release-3-kofi](tasks/release-3-kofi.md) — 發佈 3：Ko-fi 開帳與金流（多為使用者本人操作） — 下一步：商品頁已上線（2026-07-30，連結已進 app）；剩確認 .ttpack 掛檔發貨、說明文案定稿、首筆提領實測
- [ui-overhaul](tasks/ui-overhaul.md) — UI 全面改版：Emblem 設計系統（桌遊組件卡＋playbill 對話＋token 化） — 下一步：第一輪實作完成（npm build 綠＋淺色實跑驗證），淺色主題去黃調色已驗收定案，等實機驗收深色模式與實聊 playbill
- [test-build-cross-platform](tasks/test-build-cross-platform.md) — 測試版打包：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（CI 未簽章） — 下一步：2026-08-02 以 HEAD ffd447b 重打兩平台並自驗通過，等使用者實機驗收（MacBook Air 測 DMG、真 Windows 機裝 artifact）後結案

- [worldbook-card-import](tasks/worldbook-card-import.md) — 世界書卡（PNG）匯入＋條目就地展開編輯＋純世界書開局 — 下一步：四項實作皆自驗全綠（cargo test 151、真卡 17 條煙霧、tsc＋i18n 檢查），2026-08-02 補修空桌回收誤刪世界書（判空納入 worldbook.json），等使用者實機驗收（含零角色匯入自動選 GM、條目切換自動存、匯完世界書切桌不被刪）後結案
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：十語系三處全部上齊、npm build 與 cargo test 116 全綠，等實機逐語系看畫面驗收；原四件待拍板（日文世界書用詞、範例桌地名處理等）已於 2026-07-30 全數拍板
- [undo-last-message](tasks/undo-last-message.md) — 收回上一句（一次一則、可連按往回收；復原同樣可連按逐則倒回） — 下一步：實作＋三項自驗全綠（cargo test 127、npm build、check:i18n），等使用者實機驗收六項後結案
- [ai-table-generator](tasks/ai-table-generator.md) — 一句話開桌：AI 生成世界觀＋角色（免費基礎功能） — 下一步：三塊實作完成、自驗全綠（cargo test 133、build、check:i18n），等使用者實機驗收四項後結案
- [st-ecosystem-upgrades](tasks/st-ecosystem-upgrades.md) — SillyTavern 生態升級：匯入補強＋巨集替換＋訊息 Markdown＋GM 狀態欄＋條目互轉＋開場白 — 下一步：六項全部實作完成（cargo test 174 綠）；第一、二、三、五、六項與狀態列條件顯示（只給帶狀態列規則的桌）2026-08-03 實機驗收通過，剩第四項狀態欄後續鏈（GM 更新／手改／收回倒回／壞格式）待驗
- [prompt-cache-optimization](tasks/prompt-cache-optimization.md) — 提示詞快取優化：resume 續聊架構（claude lane） — 下一步：實機驗收（包 6 額度分頁八項＋包 7 保溫計時器，清單見交接檔 Verification）；包 1–7 程式面全部完成、架構驗收已過

## Done

見 [DONE.md](DONE.md)（29 項）。

## Todo
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成且自驗綠，但本機無 DeepSeek／GLM／Kimi 訂閱可測，暫掛；等有相容端點的訂閱或協力者時再實測結案
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [easy-pay-onboarding](tasks/easy-pay-onboarding.md) — 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值 — 下一步：遠期構想，等 BYOK 版初步測試後先做第一階段 OAuth；完整路線圖與合規前提見任務檔

## Blocked
- None.
