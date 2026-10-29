# 待辦清單

2026-10-29 作者授權主線依「罕見邊際狀況輕防」重審各案範圍〔作者裁決 2026-10-29〕；各案縮小／只記限制的結論標〔模型判斷·未裁決〕。

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

- [mvu-command-syntax](tasks/mvu-command-syntax.md) — 未重構 MVU 卡指令式更新（_.set）被靜默略過 — 下一步：只記已知限制，等出現實卡再開工〔模型判斷·未裁決〕（2026-10-27 掃 TestCards 全部 55 項無一張教指令式語法）；開工時優先讓非 zod 路徑重用共用核心 source-commands.ts
- [mvu-meta-schema](tasks/mvu-meta-schema.md) — 非 zod MVU 卡的 schema 檢查（可擴充、必填鍵、模板） — 下一步：等出現帶 $meta 的非 zod 實卡再開工〔作者裁決 2026-10-27〕

## 階段 C：卡片介面接管

作者裁決 2026-10-27（Sol／Grok 共識後提問）：
- 順序：①介面桌處理 NPC 開口蓋掉旁白 → ②修 cardformat-presence-autohide（已結案）→ ③shell-update-flash 先校準量測、不依門檻直接做雙緩衝 → ④character-presence 名冊接線降後獨立排（在場規則搬出 chat.rs 跟著④做）。〔作者裁決 2026-10-27〕
- ①的做法：介面桌停用角色點名（不做回合聚合正文；代價是介面桌失去多角色開口）。〔作者裁決 2026-10-27〕

- [character-presence](tasks/character-presence.md) — 卡片自訂名冊欄位（如駐留角色）接到既有在場機制（順序④） — 下一步：延後到 VN 前再評估〔作者裁決 2026-10-27〕（①②做完後受益桌型少：純介面桌不拆人物）；屆時做法 Sol／Grok 已共識：重構時標名冊路徑＋沒重構桌嚴格欄位名比對退路（只認唯一、形狀合法候選，否則當未知）＋依桌型決定信 present 或名冊；在場規則搬出 chat.rs 跟著本案做
- [shell-swap-flash-multicard](tasks/shell-swap-flash-multicard.md) — 多卡桌跟著最新 GM 樓自動換卡時，介面殼硬切閃白（見 [shell-update-flash](plans/shell-update-flash.md)「已知邊界」） — 下一步：只記已知限制、暫不動工〔模型判斷·未裁決〕（已寫在 shell-update-flash「已知邊界」）

## 階段 D：引擎行為實聊
已在 HANDOFF／實測佇列，不重列任務檔：[card-arrival-private-leak](handoffs/card-arrival-private-leak.md)、[worldbook-card-import](handoffs/worldbook-card-import.md)、[ui-redesign](handoffs/ui-redesign.md)（9、12）、順手驗梯 2 第 11／18／19、[hide-first-action](handoffs/archive/hide-first-action.md)。

- [lane-session-superseded-leftover](tasks/lane-session-superseded-leftover.md) — 被取代的 grok lane session 目錄沒刪（測試 root 見兩例） — 下一步：只記已知限制〔模型判斷·未裁決〕；前置 agy-home-cleanup 已結案，剩確認 grok TTL 涵蓋 lane session 目錄
- [interface-table-state-report](tasks/interface-table-state-report.md) — 未重構的真介面桌走中性版指示，不保證回報 time／place／present — 下一步：縮小成只量測幾張真介面桌的缺欄頻率〔模型判斷·未裁決〕，不改中性版；動中性版需作者裁決
- [refactor-preview-ellipsis](tasks/refactor-preview-ellipsis.md) — 重構進行中預覽文字夾雜「⋯」，純外觀、低優先 — 下一步：未排程
- [branch-glyph-match](tasks/branch-glyph-match.md) — 分支綁定 auto_match_branch 只認精確同名，卡名與分支鍵字形不同要手動指認 — 下一步：只記已知限制、暫不動工〔模型判斷·未裁決〕；真遇到實卡再做
- [story-html-style-attr](tasks/story-html-style-attr.md) — 訊息 HTML 放行 style 屬性（字色底色），需 CSS 消毒與網頁 CSP 取捨 — 下一步：縮小成少數屬性白名單（字色、底色、字重等，帶函式整條丟棄）〔模型判斷·未裁決〕；前置 story-html-whitelist 已結案，可排程
- [story-html-style-class](tasks/story-html-style-class.md) — 支援 <style> 與 class，讓狀態欄卡外觀接近 ST（需 CSS 解析新依賴） — 下一步：排在 story-html-style-attr 最小版之後再評估〔模型判斷·未裁決〕；前置 story-html-whitelist 已結案
- [desktop-csp](tasks/desktop-csp.md) — 桌面版 CSP 是 null，訊息渲染只靠 DOMPurify 一層 — 下一步：未排程；先盤點 app 需要的來源
- [card-prompt-only-regex](tasks/card-prompt-only-regex.md) — 後端套卡片 promptOnly 正則（含 depth），對齊 ST 送模型前的歷史處理 — 下一步：縮小成只套 TestCards 實際出現的 promptOnly 形狀〔模型判斷·未裁決〕；先盤點測試卡用法
- [refactor-outcome-fields](tasks/refactor-outcome-fields.md) — 重構產物帶盤點 FIELDS 與不收格子名單（開場沒附狀態欄的卡也能追蹤；地圖、記憶不收） — 下一步：前置 statusbar-only-fields 已結案，可排程
- [unix-only-fake-cli-tests](tasks/unix-only-fake-cli-tests.md) — 123 個測試用 shell 假 CLI，Windows 跑不到 — 下一步：縮小成共用 Rust 假 CLI＋改 lane／runner 約 10–20 個代表測試〔模型判斷·未裁決〕；其餘維持 unix-only

