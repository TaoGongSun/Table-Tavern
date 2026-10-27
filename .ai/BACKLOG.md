# 待辦清單

還沒開工的案子，按執行順序排（2026-10-27 作者排定；目標＝VN 模式，基礎先穩）。手寫維護，一條一行；立案說明在各自的 [tasks/](tasks/)`<id>.md`。
開工＝把該檔搬進 [handoffs/](handoffs/) 並在 [HANDOFF.md](HANDOFF.md) 登記一條，本檔那行刪掉。
已在進行中的看 [HANDOFF.md](HANDOFF.md)，等實機驗收的看 [實測佇列](reference/verification-queue.md)。
競品研究：[Marinara Engine 參考策略](plans/marinara_reference_notes.md)（借鏡清單 §14、強化順序 §15、研究待辦 §20 尚未立案）。

**每案交辦固定帶**：動到的檔案先看行數與資料夾檔數；照 [docs/STRUCTURE.md](../docs/STRUCTURE.md)「修改既有功能要不要順便搬」——容得下的 structure-only 小搬遷同案做，大搬家另立案先做；驗收回報附動到檔案的前後行數。

## 階段 A：先拍板
- 梯 2 第 5、6 項（refactor-mode-split、ai-card-refactor＋person-promote＋state-values-mvu）已驗已結案（2026-10-27），不再列。
- 剩兩件要作者拍：mvu-zod-semantics 決策（已拍板，見階段 B 註記）、revert／fork 補 `auto_hidden` 的做法（已結案 2026-10-27，見 [交接檔](handoffs/archive/revert-fork-auto-hidden.md)）。

## 階段 B：MVU 相容
mvu-zod-semantics 已結案（2026-10-27 合併 main，[交接檔](handoffs/archive/mvu-zod-semantics.md)），順序：zod（已結案）→ command-syntax → meta-schema → shim-flaky-test（已結案）→ card-mvu-shim 4d（作者手點，見[實測佇列](reference/verification-queue.md)）。
開工前結構體檢：import/（目前 29 檔）、import/card.rs 1188、message_vars。

- [mvu-command-syntax](tasks/mvu-command-syntax.md) — 未重構 MVU 卡指令式更新（_.set）被靜默略過 — 下一步：等出現實卡再開工〔模型判斷·未裁決〕（2026-10-27 掃 TestCards 全部 55 項無一張教指令式語法）；開工時優先讓非 zod 路徑重用共用核心 source-commands.ts
- [mvu-meta-schema](tasks/mvu-meta-schema.md) — 非 zod MVU 卡的 schema 檢查（可擴充、必填鍵、模板） — 下一步：未排程
- [web-mvu-memory-cap](tasks/web-mvu-memory-cap.md) — 網頁版 MVU 引擎補記憶體硬上限（現只有時間上限；可選 quickjs-emscripten） — 下一步：作者裁決先當例外、記待辦〔作者裁決 2026-10-27〕；排在 mvu-zod-semantics 合併之後
- [mvu-abandoned-threads-flaky](tasks/mvu-abandoned-threads-flaky.md) — mvu_engine 作廢執行緒名額測試在負載下偶發失敗，讓 verify 假紅 — 下一步：未排程；先在高負載下重現
- [mvu-memory-bomb-flaky](tasks/mvu-memory-bomb-flaky.md) — mvu_engine 記憶體炸彈測試負載下 Deadline 搶在 Memory 前，verify 假紅 — 下一步：未排程；用 abandoned-threads 案的負載腳本重現

## 階段 C：卡片介面接管

作者裁決 2026-10-27（Sol／Grok 共識後提問）：
- 順序：①介面桌處理 NPC 開口蓋掉旁白 → ②修 cardformat-presence-autohide（已結案）→ ③shell-update-flash 先校準量測、不依門檻直接做雙緩衝 → ④character-presence 名冊接線降後獨立排（在場規則搬出 chat.rs 跟著④做）。〔作者裁決 2026-10-27〕
- ①的做法：介面桌停用角色點名（不做回合聚合正文；代價是介面桌失去多角色開口）。〔作者裁決 2026-10-27〕

