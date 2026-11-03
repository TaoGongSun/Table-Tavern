# vn-mode — VN 桌型：AI 生成視覺小說模式

總綱 [plans/vn-mode.md](../plans/vn-mode.md)（§11 分包與順序、§12 十九項拍板）；立案背景與八項拍板〔作者裁決 2026-08-07〕見下方「立案摘要」。生圖管線定案 [vn-mode-image-spike](../plans/vn-mode-image-spike.md)、前置拆檔 [vn-prestructure-split](archive/vn-prestructure-split.md) 皆完成。分支 `vn-mode`，整案做到結案才進 main。

## 案別與預算（立案凍結）
- 大案，分包 0–9 核心估共約 4300 行（各包估見總綱 §11，已乘 1.5）；順序 0 → 1 → 2 → 7 → 3 → 4 → 9a → 9b → 5 → 6 → 8。
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
- 包 2（大案，核心估 600）2026-11-01 完成，在分支 `vn-mode`（commit 5ff4f199、b09f8f37、7327c23c）：方案 [plans/vn-mode-pkg2.md](../plans/vn-mode-pkg2.md)（Sol 方案審 2 輪必改 3 全採納、驗收 1 輪 PASS 無必改；方案串 01a19548、驗收串 01a1957a）。落地 `features/vn/`：`VnPlayer`（工具列、共用動作鎖、換一組錯誤碼、效能標記）、`Stage`、`DialogueBox`、`ChoicePanel`、`Backlog`、`useVnCells`（整幕重抓＋串流暫定格＋凍結快照換手）、`useVnPlayback`（游標、打字機、auto、skip、結尾狀態）、`vn-cells.ts`（攤平、可播判定、`realignCursor`、`stageAt`）、`vn-player.css`；`VnTableView` 刪除；包 0 限制 (a)(b)(d) 已解（精靈開著不出開場白面板、聊天頂列「VN 視圖」鈕、主題）。實測修正連帶改包 1 的 `vn_script.rs`（卡名括號註記對名、剝「旁白：」）與角色規範一句。實作核心約 1050（含 CSS，1.75 倍）、測試約 430；verify 13 步綠；測試通道真模型實測三個時間見方案 §7。
- 包 2 已知限制：進桌從最後一則非玩家事件開頭播、不記讀到哪；skip 不分已讀；打字速度與 auto 時間固定；立繪是角色卡原圖（可能帶背景）、表情不換圖；背景只有地點字卡；單角色桌 GM 常自己寫名單角色台詞（Haiku 遵守度低，包 1 指示已要求不替名單角色說話）。
- 包 7（大案，立繪管線）2026-11-01 完成，在分支 `vn-mode`：前置補測見 [實測報告](../plans/vn-mode-image-spike.md)「包 7 前補測」（裁頭重繪勝出、CLI 抓圖改讀 thread 目錄）；方案 [plans/vn-mode-pkg7.md](../plans/vn-mode-pkg7.md)（Sol 方案審 2 輪 PASS；作者裁決：權重首次下載、來源 Codex／agy／API、拆 7a／7b、`ort` 靜態連結）。
  - 7a（commit aeee6713、22747c73、0c330bb7；驗收 2 輪 PASS）：取圖層改讀 codex thread／agy brain 目錄（既有角色生圖一併修）、`vn_sprites/`（sprite.json、裁頭重繪臉層＋對齊門檻＋定稿 alpha 裁剪、背景佇列可續生、定稿世代號）、Stage 兩層疊圖與退路鏈、精靈 ③、立繪分頁。
  - 7b（commit 9397bab6、e78256c9、4390ccef、eb9b3062；驗收 2 輪 PASS，串 01a19650）：變體（共用／獨立表情組，換定稿前共用組自動轉獨立組）、`@變體` 上台沿用與 GM／角色線提示詞注入、去背（`ort` 靜態連結＋BiRefNet 首用下載、SHA-256 驗證，執行檔 +27 MB）、立繪來源偏好 `vn_sprite_source`（Codex／agy／API）、變體鍵路徑驗證兩層擋。
  - 核心 7a 約 2000–2300、7b 約 1035（高於估計，收尾審過無不必要程式碼，見 process-log 第 3 次檢討）。通道真生圖共約 29 張：雙角色、closeup、重開續生、改臉框、變體、換定稿、去背，接縫目視皆過。
  - 已知限制：對齊門檻沒有真位移樣本；臉框不合格退預設框（獨立組退預設框時分頁不提示）；agy 生圖與 API 來源未實生（API 排實機驗收佇列，agy 撞 429）；新增變體改角色線固定前段、resume 線斷一次快取；ort 共用環境第二次推論與 Windows DirectML 連結待驗（看第一次 CI）；`AI_IMAGE_FREE_KEY` 在立繪分頁顯示原文；預算量測未算變體那段提示詞。