## 階段 E：成本
[api-shared-lane](handoffs/api-shared-lane.md) 四路成對測試先做；以下各案依其數據決定做不做：
- [codex-floor-sandbox-group](tasks/codex-floor-sandbox-group.md) — codex 自帶底線 12,288 vs 11,008，疑因沙盒模式不同而帳本分組沒算進去 — 下一步：只記已知限制〔模型判斷·未裁決〕；階段 E 要用這個數字做決策時再驗

## 階段 F：生圖
[sponsor-features](handoffs/sponsor-features.md) 的 AI 生圖（梯 2 第 8）、[image-model-picker](handoffs/image-model-picker.md)（第 21）、[image-model-free-tier-hide](handoffs/image-model-free-tier-hide.md)（第 22）先實測；之後 vn-mode 前置實測 a／b／c（已有[實測報告](plans/vn-mode-image-spike.md)，待作者三項決定）。

## 階段 G：整理與立案
- [ai-workspace-tidy](tasks/ai-workspace-tidy.md) — .ai/tasks/ 累積到 62 檔，逐檔判斷該留該刪該封存 — 下一步：未排程；開工首步＝比對 tasks/ 與 BACKLOG.md 列出三類清單，狀態不明的逐條問使用者。
- [vn-audio](tasks/vn-audio.md) — VN 音訊 — 下一步：暫不動工、保留立案〔模型判斷·未裁決〕；屬 vn-mode 後續
- [vn-story-flags](tasks/vn-story-flags.md) — 劇情旗標規格 — 下一步：暫不動工、保留立案〔模型判斷·未裁決〕；屬 vn-mode 後續
- [vn-full-auto-dual-track](tasks/vn-full-auto-dual-track.md) — 全自動雙軌 — 下一步：暫不動工、保留立案〔模型判斷·未裁決〕；屬 vn-mode 後續
- [vn-prestructure-split](tasks/vn-prestructure-split.md) — VN v1 開工前結構案：commands/chat.rs 1525、transport/response.rs 1114、data/scene/transcript.rs 1257 行；另列 App.tsx 1049、useChatController.ts 1017、features/play（23 檔）、transport/（25 檔） — 下一步：未排程；開工前重量行數，commands 應只做邊界

## 階段 H：VN 模式
- [vn-mode](tasks/vn-mode.md) — VN 桌型：AI 生成視覺小說模式（劇本格式＋演出＋選項制） — 下一步：生圖管線已定案（[實測報告](plans/vn-mode-image-spike.md)）；v1 開工前先做 vn-prestructure-split
- [vn-cg-generation](tasks/vn-cg-generation.md) — VN 模式 CG 即時生成：外接吃到飽生圖訂閱（NAI 類）＋提示詞規範 — 下一步：前置 `vn-mode` 已立案（2026-08-07），本任務為其 v3 分期；最省驗證＝拿一把 NAI token 打一發看出圖品質與回傳格式

## 排後
原訂 16 件（發佈平台 5、API 限定 5、供應商設定 3、其他功能 2、碰運氣 1）；以下為目前 BACKLOG 裡不屬於上面各階段的案子，逐件歸類待作者確認〔模型判斷·未裁決〕。
- [initvar-entry-yaml-semantics](tasks/initvar-entry-yaml-semantics.md) — [initvar] 條目挑選與 YAML 語意（$meta、# 註解）對齊上游 — 下一步：縮小〔模型判斷·未裁決〕：第 3 項照作者裁決，第 1、2 項只做合併條目＋剝圍欄＋# 當註解，$meta 分支留限制；排在 initvar-yaml-list 之後
- [list-field-line-update](tasks/list-field-line-update.md) — 清單欄逐行更新（依鍵 upsert／remove） — 下一步：未排程〔作者裁決 2026-10-11〕；縮小成重構時拆欄位、不做 upsert 協定〔模型判斷·未裁決〕
- [model-version-follow](tasks/model-version-follow.md) — 模型新版推出時跟上：CLI 手選型號的人提醒換新版、OpenRouter 檔位預填不再寫死 — 下一步：縮小〔模型判斷·未裁決〕：OpenRouter 預填改執行時決定、CLI 提醒先只做 grok〔作者裁決 2026-10-06：立案、兩件併一案〕
- [ai-connection-provider-panels](tasks/ai-connection-provider-panels.md) — AI 連線設定重整：供應商專屬面板（延後） — 下一步：暫不動工〔模型判斷·未裁決〕；前置 free-player-onboarding 已結案，等真實玩家回饋設定頁困擾再評估
- [ttrpg-rules-system](tasks/ttrpg-rules-system.md) — 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容） — 下一步：五題拍板完成（2026-08-02）；前置 st-ecosystem 已結案，排程時機由作者定；v1（指南＋骰池＋骰鈕＋注入實測）不依賴狀態欄，v2 等狀態欄二期後細拍
- [release-1-mac-signing](tasks/release-1-mac-signing.md) — 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool） — 下一步：首發不簽章〔作者裁決 2026-10-04〕，等使用者變多再考慮；Gatekeeper 繞過說明併入 release-2 發布說明
- [cli-custom-provider](tasks/cli-custom-provider.md) — 自訂 CLI 供應商：使用者自填指令模板接任意 CLI（如 Kimi） — 下一步：暫不動工〔模型判斷·未裁決〕；確認真實需求後拍板設定 schema，v1 只做純文字模式
- [character-to-player-card](tasks/character-to-player-card.md) — 角色卡升級成玩家卡（角色編輯頁的獨立入口） — 下一步：維持排後；兩項待拍板，建議最小版（直接互換、不做取消成零張）〔模型判斷·未裁決〕已寫進任務檔，開工時請作者確認
