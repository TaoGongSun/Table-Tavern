# web-version — 網頁版：點開網址就能玩的免費引流入口

分支：`web-version`（已合 main 到 cb9583a）。計畫：[plans/web-version.md](../plans/web-version.md)（範圍、架構、格式契約、九包＋公開前門檻、查證結論、作者拍板 D1–D18、驗收方式）。

## 定位
點開網址就能玩的免費入口，用來引流：玩家玩完透過網頁版的連結下載桌面版。網頁版是漏斗入口，桌面版才是主力產品〔作者裁決 2026-10-07〕。

## 已定（照辦，不翻案）
- 網頁版與桌面版分開做，只保留基礎功能；網頁版不常更新，主力更新桌面版。〔作者裁決 2026-09-30〕
- 功能架構與頁面美感全新設計，不參考舊設計；只沿用使用者看不到的底層功能。〔作者裁決 2026-09-30〕
- 重構按鈕、快取命中／用量頁不進網頁版。〔作者裁決 2026-09-30〕
- 做卡片內嵌世界書的關鍵字觸發（只觸發，不做編輯器）。〔作者裁決 2026-09-30〕
- 網頁版存檔要能匯入桌面版；桌面版要新增對應的匯入。〔作者裁決 2026-09-30〕
- 支援卡片介面（原卡直玩的介面渲染與 MVU 變數），盡量做到跟 ST 一樣。〔作者裁決 2026-10-04〕
- 要有「去哪裡找卡」的導流；爬別站、另存別站卡的站（JannyAI、CharaVault、DeepSeek Tavern 等）不收。〔作者裁決 2026-09-30〕
- 網頁版的桌檔與卡片（角色卡＋內嵌世界書的複合卡）要能跟桌面版通用。〔作者裁決 2026-10-07〕
- D1–D23 全部拍板，見計畫第五節。〔作者裁決 2026-10-07〕

## 現況
- **包 1（最小縱切）封板**：commit be1093d..0de69dd，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/` 是獨立 Vite＋React 專案（自己的 package.json），`@desktop/*` 別名共用桌面版純 TS（D2）。
  - 內容：OpenRouter PKCE／貼金鑰、穩定免費選模與換模兩發（不試打）、範例卡「瑟拉」、串流含停止／停滯逾時／失控上限／送出互斥／取消兩分法、頁首常駐下載連結（GitHub Releases API，404 退 releases 頁）、今日免費次數與用完導流面板（只認每日證據，每分鐘限流不算）。對話只在記憶體。
  - repo 設定：根 vitest 排除 `web/**`、check-structure 也走 `web/src`、verify 加 web vitest＋build、verify.yml 加 `npm ci --prefix web`、STRUCTURE.md 有 web 一節。
- 包 1 未驗：真 OpenRouter（金鑰無額度 403）、正式網域部署（包 9）、Chromium 與手機實機、正文持續串流中按停止（只測過吐一段後卡住再停）。
- **包 2（匯入卡）封板**：commit 45baf78..52fcd59，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - 卡片契約：`src/shared/contracts/card-view/`（card-view.md、fixture、golden.json）；桌面版 `src-tauri/src/import/card_view_tests.rs` 與網頁版 `web/src/features/cards/card-view.ts` 對同一份黃金檔。條目展開一律走 `import::card::book_entries_keyed`。檢視含 `validity`，網頁版只收角色卡路有效的卡。
  - 網頁版 ST 行為（釘版本 SillyTavern `06bde939`）：`web/src/features/sillytavern/`（巨集引擎、變數、seedrandom、regex 三時機分流、mes_example 切段；時間巨集直接用 ST 同版 moment 2.30.1）；提示組裝 `web/src/features/chat/prompt.ts` 的 `composePrompt`（卡欄位→第 0 則寫回→其餘照 ST 先後）；每一發派送用該發模型與上限重組，副作用只在真正派送那次落地（`useChat.ts`）。
  - D21：卡內 regex 腳本預設不允許、匯入預覽有開關，結果存 `PlayCard.regexAllowed`（存檔要帶）。
  - personality 含 setvar 會跑兩次，照 ST（測試「a personality with setvar runs twice like ST」）。
  - TestCards 本機 35 檔 Rust／TS 檢視一致（`TT_CARD_VIEW_TESTCARDS`、`TT_CARD_VIEW_OUT`：先跑 cargo 的 `writes_local_testcard_views_when_requested` 再跑 web 的 card-view 測試）。
  - 未驗：真模型未實送；惡意卡 regex 的 ReDoS 風險同 ST（玩家允許後才套）；只測 WebKit；token 預算排包 4b（D23）。
- 桌面版尚未發過任何 GitHub release（`releases/latest` 回 404）。
- 測試用模型〔作者裁決 2026-10-07〕：網頁端用本機假端點，桌面端用測試通道＋claude CLI Sonnet；真免費模型實送等 OpenRouter 有額度再補，每包報告註明「真模型未實送」。
- 作者實測：OpenRouter 角色扮演排行前兩名免費模型都能輸出 NSFW（DeepSeek 較保守、GLM 很開放）。〔作者實測 2026-09-30〕

## 下一步：包 3（桌檔契約＋桌面版匯入器）
照計畫三分包表包 3、二之二「桌檔契約」與「落地規則」、第五節 D3／D13／D15／D16／D18／D21 施工。

接手者需知：
- check-structure 會擋 `use` 開頭的 `.tsx`：hook 測試要用 JSX 就依行為命名（例：`send-cancel.test.tsx`）。用到 DOMPurify 的測試要 `// @vitest-environment happy-dom`。
- 測試用的記憶體 Storage 在 `web/src/features/openrouter/memory-storage.ts`；Node 26 的全域 localStorage 在 happy-dom 底下是 undefined，要 `vi.stubGlobal`。
- e2e（`cd web && npm run e2e`）用 `vite build --mode e2e`；只有 e2e 模式接受 `VITE_TT_*` 端點覆寫（`web/src/shared/endpoints/resolve.ts`）。假端點 `chat: "numbered"` 回「第 N 次回覆」。
- 改契約＝桌面版、網頁版、golden.json 同一筆 commit；golden 可用上面兩個環境變數讓 Rust 寫出各 fixture 的檢視再組回。
- ST 巨集測試向量 `web/src/features/sillytavern/st-macro-cases.json` 抽自 ST 06bde939 `tests/frontend/MacroEngine.e2e.js`（只收內建巨集）。
- serde_json 開了 preserve_order（schemars 帶進來），Map 是原序，需要排序的地方自己排。
- 「取消未完成回合」自動收回玩家句是網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是由玩家手動收回。介面文案與範例卡只有繁中（十語系在包 8）。

## 等作者
- 開 Cloudflare 帳號並接上 repo（D1），包 9 上線前要好。

## 之後再談
- 中文找卡：候選只有類腦、旅程兩個 Discord 社群（簡中、要答題、邀請連結會失效）。〔作者：之後再談，2026-09-30〕