- 包 3（中案，背景管線）2026-11-01 完成，在分支 `vn-mode`（commit c91460ca、9bc5ac42＋收尾一筆）：方案 [plans/vn-mode-pkg3.md](../plans/vn-mode-pkg3.md)；Sol 驗收 1 輪 PASS 無必改（串 01a196ad），採納建議一條（背景 PNG 改原子寫入）。落地：`vn_backgrounds.rs`（地點雜湊 slug＋固定四時段鍵、背景庫與 `backgrounds.json`、整幕讀取補沿用時段與幕首退路格、改光線參考圖、失敗自動重試一次）、佇列泛化成臉層／背景共用一次一張（背景優先）、`commands/vn_backgrounds.rs`、提示詞 `transport/vn_background_prompts.rs`、VN 開桌開場第一行寫 `[bg]`、舞台背景層（生成中前一張模糊或黑幕＋字卡、好了淡入）、工具列「重生成背景」、精靈 ④ 預生並預覽。核心淨約 650–750（立案估 250）、測試約 215；verify 13 步綠；通道 Codex 真生圖五項全過，主線親看截圖。
- 包 3 已知限制：失敗只記記憶體，重開 app 會再自動試一次；重生成無確認；英文時段子字串比對（today→day）；字卡照模型原文顯示時段；agy 背景回 NO_IMAGE；立繪與背景同框、真 GM 回合寫的時段詞、開場時段改內容語言後都未在通道重看；來源鍵仍叫 `vn_sprite_source`。
- 包 4（大案，聖經、劇本分頁、劇透鎖、旗標、揭露）2026-11-01 完成，在分支 `vn-mode`（commit 0f304792、dbc14863、dd5176e5、e8db6b43＋收尾一筆）：方案 [plans/vn-mode-pkg4.md](../plans/vn-mode-pkg4.md)。
  - Sol 方案審 2 輪：必改 3＋1 全採納，串 01a196d9。驗收 2 輪：必改 3＋0，串 01a19725。
  - 落地：`data/vn.rs`（`vn.json`、驗證正規化、補種）、`vn_bible.rs`（生成、套用、凍結注入，以幕為鍵）、`transport/vn_bible_prompts.rs`、`commands/vn_bible.rs`、`mechanism/write_guard.rs`（`FieldRule.allowed`、受保護根 `flags`／`revealed`／`chapter`，擋模型更新、圍欄、面板手改、once 旗標四個入口）；前端 `features/vn/bible/`（遮罩、劇本分頁、精靈 ②、格邊旗標提示），世界編輯器「世界｜劇本」分頁。
  - 規模：核心約 2110（立案估 450，重估 1000–1200），測試約 870；verify 13 步綠。
  - 通道真模型實測六項，主線親看截圖。
- 包 4 已知限制：
  - Haiku 不回報旗標，gpt-5.6-luna 會；包 5 結局判定前要處理遵守度。
  - 「路徑不存在」回饋句會誘導模型在頂層長出表外鍵。
  - 簡體卡的聖經沿用簡體。
  - 改章數後，理由句仍是舊章數。
  - 通道只驗到部分的項目：計數旗標「+1」、模型自寫非法值、換幕後聖經生效（皆有單元測試）。
  - zod 引擎桌不補種。
  - 劇本只能在分頁內存（頂列儲存只存 world.md）。
  - 結構類（增刪章、旗標、結局）不能改，只能重骰。
- 包 9 拆 9a／9b〔作者裁決 2026-11-01〕。9a（初始 VN 卡）2026-11-01 完成，在分支 `vn-mode`（commit a092deda、d8763972、20b10c80＋收尾一筆）：方案 [plans/vn-mode-pkg9.md](../plans/vn-mode-pkg9.md)。
  - Sol 方案審 2 輪：必改 3＋0 全採納，串 01a1975e。驗收 2 輪：必改 1＋0，串 01a19798。
  - 落地：後端 `import/vn_card/`（schema、remap、materialize、export）、`commands/vn_card.rs`、`vn.json` 加 `card`（卡原件、逐角色欄位、美術欄位原樣保存）、`Bible.chapter_lock`、恢復卡內原版（`vn_bible_generate` 在有原版的卡桌改走恢復）、GM 總則／玩家段／導演指示的卡定主角與 `branching`／人稱句、出場限制段；前端 `features/vn/card/`、精靈從 ③ 起、VN 工具列 ⋯「匯出成 VN 卡」、劇本分頁恢復鈕與章數唯讀。
  - 規模：核心約 1500（立案估 450，重估 900–1300），測試約 600；verify 13 步綠。
  - 通道真模型（luna）實測六項通過，主線親看截圖；SunSmell 測試卡在 `TestCards/sunsmell-vn-card.json`／`.png`。
  - 已知限制見方案 §4；觀察見方案 §5（`[chapter]` 未寫屬包 5、`[bg]` 自由地名屬 9b）。角色線內心獨白洩漏私設另立案 [vn-thought-spoiler](../tasks/vn-thought-spoiler.md)。
- 包 9b（卡內美術欄位接進生圖）2026-11-02 完成，在分支 `vn-mode`（commit 6da5cf5f、18dd5182＋收尾一筆）：方案 [plans/vn-mode-pkg9b.md](../plans/vn-mode-pkg9b.md)。
  - Sol 方案審 2 輪：必改 3＋0 全採納，串 01a197a8。驗收 1 輪 PASS 無必改，串 01a197f5。
  - 落地：卡美術欄位改型別並在匯入時驗（`data/vn_art.rs`、`import/vn_card/art.rs`，匯出寫回原名）；外觀規格取代公開設定、卡畫風與避免項進立繪與背景提示詞；每角色額外表情鍵（拍板 16）貫穿臉層、佇列、解析器、規範與立繪分頁；卡宣告變體是保留鍵、立繪分頁顯示空格按了才生；卡地點（鍵或名稱比對）與每地點有效時段清單、GM「可用地點」段；參考圖種類列舉、封面當畫風參考；`height_cm` 以名單最高者為基準固定縮放（特寫不套）。
  - 規模：核心約 1230（方案估 750），測試約 590；verify 13 步綠。通道 Codex 真生圖 23 張八項全過，主線親看截圖。
  - 已知限制、待作者裁決三項（變體不自動生、身高基準、主角不上台）見方案 §4、§4.1；分級被拒提示（§2.8）沒做。
- **下一步：包 5**（章、結局、完結畫面；見總綱 §8、§11 包 5 列）。包 4／9a 實測觀察：Haiku 不回報旗標、GM 不寫 `[chapter]`，包 5 要處理遵守度。之後順序：6 → 8。
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
