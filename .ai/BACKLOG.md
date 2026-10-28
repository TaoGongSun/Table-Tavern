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
- [mvu-meta-schema](tasks/mvu-meta-schema.md) — 非 zod MVU 卡的 schema 檢查（可擴充、必填鍵、模板） — 下一步：等出現帶 $meta 的非 zod 實卡再開工〔作者裁決 2026-10-27〕

## 階段 C：卡片介面接管

作者裁決 2026-10-27（Sol／Grok 共識後提問）：
- 順序：①介面桌處理 NPC 開口蓋掉旁白 → ②修 cardformat-presence-autohide（已結案）→ ③shell-update-flash 先校準量測、不依門檻直接做雙緩衝 → ④character-presence 名冊接線降後獨立排（在場規則搬出 chat.rs 跟著④做）。〔作者裁決 2026-10-27〕
- ①的做法：介面桌停用角色點名（不做回合聚合正文；代價是介面桌失去多角色開口）。〔作者裁決 2026-10-27〕

- [character-presence](tasks/character-presence.md) — 卡片自訂名冊欄位（如駐留角色）接到既有在場機制（順序④） — 下一步：延後到 VN 前再評估〔作者裁決 2026-10-27〕（①②做完後受益桌型少：純介面桌不拆人物）；屆時做法 Sol／Grok 已共識：重構時標名冊路徑＋沒重構桌嚴格欄位名比對退路（只認唯一、形狀合法候選，否則當未知）＋依桌型決定信 present 或名冊；在場規則搬出 chat.rs 跟著本案做
- [shell-swap-flash-multicard](tasks/shell-swap-flash-multicard.md) — 多卡桌跟著最新 GM 樓自動換卡時，介面殼硬切閃白（見 [shell-update-flash](plans/shell-update-flash.md)「已知邊界」） — 下一步：未排程

## 階段 D：引擎行為實聊
已在 HANDOFF／實測佇列，不重列任務檔：[card-arrival-private-leak](handoffs/card-arrival-private-leak.md)、[worldbook-card-import](handoffs/worldbook-card-import.md)、[ui-redesign](handoffs/ui-redesign.md)（9、12）、順手驗梯 2 第 11／18／19、[hide-first-action](handoffs/archive/hide-first-action.md)。

- [agy-long-survey-fail](tasks/agy-long-survey-fail.md) — agy 跑長篇重構盤點多半 cli_stalled 或工具權限被拒，短初判正常 — 下一步：未排程；先在 main 重現分因
- [interface-table-state-report](tasks/interface-table-state-report.md) — 未重構的真介面桌走中性版指示，不保證回報 time／place／present — 下一步：未排程；先量測缺欄頻率，動中性版需作者裁決
- [refactor-preview-ellipsis](tasks/refactor-preview-ellipsis.md) — 重構進行中預覽文字夾雜「⋯」，純外觀、低優先 — 下一步：未排程
- [branch-glyph-match](tasks/branch-glyph-match.md) — 分支綁定 auto_match_branch 只認精確同名，卡名與分支鍵字形不同要手動指認 — 下一步：未排程；先確認實際會不會遇到
- [story-html-style-attr](tasks/story-html-style-attr.md) — 訊息 HTML 放行 style 屬性（字色底色），需 CSS 消毒與網頁 CSP 取捨 — 下一步：等 story-html-whitelist 結案後評估
- [story-html-style-class](tasks/story-html-style-class.md) — 支援 <style> 與 class，讓狀態欄卡外觀接近 ST（需 CSS 解析新依賴） — 下一步：等 story-html-whitelist 結案後評估
- [desktop-csp](tasks/desktop-csp.md) — 桌面版 CSP 是 null，訊息渲染只靠 DOMPurify 一層 — 下一步：未排程；先盤點 app 需要的來源
- [card-prompt-only-regex](tasks/card-prompt-only-regex.md) — 後端套卡片 promptOnly 正則（含 depth），對齊 ST 送模型前的歷史處理 — 下一步：未排程；先盤點測試卡用法並估工
- [gm-advance-hide-no-nomination](tasks/gm-advance-hide-no-nomination.md) — 不要求點名的卡片介面桌，「GM 推進」要不要藏 — 下一步：等 multichar-next-speaker-silent 結案
- [grok-first-session-preamble-drop](tasks/grok-first-session-preamble-drop.md) — 新 grok-home 第一通回合後抹寫失敗、整條 lane 丟掉重開；冷啟動快取掉到 128 — 下一步：未排程；先在乾淨 grok-home 重現並保留 session 檔
- [grok-image-original-leftover](tasks/grok-image-original-leftover.md) — grok 生圖原始 JPEG 累積在 app 的 grok-home/sessions，app 不清 — 下一步：未排程
- [character-send-failure-draft](tasks/character-send-failure-draft.md) — 對角色送出時對白已上桌後才出錯，仍彈帶原文的失敗視窗 — 下一步：等 multichar-next-speaker-silent 結案
- [narrator-prefix-echo](tasks/narrator-prefix-echo.md) — GM 照抄歷史裡 app 的「（旁白）」標籤進正文 — 下一步：未排程；先量發生率
- [refactor-worldbook-statusbar-wording](tasks/refactor-worldbook-statusbar-wording.md) — 重構產出的世界書仍要求「每回合在狀態欄回報」，誘發 GM 自寫狀態段 — 下一步：未排程