- [character-presence](tasks/character-presence.md) — 卡片自訂名冊欄位（如駐留角色）接到既有在場機制（順序④） — 下一步：延後到 VN 前再評估〔作者裁決 2026-10-27〕（①②做完後受益桌型少：純介面桌不拆人物）；屆時做法 Sol／Grok 已共識：重構時標名冊路徑＋沒重構桌嚴格欄位名比對退路（只認唯一、形狀合法候選，否則當未知）＋依桌型決定信 present 或名冊；在場規則搬出 chat.rs 跟著本案做
- [shell-swap-flash-multicard](tasks/shell-swap-flash-multicard.md) — 多卡桌跟著最新 GM 樓自動換卡時，介面殼硬切閃白（見 [shell-update-flash](plans/shell-update-flash.md)「已知邊界」） — 下一步：未排程

## 階段 D：引擎行為實聊
已在 HANDOFF／實測佇列，不重列任務檔：[card-arrival-private-leak](handoffs/card-arrival-private-leak.md)、[worldbook-card-import](handoffs/worldbook-card-import.md)、[ui-redesign](handoffs/ui-redesign.md)（9、12）、順手驗梯 2 第 11／18／19、[hide-first-action](handoffs/archive/hide-first-action.md)。

- [takeover-yaml-key-glyph](tasks/takeover-yaml-key-glyph.md) — 接管桌續聊 GM 把清單欄 YAML 鍵寫成繁體，原卡腳本只認簡體鍵而少欄（ai-card-refactor B5 重現） — 下一步：未排程；先重現
- [takeover-flat-nonfixed-keys](tasks/takeover-flat-nonfixed-keys.md) — 介面接管路線開增量時平欄非固定劇情鍵凍住不更新 — 下一步：先重現
- [card-field-overlaps-fixed-keys](tasks/card-field-overlaps-fixed-keys.md) — 卡欄位（当前环境、驻留/交涉角色）與 place／present 同義，GM 只報固定鍵、卡欄位凍住 — 下一步：先提做法選項，與 character-presence 一併評估
- [interface-table-state-report](tasks/interface-table-state-report.md) — 未重構的真介面桌走中性版指示，不保證回報 time／place／present — 下一步：未排程；先量測缺欄頻率，動中性版需作者裁決
- [card-format-html-comment-shown](tasks/card-format-html-comment-shown.md) — 模型照抄卡片格式裡的 HTML 註解（如 <!-- 隐藏记忆栏 -->）後畫面顯示成純文字，ST 會藏 — 下一步：未排程；先重現、查 ST 處理方式
- [refactor-preview-ellipsis](tasks/refactor-preview-ellipsis.md) — 重構進行中預覽文字夾雜「⋯」，純外觀、低優先 — 下一步：未排程
- [branch-glyph-match](tasks/branch-glyph-match.md) — 分支綁定 auto_match_branch 只認精確同名，卡名與分支鍵字形不同要手動指認 — 下一步：未排程；先確認實際會不會遇到

## 階段 E：成本
[api-shared-lane](handoffs/api-shared-lane.md) 四路成對測試先做；以下三案依其數據決定做不做：
- [vendor-prefix-floor](tasks/vendor-prefix-floor.md) — 只中到供應商白送的那段，不該報成命中 — 下一步：排在 api-shared-lane 的四路成對測試之後開工——那批數據才估得準底線該怎麼定、以及這個功能還需不需要。開工首步是拍板底線的統計量（最小值／眾數／出現 ≥N 次的最小值）與「樣本不足就不判定」的 N。
- [non-claude-real-cache](tasks/non-claude-real-cache.md) — codex／agy／OpenRouter 沒有續聊，快取到底有沒有真的抓到 — 下一步：[usage-cache-audit](handoffs/archive/usage-cache-audit.md) 四家實跑對帳已完成（結果見 plans/usage-cache-audit.md「四」），據此決定做不做〔作者裁決 2026-10-02〕；ox-alpha 若已下架，根據的現象要重新立證。
- [no-cache-model-optout](tasks/no-cache-model-optout.md) — 零命中的模型不走共線：自動退回單角色組裝 — 下一步：開工前先重新立證：等帶 `cache_reporting: "reported"` 的 eligible zero 累積出來，確認真的有模型零命中。證據站得住再拍板規格檔的四項（solo 的 role 分配、要不要讓玩家看見、冷卻週期、與 usage-diag-non-claude 的先後）。

