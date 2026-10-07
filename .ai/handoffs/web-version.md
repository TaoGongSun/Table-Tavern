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
- D1–D18 全部拍板，見計畫第五節。〔作者裁決 2026-10-07〕

## 現況
- **包 1（最小縱切）封板**：commit be1093d..0de69dd，Sol、Grok 驗收通過，主線 verify 12 步綠、e2e 過。
  - `web/` 是獨立 Vite＋React 專案（自己的 package.json），`@desktop/*` 別名共用桌面版純 TS（D2）。
  - 內容：OpenRouter PKCE／貼金鑰、穩定免費選模與換模兩發（不試打）、範例卡「瑟拉」、串流含停止／停滯逾時／失控上限／送出互斥／取消兩分法、頁首常駐下載連結（GitHub Releases API，404 退 releases 頁）、今日免費次數與用完導流面板（只認每日證據，每分鐘限流不算）。對話只在記憶體。
  - repo 設定：根 vitest 排除 `web/**`、check-structure 也走 `web/src`、verify 加 web vitest＋build、verify.yml 加 `npm ci --prefix web`、STRUCTURE.md 有 web 一節。
- 包 1 未驗：真 OpenRouter（金鑰無額度 403）、正式網域部署（包 9）、Chromium 與手機實機、正文持續串流中按停止（只測過吐一段後卡住再停）。
- 桌面版尚未發過任何 GitHub release（`releases/latest` 回 404）。
- 測試用模型〔作者裁決 2026-10-07〕：網頁端用本機假端點，桌面端用測試通道＋claude CLI Sonnet；真免費模型實送等 OpenRouter 有額度再補，每包報告註明「真模型未實送」。
- 作者實測：OpenRouter 角色扮演排行前兩名免費模型都能輸出 NSFW（DeepSeek 較保守、GLM 很開放）。〔作者實測 2026-09-30〕

## 下一步：包 2（匯入卡）
先讀：計畫「二之二、格式契約」的卡片契約、「三、分包」表的包 2 列、「四、4.1 可沿用的底層」。

交付：
- 照卡片契約解析與分路：PNG 只讀 tEXt `chara` 優先、沒有才 `ccv3`；JSON 收 V1／V2／V3 並保留完整外殼；`character_book` 陣列形與物件形；世界書卡走世界書路，不能只走角色卡那條（照桌面版 `probe_import`，`src-tauri/src/import/card.rs:136`）。
- 選開場白（first_mes＋alternate_greetings）、玩家名、重新生成／編輯／刪除最後一則、錯誤說明。
- 提示組裝補成 ST 行為並釘一個 ST 對照版本：`mes_example` 依 `<START>` 切段、system_prompt／post_history_instructions／depth_prompt、送模前 regex（placement 含 1）與顯示 regex 分流、完整巨集引擎。
- 卡片正規化檢視：Rust 測試與 web vitest 對同一份黃金檔比對，並修桌面版 `import/card.rs:474` 的 `has_entries`——物件形 entries 一律算成有條目、照 uid 鍵展開，黃金檔不得收錄桌面版現行「掉進人設轉換」的結果。

注意：
- 黃金檔兩層 fixture：進 repo 的小型自製複合卡（角色＋內嵌世界書＋regex 介面＋MVU，含物件形 entries 與 iTXt 對照，比照 `scripts/harness-fixtures/mvu-write-probe.json`）讓 CI 跑；`TestCards/`（repo 根，已 gitignore、本機限定）的實卡本機跑。
- check-structure 會擋 `use` 開頭的 `.tsx`：hook 測試要用 JSX 就依行為命名（例：`send-cancel.test.tsx`）。
- 測試用的記憶體 Storage 在 `web/src/features/openrouter/memory-storage.ts`；Node 26 的全域 localStorage 在 happy-dom 底下是 undefined，要 `vi.stubGlobal`。
- e2e（`cd web && npm run e2e`）用 `vite build --mode e2e`；只有 e2e 模式接受 `VITE_TT_*` 端點覆寫（`web/src/shared/endpoints/resolve.ts`），一般建置一律官方網址。
- 包 1 的提示組裝是最小版（`web/src/features/chat/prompt.ts`）：巨集只換 `{{char}}`／`{{user}}`，玩家名預設「玩家」，介面文案與範例卡只有繁中（十語系在包 8）。
- 「取消未完成回合」自動收回玩家句是網頁版提案〔模型判斷·未裁決〕；桌面版零字停止是由玩家手動收回。

## 等作者
- 開 Cloudflare 帳號並接上 repo（D1），包 9 上線前要好。

## 之後再談
- 中文找卡：候選只有類腦、旅程兩個 Discord 社群（簡中、要答題、邀請連結會失效）。〔作者：之後再談，2026-09-30〕
