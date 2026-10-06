# 待辦清單

還沒開工的案子。手寫維護，一條一行；立案說明在各自的 [tasks/](tasks/)`<id>.md`。
開工＝把該檔搬進 [handoffs/](handoffs/) 並在 [HANDOFF.md](HANDOFF.md) 登記一條，本檔那行刪掉。
已在進行中的看 [HANDOFF.md](HANDOFF.md)，等實機驗收的看 [實測佇列](reference/verification-queue.md)。

- [model-version-follow](tasks/model-version-follow.md) — 模型新版推出時跟上：CLI 手選型號的人提醒換新版、OpenRouter 檔位預填不再寫死 — 下一步：未排程；先查各 CLI 清單能否分辨預設、OpenRouter 官方 API 能否判斷新版〔作者裁決 2026-10-06〕
- [claude-1h-cache](reference/overnight-2026-10-06.md) — claude CLI 1 小時快取：過期門檻、保溫、省額係數 — 下一步：已開工，分支 `claude-1h-cache`（交接與證據在分支上）；查出 1h 不是 CLI 固定行為，等作者三選一（見夜間總表）
- [api-request-header-timeout](tasks/api-request-header-timeout.md) — API 請求送出後到回應頭之前、非 2xx 讀錯誤本文都沒有逾時 — 下一步：只包 send() 與錯誤本文讀取，不用 ClientBuilder::timeout
- [image-save-strict-validate](reference/overnight-2026-10-06.md) — 一般存圖接上嚴格 PNG 驗證 — 下一步：已開工，分支 `image-save-strict-validate`（盤點在分支 plans 第四節）；等作者決定八項拒收處理（見夜間總表）
- [ai-workspace-tidy](tasks/ai-workspace-tidy.md) — .ai/tasks/ 累積到 62 檔，逐檔判斷該留該刪該封存 — 下一步：未排程；開工首步＝比對 tasks/ 與 BACKLOG.md 列出三類清單，狀態不明的逐條問使用者。
- [no-cache-model-optout](tasks/no-cache-model-optout.md) — 零命中的模型不走共線：自動退回單角色組裝 — 下一步：開工前先重新立證：等帶 `cache_reporting: "reported"` 的 eligible zero 累積出來，確認真的有模型零命中。證據站得住再拍板規格檔的四項（solo 的 role 分配、要不要讓玩家看見、冷卻週期、與 usage-diag-non-claude 的先後）。
- [non-claude-real-cache](tasks/non-claude-real-cache.md) — codex／agy／OpenRouter 沒有續聊，快取到底有沒有真的抓到 — 下一步：[usage-cache-audit](handoffs/archive/usage-cache-audit.md) 四家實跑對帳已完成（結果見 plans/usage-cache-audit.md「四」），據此決定做不做〔作者裁決 2026-10-02〕；ox-alpha 若已下架，根據的現象要重新立證。
- [vendor-prefix-floor](tasks/vendor-prefix-floor.md) — 只中到供應商白送的那段，不該報成命中 — 下一步：排在 api-shared-lane 的四路成對測試之後開工——那批數據才估得準底線該怎麼定、以及這個功能還需不需要。開工首步是拍板底線的統計量（最小值／眾數／出現 ≥N 次的最小值）與「樣本不足就不判定」的 N。
- [ai-connection-provider-panels](tasks/ai-connection-provider-panels.md) — AI 連線設定重整：供應商專屬面板（延後） — 下一步：等 free-player-onboarding 兩階段完成後再重新評估；目前不動 CLI、高中低與 provider-specific UI。
- [vn-cg-generation](tasks/vn-cg-generation.md) — VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範 — 下一步：前置 `vn-mode` 已立案（2026-08-07），本任務為其 v3 分期；最省驗證＝拿一把 NAI token 打一發看出圖品質與回傳格式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：2026-08-07 討論立案完成（八項拍板＋三分期）；尚未排程，開工前置＝半天生圖實測 a／b／c 定管線，重點研究 NAI
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [shell-update-flash](tasks/shell-update-flash.md) — 卡片介面殼更新無閃白：postMessage ready 取代 load 事件重建雙緩衝 — 下一步：2026-08-12 立案；現行單 iframe 直繪是正確基線，開工前先實測閃白痛感（每回合一次、毫秒級）再定優先序
- [release-2-ci-windows](tasks/release-2-ci-windows.md) — 發佈 2：Windows 首個正式版（CI 產線與 NSIS 安裝檔已由 desktop-update-detect 包 1 的 release.yml 做完） — 下一步：發未簽章正式版（Mac 同樣不簽）＋發布說明附 SmartScreen／Gatekeeper 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：首發不簽章〔作者裁決 2026-10-04〕，等使用者變多再考慮；Gatekeeper 繞過說明併入 release-2 發布說明
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [character-to-player-card](tasks/character-to-player-card.md) — 角色卡升級成玩家卡（角色編輯頁的獨立入口） — 下一步：2026-08-10 立案；重構面板只在 AI 認人時問一次，之後改主意需要這條路，兩項待拍板（已有玩家卡時換不換、能不能反向取消）
- [character-presence](tasks/character-presence.md) — 卡片自訂名冊欄位（如駐留角色）接到既有在場機制 — 下一步：拍板怎麼認名冊欄位、名冊與 present 不一致時誰優先；包 4 已做的在場過濾與自動上下場不重做。