## 階段 F：生圖
[sponsor-features](handoffs/sponsor-features.md) 的 AI 生圖（梯 2 第 8）、[image-model-picker](handoffs/image-model-picker.md)（第 21）、[image-model-free-tier-hide](handoffs/image-model-free-tier-hide.md)（第 22）先實測；之後 vn-mode 前置實測 a／b／c（已有[實測報告](plans/vn-mode-image-spike.md)，待作者三項決定）。

## 階段 G：整理與立案
- [ai-workspace-tidy](tasks/ai-workspace-tidy.md) — .ai/tasks/ 累積到 62 檔，逐檔判斷該留該刪該封存 — 下一步：未排程；開工首步＝比對 tasks/ 與 BACKLOG.md 列出三類清單，狀態不明的逐條問使用者。
- [vn-audio](tasks/vn-audio.md) — VN 音訊 — 下一步：未排程，細節開工時定
- [vn-story-flags](tasks/vn-story-flags.md) — 劇情旗標規格 — 下一步：未排程，細節開工時定
- [vn-full-auto-dual-track](tasks/vn-full-auto-dual-track.md) — 全自動雙軌 — 下一步：未排程，細節開工時定
- [vn-prestructure-split](tasks/vn-prestructure-split.md) — VN v1 開工前結構案：commands/chat.rs 1525、transport/response.rs 1114、data/scene/transcript.rs 1257 行；另列 App.tsx 1049、useChatController.ts 1017、features/play（23 檔）、transport/（25 檔） — 下一步：未排程；開工前重量行數，commands 應只做邊界

## 階段 H：VN 模式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：前置＝階段 F 生圖實測與[實測報告](plans/vn-mode-image-spike.md)的三項作者決定；v1 開工前先做 vn-prestructure-split
- [vn-cg-generation](tasks/vn-cg-generation.md) — VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範 — 下一步：前置 `vn-mode` 已立案（2026-08-07），本任務為其 v3 分期；最省驗證＝拿一把 NAI token 打一發看出圖品質與回傳格式

## 排後
原訂 16 件（發佈平台 5、API 限定 5、供應商設定 3、其他功能 2、碰運氣 1）；以下為目前 BACKLOG 裡不屬於上面各階段的案子，逐件歸類待作者確認〔模型判斷·未裁決〕。
- [initvar-entry-yaml-semantics](tasks/initvar-entry-yaml-semantics.md) — [initvar] 條目挑選與 YAML 語意（$meta、# 註解）對齊上游 — 下一步：未排程，排在 initvar-yaml-list 之後
- [list-field-line-update](tasks/list-field-line-update.md) — 清單欄逐行更新（依鍵 upsert／remove） — 下一步：未排程〔作者裁決 2026-10-11〕
- [model-version-follow](tasks/model-version-follow.md) — 模型新版推出時跟上：CLI 手選型號的人提醒換新版、OpenRouter 檔位預填不再寫死 — 下一步：未排程；先查各 CLI 清單能否分辨預設、OpenRouter 官方 API 能否判斷新版〔作者裁決 2026-10-06〕
- [ai-connection-provider-panels](tasks/ai-connection-provider-panels.md) — AI 連線設定重整：供應商專屬面板（延後） — 下一步：等 free-player-onboarding 兩階段完成後再重新評估；目前不動 CLI、高中低與 provider-specific UI。
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02），排程晚於 st-ecosystem；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：首發不簽章〔作者裁決 2026-10-04〕，等使用者變多再考慮；Gatekeeper 繞過說明併入 release-2 發布說明
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：確認真實需求後拍板設定 schema，v1 只做純文字模式
- [character-to-player-card](tasks/character-to-player-card.md) — 角色卡升級成玩家卡（角色編輯頁的獨立入口） — 下一步：2026-08-10 立案；重構面板只在 AI 認人時問一次，之後改主意需要這條路，兩項待拍板（已有玩家卡時換不換、能不能反向取消）
