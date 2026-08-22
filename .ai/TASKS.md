# Project tasks

## In progress
- [api-cache-visibility](tasks/api-cache-visibility.md) — API 路快取看得見：多來源 usage 欄位＋「抓不到」與「沒中」分開顯示 — 下一步：真跑一輪 api 對話驗收：新行應帶 `cache_reporting: "reported"`＋`cached_tokens: 0`，額度分頁顯示 **0.0%** 而非「—」（tokenrouter 有回欄位、值真的是 0）。之後把 base_url 切回 OpenRouter、選一個有 `input_cache_read` 定價的模型（如 `deepseek/deepseek-v4-pro-0813` 或 `anthropic/` 系）跑幾輪，取得真實命中率與 `cache_write_tokens`——那批數據是保溫（C 案）每個設計參數的前提。
- [api-key-paste-guard](tasks/api-key-paste-guard.md) — 金鑰貼錯防呆：貼成文件裡的指令時當場提示，401 依傳輸分流指路 — 下一步：本案功能面已完成。剩「換上真金鑰後紅字消失、發言成功」這一項，會在使用者實際建立 OpenRouter 金鑰時自然驗到。
- [refactor-survey-spans](tasks/refactor-survey-spans.md) — 盤點四分類＋照搬零輸出：判官只出小抄（章＋分組＋命名權威），乾淨拆零呼叫 — 下一步：**T4 通用 2026-08-14 收工**：①取消在途＋Cmd-Q 無孤兒過（並行取消殺得乾淨、零孤兒、未完成不計費）、③舊產物相容過、④十語系面板骨架過、②API 退 GM 檔＝單元測試綠但 CLI 模式測不到，實機延後到哪天真用 API 模式時看 jsonl lane。refactor-dispatch 的 P4–P6 隨 ① 綠、P8 同 ② 延後。
- [refactor-mode-split](tasks/refactor-mode-split.md) — 重構雙軌定向：介面優先 vs 角色優先（兩段式選擇＋模式專屬解析） — 下一步：包 1–3 全部實作完成（2026-08-14 主線直寫，三 commit 25ca9cd／8c2ce17／a711287，cargo 490／vitest 134／build／i18n 十語系全綠）；剩包 4 實機驗收矩陣歸使用者實跑：WestFantsy／bcd368／Transfur／NorthHall／TrainEmperor（該被擋）＋同卡連跑三次，清單見交接檔 Remaining。
- [grok-profile-isolation](tasks/grok-profile-isolation.md) — grok 通道環境隔離：app 自帶 grok profile，不再吃使用者的 ~/.claude 與 ~/.grok — 下一步：剩使用者在設定頁跑一次 grok 登入（app profile 全新，與終端機 `~/.grok` 不共用），登入後模型下拉抓得到清單、旁白能正常發言即結案。
- [ai-card-refactor](tasks/ai-card-refactor.md) — AI 卡重構按鈕：整卡抽成機制格式＋介面本地化＋人物拆成角色卡 — 下一步：七包實作完成，2026-08-10 起實機驗收，開跑抓到三 bug（假檔停在舊契約→展開細看白畫面、匯入無驗證、AI 產的 HTML 殼被前端丟掉）已全修（vitest 82／build／i18n 綠，未 commit）；實測順序改成先做 `refactor-outcome-export`，再從 B 段真跑 orc-cave 卡、產物存檔後回頭跑 A 段，全過與 `person-promote` 兩案一起結案
- [person-promote](tasks/person-promote.md) — AI 認人並合併升格：把散在多條的同一角色併成一張角色卡 — 下一步：實作完成、四項自驗全綠（cargo 422／vitest 71／build／i18n，2026-08-08），等與 `ai-card-refactor` 的 A–E 一起實機驗收、兩案一起結案
- [state-values-mvu](tasks/state-values-mvu.md) — 狀態欄二期：機制格式（IR）＋本地權威數值＋觸發表 — 下一步：八包全部完成（2026-08-04，cargo test 317 綠）；真桌實跑延後至 `ai-card-refactor` 完成後合併驗收（2026-08-05 拍板），三處面板實機驗收照舊可先做
- [refactor-dispatch](tasks/refactor-dispatch.md) — AI 重構提速省費：展開並行＋展開下放檔位＋取消真停 — 下一步：包 1–3 實作完成（2026-08-11 三 commit，cargo 442／vitest 94／build／i18n 全綠）；2026-08-11 實機開跑：P1/P3 綠、P2 紅（~24 分）、P7-b 品質紅——提速與品質由 refactor-survey-spans 接手，剩 P4–P6/P8 等新案後合併驗
- [prompt-cache-optimization](tasks/prompt-cache-optimization.md) — 提示詞快取優化：resume 續聊架構（claude lane） — 下一步：本任務主體完成——包 1–7 全數實作並通過實機驗收（架構 85–88% 命中、額度分頁九項過、保溫 ping 94.6%）；2026-08-06 額度分頁改成「已省 X% 費用／約省下 $Y」口徑並實機看過；剩 grok／agy 顯示驗收延後與 OpenRouter 計量未接，見交接檔 Remaining
- [interface-card-panel](tasks/interface-card-panel.md) — 介面卡渲染面板：ST 介面卡原樣顯示（殼匯入＋沙盒面板） — 下一步：2026-08-04 v1 完成且**實機驗收全數通過**（匯入→開介面→點行動→送出→GM 照卡片格式回覆→介面就地換新畫面）；聊天收合 XML 已拍板不做；v2 首要＝省額度（歷史裡每輪整包 XML 重送，要留正文砍掉重複區塊）
- [i18n-more-languages](tasks/i18n-more-languages.md) — 介面擴充多語系（十國語言，AI 產字典） — 下一步：**改為全 app 功能定案後一次驗，現在不動手**（2026-08-17 拍板）。原驗收單已過期——基準 2026-07-30 是 247 鍵／55 顆按鈕（commit 9a17562），2026-08-17 已成 474 鍵／102 顆按鈕、期間 50 次 commit 動過 src/i18n/，最終要驗多少項無法預估，功能全數完工前不重排。機械關卡持續有效（缺鍵 TypeScript 編譯不過、`npm run check:i18n` 十語系佔位符與按鈕寬度全綠），缺的是人眼：07-30 之後新增的鍵未經逐語系審校，新功能畫面沒人切語系看過

