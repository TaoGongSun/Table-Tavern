# gm-no-state-fence-worldbook-table 根因與做法

狀態：結案 2026-10-27。送審與驗收皆三方（Opus、Sol、Grok）共識。未標〔作者裁決〕的都是〔模型判斷·未裁決〕。

## 共識與已知限制

- 判定收緊為「字面等於開場白完整一行」，Sol、Grok 驗收都收。理由：只看「出現在開場白裡」會把 `/她/gi` 這類代名詞替換誤收成開場白美化。
- 開場白美化判定是依形狀推測卡片作者的用途，不是精確辨識；判不出一律維持原判定（讓路給卡片格式）。
- 輸出順序是提示契約不是強制：實測 19/20 照順序，違反時圍欄照樣抽到、卡片段落照樣顯示。
- 未重構真介面桌的三欄回報另案 interface-table-state-report；卡片格式的 HTML 註解被當純文字顯示另案 card-format-html-comment-shown。

## 根因

立案猜「世界書格式壓過圍欄指示」不對：**圍欄指示根本沒送出**。GM 實送提示詞（CLI session 落檔）回合尾是中性版
「（導演指示）請推進本回合的劇情，遵循上下文中已有的卡片格式要求與狀態更新協定……」，整段沒有 ```state。

1. **分類錯（主因）**：`import/interface.rs` `is_display_script` 只看「輸出後套用、啟用中」，WestFantasy_NOUI 唯一的腳本
   「开场白」（`\s*请选择你的身份\s*` → 角色選擇選單，markdownOnly）被算成卡片自帶介面 →
   `chat_assembly.rs:294` `gm_turn_format(!card_scripts.is_empty(), …)` 回 CardFormat → `card_format_entry` 從 find_regex
   抽不到 `<Tag>` → CardFormatAbsent → `response.rs:208` `card_format_instruction(lang, None)`。同一份判定在
   `data/world.rs:365` `requires_present_report` 重複一次（換幕結算也當這桌「沒被要求報 present」）。
2. **指示措辭擋掉卡片狀態欄（修 1 之後立刻浮現）**：`narrate_instruction` 的「圍欄以外不要有多餘說明」與收尾句
   「只輸出旁白本文、要求的狀態欄與『下一位』行」讓模型把世界書「回复规则」（depth 1）要求的四段狀態欄全丟掉。
   這影響所有沒腳本、但世界書自帶狀態欄格式的一般旁白桌，違反酒館行為照做。

## 實測（Haiku 5.5，claude CLI 單發，同一份實送提示詞；成人段落已去除：system 拿掉「奴隶贸易」節、回合尾拿掉 🍆／🗝 行、
「性爱」改「亲密关系」；n＝5／組，共 25 次 $0.064）

| 組 | 回合尾 | ```state 三欄有值 | 世界書四段狀態欄 |
|---|---|---|---|
| absent（現況重現） | 中性版 | 0/5 | 5/5 |
| narr（只修分類） | 現行旁白指示 | 5/5 | 0/5 |
| c（分類＋措辭） | 旁白指示改三處 | 5/5 | 5/5 |
| c_plain（拿掉世界書尾段的對照） | 同 c | 5/5 | 無多餘輸出 5/5 |

