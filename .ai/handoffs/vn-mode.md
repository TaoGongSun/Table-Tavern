# vn-mode — VN 桌型：AI 生成視覺小說模式

總綱 [plans/vn-mode.md](../plans/vn-mode.md)（§11 分包與順序、§12 十九項拍板）；立案背景與八項拍板〔作者裁決 2026-08-07〕見下方「立案摘要」。生圖管線定案 [vn-mode-image-spike](../plans/vn-mode-image-spike.md)、前置拆檔 [vn-prestructure-split](archive/vn-prestructure-split.md) 皆完成。分支 `vn-mode`，整案做到結案才進 main。

## 案別與預算（立案凍結）
- 大案，分包 0–9 核心估共約 4300 行（各包估見總綱 §11，已乘 1.5）；順序 0 → 1 → 2 → 7 → 3 → 4 → 9 → 5 → 6 → 8。
- 每包各自算案別：包 0／3／5 中案只驗收 1 次；其餘大案方案＋驗收；包 8／9 碰信任邊界輸入，方案必送。審查員 Sol，每段最多 2 輪、每輪必改最多 3 條。
- 核心＝非測試、非 i18n 譯文、非純型別連帶改動；測試不超過核心 2 倍。

## 目前進度
- 包 0（中案，核心估 250）2026-11-01 施工完成，在分支 `vn-mode`（commit 47233845、8137cf22＋驗收修正一筆）：`WorldState.table_kind`（`chat`｜`vn`，缺省 `chat`；`WorldMeta` 帶出、重構重建沿用、網頁存檔與範例桌固定 chat）、大廳第三張入口卡「視覺小說」與桌卡 VN 徽章、匯入路由框「開 VN 桌並匯入」、VN 開桌精靈骨架 `features/vn/VnWizard.tsx`（① 一句話開桌重用 `GenerateTableDialog`＋genesis 建 vn 桌，或「我有角色卡」選卡建 vn 桌並匯入；② 佔位文字；③④ 佔位；結束進桌）、`enterTable` 依桌型進 `features/vn/VnTableView.tsx` 佔位視圖並可切聊天視圖、i18n 補充字典 `src/i18n/features/vn-mode.ts`。核心淨約 360 行（預算 1.4 倍）、測試約 200 行；verify 13 步綠；測試通道實測大廳→精靈①選卡→②③④→VN 視圖→徽章全通。
- Sol 驗收（串 01a194e8，2 輪）：第 1 輪必改 1（生成中仍可按「我有角色卡」）已修；主線通道實測另抓到從大廳開精靈匯入後狀態重讀帶空桌 id 的錯，已修（`refreshStateOf`）。第 2 輪 PASS、無剩餘必改（2026-11-01）。
- 落點判斷〔模型判斷·未裁決〕：VN 桌進桌先落 `features/vn/` 佔位視圖並可切聊天視圖；包 0 無新解析、不碰信任邊界，照流程上限只驗收不做方案審。
- 包 0 已知限制（留給後包）：(a) 匯入建 VN 桌後精靈 ② 蓋在開場白面板上，兩個都 modal，要先走完精靈再答開場白；(b) 聊天視圖切不回 VN 視圖，要重新進桌（包 2 播放器）；(c) 精靈 ③④ 中途關掉，已建好的 VN 桌留在大廳；(d) VN 佔位視圖沒套版面樣式（包 2 取代）；(e) 入口卡借用 `IconBook`；(f) 精靈 ① 選卡不問身分，帶書的角色卡當角色卡匯、書一起進（純世界書檔走世界書）。
- 熱點行數：`src/App.tsx` 1091→1132、`useImportController.ts` 729→771、`GenerateTableDialog.tsx` 357→368、`Lobby.tsx` 117→129；新檔 VnWizard 114、VnTableView 42。