## Todo
- [api-shared-lane](tasks/api-shared-lane.md) — API 路徑改走 chars 共線：讓換角色不再打散前綴快取 — 下一步：API 路徑的實機 runtime 驗收（要使用者在電腦前）：錯認前言者（只有 API 測得到，CLI 攤平後 role 就消失）＋四路快取成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens，codex 要先扣掉固定的 9,984。
- [refactor-card-png-export](tasks/refactor-card-png-export.md) — 重構卡 PNG 匯出：單檔圖卡＋含角色圖版＋套用映射地基 — 下一步：排程待定；開工首包＝套用映射持久化（refactor-outcome.json 擴充 envelope＋舊格式相容讀取），再做 #2/#3 PNG 封裝。
- [interface-scene-change](tasks/interface-scene-change.md) — 介面桌換幕：前情提要進介面正文槽、面板與狀態樹原樣續存 — 下一步：開工首步＝在西幻接管桌實測兩個【待實測】假設（換幕後檯面樹不變、前情提要落正文槽），結果回填底稿再分包
- [interface-takeover-spike](tasks/interface-takeover-spike.md) — 介面接管：重構把卡的每回合輸出格式照搬成骨架，app 用狀態樹組裝介面 — 下一步：逐型驗其他卡（MVU 前端型 bcd368 優先，見交接檔待辦 2），最後清舊產殼路線（待辦 4）；玩家選擇那條已移交 refactor-mode-split
- [no-cache-model-optout](tasks/no-cache-model-optout.md) — 零命中的模型不走共線：自動退回單角色組裝 — 下一步：開工前先重新立證：等帶 `cache_reporting: "reported"` 的 eligible zero 累積出來，確認真的有模型零命中。證據站得住再拍板規格檔的四項（solo 的 role 分配、要不要讓玩家看見、冷卻週期、與 usage-diag-non-claude 的先後）。
- [settings-overflow-i18n](tasks/settings-overflow-i18n.md) — 設定頁長字串爆版 — 下一步：挑一種排版方案（modal 加寬／列內換行／狀態按鈕移到次行），先在俄文與德文下驗連線分頁，再掃額度分頁與其餘八語系。
- [vendor-prefix-floor](tasks/vendor-prefix-floor.md) — 只中到供應商白送的那段，不該報成命中 — 下一步：排在 api-shared-lane 的四路成對測試之後開工——那批數據才估得準底線該怎麼定、以及這個功能還需不需要。開工首步是拍板底線的統計量（最小值／眾數／出現 ≥N 次的最小值）與「樣本不足就不判定」的 N。
- [vn-cg-generation](tasks/vn-cg-generation.md) — VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範 — 下一步：前置 `vn-mode` 已立案（2026-08-07），本任務為其 v3 分期；最省驗證＝拿一把 NAI token 打一發看出圖品質與回傳格式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：2026-08-07 討論立案完成（八項拍板＋三分期）；尚未排程，開工前置＝半天生圖實測 a／b／c 定管線，重點研究 NAI
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [shell-update-flash](tasks/shell-update-flash.md) — 卡片介面殼更新無閃白：postMessage ready 取代 load 事件重建雙緩衝 — 下一步：2026-08-12 立案；現行單 iframe 直繪是正確基線，開工前先實測閃白痛感（每回合一次、毫秒級）再定優先序
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [easy-pay-onboarding](tasks/easy-pay-onboarding.md) — 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值 — 下一步：遠期構想，等 BYOK 版初步測試後先做第一階段 OAuth；完整路線圖與合規前提見任務檔
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成且自驗綠，但本機無 DeepSeek／GLM／Kimi 訂閱可測，暫掛；等有相容端點的訂閱或協力者時再實測結案
- [character-to-player-card](tasks/character-to-player-card.md) — 角色卡升級成玩家卡（角色編輯頁的獨立入口） — 下一步：2026-08-10 立案；重構面板只在 AI 認人時問一次，之後改主意需要這條路，兩項待拍板（已有玩家卡時換不換、能不能反向取消）
- [character-presence](tasks/character-presence.md) — 角色在場/退場狀態管理：自動上下場＋在場過濾 — 下一步：2026-08-11 立案；地基見 CARD-REFACTOR-SPEC 包 4，開工前逐點重拍板，排序在 refactor-dispatch 之後

## Blocked
- None.
