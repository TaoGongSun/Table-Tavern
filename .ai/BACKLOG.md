# 待辦清單

還沒開工的案子。手寫維護，一條一行；立案說明在各自的 [tasks/](tasks/)`<id>.md`。
開工＝把該檔搬進 [handoffs/](handoffs/) 並在 [HANDOFF.md](HANDOFF.md) 登記一條，本檔那行刪掉。
已在進行中的看 [HANDOFF.md](HANDOFF.md)，等實機驗收的看 [實測佇列](reference/verification-queue.md)。

- [api-shared-lane](tasks/api-shared-lane.md) — API 路徑改走 chars 共線：讓換角色不再打散前綴快取 — 下一步：API 路徑的實機 runtime 驗收（要使用者在電腦前）：錯認前言者（只有 API 測得到，CLI 攤平後 role 就消失）＋四路快取成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens，codex 要先扣掉固定的 9,984。
- [card-arrival-private-leak](tasks/card-arrival-private-leak.md) — 角色卡回歸事件把私設漏給同桌其他角色 — 下一步：先拍板「回歸事件該讓誰看到什麼」：是拆成公開回歸事件＋GM-only 私設事件，還是回歸事件只留公開設定。定了再看四條路各要怎麼改，並一併決定 grok 現在的「一角一線＋私設提進凍結 system」要保留還是改回共線——grok-cache-miss 的角色線驗收擋在這裡。
- [refactor-card-png-export](tasks/refactor-card-png-export.md) — 重構卡 PNG 匯出：單檔圖卡＋含角色圖版＋套用映射地基 — 下一步：排程待定；開工首包＝套用映射持久化（refactor-outcome.json 擴充 envelope＋舊格式相容讀取），再做 #2/#3 PNG 封裝。
- [interface-scene-change](tasks/interface-scene-change.md) — 介面桌換幕：前情提要進介面正文槽、面板與狀態樹原樣續存 — 下一步：開工首步＝在西幻接管桌實測兩個【待實測】假設（換幕後檯面樹不變、前情提要落正文槽），結果回填底稿再分包
- [interface-takeover-spike](tasks/interface-takeover-spike.md) — 介面接管：重構把卡的每回合輸出格式照搬成骨架，app 用狀態樹組裝介面 — 下一步：逐型驗其他卡（MVU 前端型 bcd368 優先，見交接檔待辦 2），最後清舊產殼路線（待辦 4）；玩家選擇那條已移交 refactor-mode-split
- [no-cache-model-optout](tasks/no-cache-model-optout.md) — 零命中的模型不走共線：自動退回單角色組裝 — 下一步：開工前先重新立證：等帶 `cache_reporting: "reported"` 的 eligible zero 累積出來，確認真的有模型零命中。證據站得住再拍板規格檔的四項（solo 的 role 分配、要不要讓玩家看見、冷卻週期、與 usage-diag-non-claude 的先後）。
- [chars-lane-rewrite-drop](tasks/chars-lane-rewrite-drop.md) — 角色線續聊被作廢：每輪冷開、快取一次都中不到 — 下一步：開工首步＝重現並定位：連續讓角色發言兩三輪，看每輪是不是都落 drop-lane，再確認 `apply_rewrite` 裡失敗的是 `find_user_line_with_segment`／`erase_user_segment`／`prefix_last_assistant` 哪一段。目前錯誤被 `Err(_)` 吞掉不落原因，可能要先讓它把失敗原因寫進帳本才查得動。
- [long-prompt-scene-hint](tasks/long-prompt-scene-hint.md) — 桌子太長撞到指令長度上限時，請玩家換幕 — 下一步：先確認撞上限時各條路實際回什麼（作業系統層的 E2BIG？CLI 自己的錯誤？還是直接沒反應），才知道要抓什麼特徵。三個作業系統的上限與表現可能不同。
- [settings-overflow-i18n](tasks/settings-overflow-i18n.md) — 設定頁長字串爆版 — 下一步：挑一種排版方案（modal 加寬／列內換行／狀態按鈕移到次行），先在俄文與德文下驗連線分頁，再掃額度分頁與其餘八語系。
- [non-claude-real-cache](tasks/non-claude-real-cache.md) — codex／agy／OpenRouter 沒有續聊，快取到底有沒有真的抓到 — 下一步：照規格檔實作三包：包 1 `CacheStrategy` 判定與帳本欄位；包 2 尾巴重播（`TranscriptEvent` 新欄位、GM 線與角色線組裝改寫、`<turn-context>` 包裝與 system 規則、十語系文案）；包 3 chain epoch 的重開條件。驗收看離線重算的 byte-LCP 要等於 100%，再實跑三輪看 `cached_tokens` 是否跟著上一輪的 `prompt_tokens` 走。
- [grok-cache-miss](tasks/grok-cache-miss.md) — Grok 快取命中率從九成掉到 2% — 下一步：等 card-arrival-private-leak 拍板角色線怎麼組裝，再驗收角色線：讓角色接三輪以上話，看 `chars:grok-4.6:<角色 id>` 的 cached_tokens 隨對話增長，並確認換角色、改卡、換幕之後不會每輪重開。GM 線已驗完，不必重驗。
- [vendor-prefix-floor](tasks/vendor-prefix-floor.md) — 只中到供應商白送的那段，不該報成命中 — 下一步：排在 api-shared-lane 的四路成對測試之後開工——那批數據才估得準底線該怎麼定、以及這個功能還需不需要。開工首步是拍板底線的統計量（最小值／眾數／出現 ≥N 次的最小值）與「樣本不足就不判定」的 N。
- [ai-connection-provider-panels](tasks/ai-connection-provider-panels.md) — AI 連線設定重整：OpenRouter 免費推薦＋供應商專屬面板 — 下一步：開工時先做 UI／資料契約切分：把目前 SettingsForm 內共用的 tier UI 改成 provider-specific 區塊；同時定義 OpenRouter 推薦 manifest 與本機 fallback 格式，再接現有 `/api/v1/models` 清單。
- [easy-pay-onboarding](tasks/easy-pay-onboarding.md) — 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值 — 下一步：遠期構想；若開工，先與 `ai-connection-provider-panels` 對齊 OpenRouter panel，再做第一階段 OAuth。完整路線圖與合規前提見規格檔。
- [vn-cg-generation](tasks/vn-cg-generation.md) — VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範 — 下一步：前置 `vn-mode` 已立案（2026-08-07），本任務為其 v3 分期；最省驗證＝拿一把 NAI token 打一發看出圖品質與回傳格式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：2026-08-07 討論立案完成（八項拍板＋三分期）；尚未排程，開工前置＝半天生圖實測 a／b／c 定管線，重點研究 NAI
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [shell-update-flash](tasks/shell-update-flash.md) — 卡片介面殼更新無閃白：postMessage ready 取代 load 事件重建雙緩衝 — 下一步：2026-08-12 立案；現行單 iframe 直繪是正確基線，開工前先實測閃白痛感（每回合一次、毫秒級）再定優先序
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：CI 產線＋Windows 安裝檔（tauri-action） — 下一步：出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，憑證再併入 release-2 的 CI secrets
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [claude-compat-endpoint](tasks/claude-compat-endpoint.md) — Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi） — 下一步：實作完成且自驗綠，但本機無 DeepSeek／GLM／Kimi 訂閱可測，暫掛；等有相容端點的訂閱或協力者時再實測結案
- [character-to-player-card](tasks/character-to-player-card.md) — 角色卡升級成玩家卡（角色編輯頁的獨立入口） — 下一步：2026-08-10 立案；重構面板只在 AI 認人時問一次，之後改主意需要這條路，兩項待拍板（已有玩家卡時換不換、能不能反向取消）
- [character-presence](tasks/character-presence.md) — 角色在場/退場狀態管理：自動上下場＋在場過濾 — 下一步：2026-08-11 立案；地基見 CARD-REFACTOR-SPEC 包 4，開工前逐點重拍板，排序在 refactor-dispatch 之後