## 下一步
- 包 1（大案，核心估 400）2026-11-01 完成，在分支 `vn-mode`（commit 5c5dfed3＋7446b5dd）：方案 [plans/vn-mode-pkg1.md](../plans/vn-mode-pkg1.md)（Sol 方案審 2 輪、驗收 2 輪皆 PASS，串 01a19506）。落地：`chat_turn/vn_script.rs`（解析器、角色線過濾、終止優先序與 `pkg1_accepted`、串流入口 `parse_text`、`replace_choices`）、`chat_turn/vn_reroll.rs`（換一組：在途互斥＋閘門內重驗）、`transport/vn_format.rs`（GM／角色規範、導演指示、`vn_assigned_line`、收尾句）、`commands/vn.rs`（`read_vn_cells`、`parse_vn_text`、`vn_reroll_choices`）、`GmTurnFormat::VnScript`、`gm_stop`（next None＋`vn_stop`）、`ReplyEcho::VnDialogue`、VN 桌角色線 own prefix 空字串且歷史不加前綴、前端 `advanceFrom`（VN 送出走推進迴圈）。實作核心淨約 1064 行（程式行約 930，估的 2.3 倍，原因見 process-log）、測試約 750；verify 13 步綠。
- 包 1 已知限制：聊天視圖看 VN 桌會看到原始標記；名單外名字沒加括號的「X：台詞」算旁白；`[bg]` 與圍欄 place 以指示要求一致、不強制；改名後舊對白對不到 id；有介面殼的卡匯進 VN 桌仍被 `reject_character_turn_on_takeover` 擋角色回合；換一組錯誤碼（`VN_REROLL_BUSY`／`STALE`／`FAILED`／`UNAVAILABLE`）未進 i18n，包 2 接按鈕時補；`[chapter]`／`[ending]` 寫法未進給模型的規範（包 5）。
- **開包 2**（大案，核心估 600；方案審＋驗收，測試通道實測）：VN 播放器——舞台（站位、底部對齊、說話者亮、呼吸微動）、對話框、打字機、斷格推進、auto／skip／log、選項鈕與「繼續」「換一組」、1 套主題；消費包 1 的 `read_vn_cells`／`parse_vn_text`（串流暫定格、正式化時游標以已顯示文字長度對位，見包 1 方案 §2）與 `vn_reroll_choices`；取代包 0 的 `VnTableView` 佔位視圖並解掉包 0 已知限制 (a)(b)(d)。順序之後：7 → 3 → 4 → 9 → 5 → 6 → 8。
- 整案結案才進 main：分支歷史整理成一包一筆（以 commit 訊息前綴「包 N」分包，含立案、方案、施工、驗收修正、收尾各筆，各壓成一筆；子代理 commit 署名是 Opus 5.5，壓縮時統一）。

## 立案摘要〔作者裁決 2026-08-07〕
VN 做在本專案內：內核即現有引擎（GM 發言權分配＝劇本推進、自動上下場＝立繪進退場、換幕＝背景切換、scene-fork＝分支、收回＝回溯），VN 是同一引擎的另一種桌型。對 ST 的差異化：任何普通角色卡都能用 VN 方式玩，多角色進退場是引擎原生。
1. 桌型二選一（聊天桌／VN 桌，開桌時選），VN 桌專屬提示詞要 AI 直接輸出 VN 劇本格式；寬容解析、壞格式退旁白格；VN 桌可切聊天視圖看，反向不支援。
2. 介面手工（CSS/SVG 主題）、內容生成；VN 感九成來自背景＋立繪。
3. 對話推進：斷格、點擊翻格、打字機；串流進緩衝吸收延遲；標配 auto／skip／log。
4. 演出＝規則為底（站位照進場順序、說話者亮）、指令為蓋（v1 只有特寫與震動，選配）。
5. 玩家輸入＝純選項制；GM 同一發尾端產選項、分岔點才出；「換一組」是唯一糾偏保險絲。
6. 背景每幕即時生成一張。
7. 立繪一張定稿、五表情差分預生存桌資料夾；主路原生透明輸出，後備本地去背（BiRefNet-lite，ort）〔作者裁決 2026-10-27〕。
8. CG 必帶人物、定稿當參考圖＋外觀規格塊＋玩家重骰把關；CG 屬 v3（[vn-cg-generation](../tasks/vn-cg-generation.md)）。

分期〔作者裁決 2026-11-01〕：v1＝包 0–9；v3 即時 CG；之後音訊、全自動雙軌、條件選項、第 2–3 套主題、跨周目。

## 約束
- 生圖一律玩家自備額度（BYOK），app 不代購不代付。
- 劇本格式與指令集從小起步，每加一個標記都是格式負擔與翻車面。
- 立繪、表情、CG 全走重骰＋玩家挑選把關。