## 階段 E：成本
[api-shared-lane](handoffs/api-shared-lane.md) 四路成對測試先做；以下各案依其數據決定做不做：
- [codex-floor-sandbox-group](tasks/codex-floor-sandbox-group.md) — codex 自帶底線 12,288 vs 11,008，疑因沙盒模式不同而帳本分組沒算進去 — 下一步：未排程；先驗差額來源
- [codex-lane-resume](tasks/codex-lane-resume.md) — codex 改走 lane 續聊讓我方內容吃到快取（單發零命中、resume 3 輪中 2） — 下一步：未排程；開工前作者裁決 session 存放位置
- [codex-skills-in-prompt](tasks/codex-skills-in-prompt.md) — 使用者自己的 codex 技能清單（4.9–19.5KB）被帶進跑團 prompt — 下一步：未排程；先查來源與關閉方式

## 階段 F：生圖
[sponsor-features](handoffs/sponsor-features.md) 的 AI 生圖（梯 2 第 8）、[image-model-picker](handoffs/image-model-picker.md)（第 21）、[image-model-free-tier-hide](handoffs/image-model-free-tier-hide.md)（第 22）先實測；之後 vn-mode 前置實測 a／b／c（已有[實測報告](plans/vn-mode-image-spike.md)，待作者三項決定）。

## 階段 G：整理與立案
- [ai-workspace-tidy](tasks/ai-workspace-tidy.md) — .ai/tasks/ 累積到 62 檔，逐檔判斷該留該刪該封存 — 下一步：未排程；開工首步＝比對 tasks/ 與 BACKLOG.md 列出三類清單，狀態不明的逐條問使用者。
- [vn-audio](tasks/vn-audio.md) — VN 音訊 — 下一步：未排程，細節開工時定
- [vn-story-flags](tasks/vn-story-flags.md) — 劇情旗標規格 — 下一步：未排程，細節開工時定
- [vn-full-auto-dual-track](tasks/vn-full-auto-dual-track.md) — 全自動雙軌 — 下一步：未排程，細節開工時定
- [vn-prestructure-split](tasks/vn-prestructure-split.md) — VN v1 開工前結構案：commands/chat.rs 1525、transport/response.rs 1114、data/scene/transcript.rs 1257 行；另列 App.tsx 1049、useChatController.ts 1017、features/play（23 檔）、transport/（25 檔） — 下一步：未排程；開工前重量行數，commands 應只做邊界

## 階段 H：VN 模式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：生圖管線已定案（[實測報告](plans/vn-mode-image-spike.md)）；v1 開工前先做 vn-prestructure-split
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