c 組排法：正文 → 卡片狀態欄（`---` 分隔）→ ```state → 下一位，與酒館卡「狀態欄接正文後」慣例一致。

## 做法（送審第 1 輪共識：Sol、Grok 同意根因、helper、措辭方向；以下為合併後定案）

1. **開場白美化腳本不算卡片自帶回覆格式**（`import/interface/opening.rs`），全部成立才算，判不出的形狀一律維持原判定：
   - find_regex 是純字面：有 `/…/flags` 外殼時 flags 只能是 g i m s u y（i＝大小寫不分比對）；去掉首 `^`、尾 `$`（跳脫的 `\$` 不算）
     與首尾任意個 `\s*`／`\s+` 後，不得剩未跳脫的 `. ^ $ * + ? ( ) [ ] { } |`；跳脫只認 `\` 後接非英數字（取字面），
     `\d`、`\n`、`\u…` 等判不出；去完是空字串判不出。
   - 保險 a：字面含 `< > { } [ ] 【 】` 任一個（格式標籤、`{{…}}` 佔位符、`【開局】` 錨點）不算——同一字面可能也是每回合格式。
   - 保險 b：replace_string 含 `<script`，或含 HTML 標籤且長達 1000 字元，不算（換成的是前端）。
   - 字面須等於該卡某則開場白（first_mes 或 alternate_greetings，組旗標時從原卡讀）裡完整的一行（前後空白不計）；
     只是開場白裡的一個詞（例如代名詞替換 `/她/gi`）不算。〔模型判斷·未裁決：比送審版多收緊這一條〕
   - 旗標 `InterfaceScript.opening_only` 只在 `card_interface()` 讀原卡時算，`#[serde(skip)]`（反序列化回來恆 false），
     判定一律從 `read_card_interfaces` 的結果拿；`is_display_script` 與前端拿到的腳本清單不動。
   - 不採「mvu 為真一律維持原判定」〔主線裁決：mvu 擋不住非 MVU 前端，且會讓只有開場白字面的 MVU 卡繼續沒三欄〕。
   - 普查 `TestCards/` 19 張有顯示腳本的卡：只有 WestFantasy_NOUI 翻成一般旁白，其餘照舊 CardFormat。
2. **兩處判定共用 `import::reply_format_scripts(root, world_id)`**（可用、非開場白美化的顯示腳本），
   `chat_assembly::gm_turn_instruction` 與 `data::requires_present_report` 都用它。
3. **旁白指示放行卡片規定的附加內容，輸出順序寫死**：正文 → 世界書／角色設定規定每回合要附的內容 → ```state →
   有增量才 `<UpdateVariable>` → 下一位。zh 用實測 c 組字串，en 對譯；`narrate_instruction`、`gm_closing` Narration 兩態、
   `narration_closing_with_updates` 四態都改。CardFormat、中性版、接管分支不動。指示在回合尾段，不動凍結 system。
4. **檔案肥大**：兩檔測試純搬到 `transport/response/tests.rs`、`import/interface/tests.rs`（比照 `transport/state_view.rs`＋
   `state_view/tests.rs` 先例），名稱、斷言、收集數不變。

## 業界做法與差距

- SillyTavern 不插任何導演指示，「回复规则」這種 depth 1 常駐條目就是唯一的格式權威，也沒有 app 側狀態追蹤，所以不存在共存問題。
  Table Tavern 的 ```state 圍欄是 app 為登場／在場裁切額外要的，差距在「兩份格式要求同時存在時誰讓誰」——答案是並存：
  卡片狀態欄照卡附在正文後，圍欄最後附，app 剝掉圍欄、卡片狀態欄照常顯示。
- 本卡「开场白」腳本的預期用途是開場白美化；ST 依 placement、用途與深度套用 regex，後續回覆出現同一字面仍可能命中，
  所以這裡只把「字面是開場白完整一行、且不像格式標籤或前端」的形狀排除在「卡片自帶回覆格式」之外。
- 酒館卡慣例：狀態欄接在正文後、用 `---` 分隔；實測輸出照這個排法。
- 未重構的真介面桌（WestFantsy UI 版、bcd368、DongeonMaster）走 CardFormat／中性版，不保證回報 time／place／present
  （`extract_state_block` 仍可能從卡片狀態格式抽到）；主線另立案，本案不動中性版〔作者裁決 2026-10-04〕。

## 改動

| 檔案 | 內容 | 行數（前→後） |
|---|---|---|
| src-tauri/src/import/interface/opening.rs（新） | 開場白美化判定 | 0→98 |
| src-tauri/src/import/interface.rs | 旗標、`reply_format_scripts`；測試搬出 | 939→429 |
| src-tauri/src/import/interface/tests.rs（新） | 純搬＋新測試 | 0→759 |
| src-tauri/src/transport/response.rs | 措辭；測試搬出 | 1157→693 |
| src-tauri/src/transport/response/tests.rs（新） | 純搬＋抽取後顯示文字測試 | 0→493 |
| src-tauri/src/chat_assembly.rs、src-tauri/src/data/world.rs | 改用 helper | 572→567、844→837 |
| src-tauri/src/lanes/scaffold_baseline/zh-TW、zh-CN、zh-HK.txt | 重生 | — |

## 驗收用去成人卡（本機限定）

`TestCards/WestFantasy_NOUI-safe.png`（gitignore）：拿掉「奴隶贸易」「阿拉克妮娅地穴」兩條；各條目刪含 🍆／🗝／性欲／奴隶／调教／交媾 的行；
「趋向依靠性爱解决问题」改「亲密关系」；description（data 與頂層兩份）只留「你必须严格按照」之前；全卡字串 调教→训练、奴隶→佣工、
色欲→虚荣、性奴→俘虏。腳本與其餘條目原樣。產生腳本（暫存，可能被清）：
`/private/tmp/claude-501/-Users-<user>--claude-spares-spare-yfWgps/853b218d-0b3d-4f00-b3b6-0b81bbbd6996/scratchpad/fence-tools/make_safe.py`。

不碰另三案的檔（card-interface controller、mvu shim、mvu_engine/wiring.rs、refactor/apply.rs、interface-card.ts、useImportController.ts）。

## 驗收結果

- `npm run verify` 12 步綠（cargo 1973 過＋6 ignored，新增 5 項；vitest 1563）。純搬那筆 commit 前後 interface／response 測試名稱 42 項逐一相同。
  第一次 verify 紅在 `mvu_engine::tests::abandoned_threads_are_capped_and_counted_until_they_exit`（已立案 mvu-abandoned-threads-flaky 的負載假紅），單跑過、重跑全綠。
- 新測試：開場白美化正例（首尾 `\s*`、`/…/gm`＋跳脫、alternate_greetings、i 旗標）與 12 個維持原判定反例（代名詞、萬用式、群組、`\d`、未知旗標、
  空字面、不在開場白、`<StatusBlock>`／`\{\{…\}\}`／`【…】` 佔位符、script、長 HTML）；混合卡只留回覆腳本、regex 關閉一支不留；
  chat_assembly 與 requires_present_report 兩端一致（只開場白→旁白＋圍欄、混合→讓路）；四段 ```json 狀態欄在抽取後留在顯示文字、圍欄與點名被剝。
- 實模型探針（Haiku 5.5，實際組裝的提示詞、去成人段落的證據桌，n＝5／組）：
  | 組 | 圍欄三欄 | 世界書四段 | 順序（段落→圍欄→更新→下一位） |
  |---|---|---|---|
  | zh 有名單 | 5/5 | 5/5 | 5/5 |
  | zh 有增量 | 5/5 | 5/5（1 次沒有 💜 emoji，人工核對四段齊） | 5/5 |
  | en 有名單 | 5/5 | 5/5 | 5/5 |
  | en 有增量 | 5/5 | 5/5（1 次沒有 💜 emoji，人工核對四段齊） | 4/5（1 次把卡片段落放在圍欄後；圍欄照樣抽到、段落照樣顯示） |
  第一輪探針見上方實測表。探針合計 45 次 $0.379。
- 測試通道（本 worktree harness 包、獨立 root、claude CLI Haiku 5.5）：匯入 `WestFantasy_NOUI-safe.png`＋艾琳，玩家一回合＋GM 推進兩回合，
  GM 四次回覆 4/4 附 ```state 三欄有值、頂部狀態列顯示「時間 上午｜地點 邊境小鎮冒險者公會｜在場：艾琳」、四段卡片狀態欄 4/4 留在畫面；
  範例桌（迷霧酒館）一回合只有正文＋圍欄＋點名，沒多吐東西。6 次呼叫 $0.038。

## 另外發現（不在本案範圍）

- 卡片格式裡的 `<!-- 隱藏記憶欄 -->` HTML 註解被模型照抄時，畫面把它當純文字顯示（ST 的渲染會藏起來）。舊的中性版也會出現（第一輪探針 absent 組 2/5），
  不是本案引入；本案讓模型更常照抄卡片格式（測試通道 4/4）。建議另立案處理顯示層。
