# vn-mode 包 9 方案：初始 VN 卡（9a 容器、匯入、匯出、卡定主角）

總綱 [vn-mode.md](vn-mode.md) §15（初始 VN 卡）、§2（卡定主角）、§3（`branching`）、§8（章數鎖）、§11 包 9 列、§12 拍板 16–19；研究報告 [vn-card-sunsmell](vn-card-sunsmell.md) §2 schema 與附錄 A 示範卡；聖經結構與驗證正規化沿用 [包 4](vn-mode-pkg4.md) §2.1。大案、碰信任邊界輸入：方案與驗收都送 Sol。

**拆包**〔作者裁決 2026-11-01〕：9a＝本檔；9b＝卡內美術欄位接進生圖管線（外觀規格、地點與時段、畫風、封面當參考圖、額外表情鍵〔拍板 16〕、變體宣告、身高縮放），緊接 9a、排在包 5 前，另寫方案。9a 匯入時把美術欄位原樣存下，匯出原樣帶回，不遺失。

核心立案估 450（整包）；9a 主線重估 900–1300（後端約 700、前端約 350），照流程上限只記不停工。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 1 業界做法與我們的對應

| 業界 | 我們 | 差距 |
|---|---|---|
| Character Card V2／V3（`chara_card_v2`；`data.extensions.<命名空間>` 給 app 放私有資料、規範要求保留不認得的 extensions；V3 另有 `.charx` zip 帶資產）；分發在 chub.ai、RisuAI、Agnai，單位是一張 PNG＋嵌入 JSON | 容器用 V2 PNG tEXt `chara`（也收同內容 `.json`），VN 內容放 `data.extensions.table_tavern.vn` | 一致；ST 重存是否保留我們的 extensions 未實測 |
| 不信任輸入：schema 先行、格式帶版本、未知欄位忽略、解析前限大小（JSON Schema；Rust `schemars`＋`jsonschema`） | serde 型別解析＋逐欄驗證夾斷、`version` 不認得退普通卡、未知欄位忽略、沿用卡檔既有大小上限 | 沒對卡作者公開 JSON Schema 檔，v1 只有本檔欄位表 |
| Ren'Py／Naninovel 發行寫死劇本；TRPG 模組 GM 本、編劇節拍表 | 卡只帶聖經（節拍、錨句、結局表）與開場，對白由模型即興 | 新形式；節拍與錨句的遵從度要本包真模型實測 |

## 2 定案設計

### 2.1 卡格式（`data.extensions.table_tavern.vn`，`version: 1`）

ST 標準欄位照報告 §2.2 填：`name`＝作品名、`description`＝世界概要＋各角色公開設定、`first_mes`＝開場去標記的散文版、`character_book`＝世界書條目（含既有 `extensions.table_tavern.visibility`）、`creator_notes`＝分級與「在 Table Tavern 以 VN 模式遊玩」。我們匯入時**不讀** `description`／`first_mes`（以 `vn` 內的欄位為準），只讀 `name`（無 `vn.title` 時當桌名）與 `character_book`。

```
vn: {
  format: "tt-vn-card", version: 1, lang,
  rating: { level: "general"|"teen"|"adult", warnings: [字串] },
  protagonist: { mode: "fixed", id: 卡內角色id } | { mode: "player" },
  pov: "first"|"second"|"third",          // 旁白人稱，進回合尾導演指示一句
  world: 字串,                              // 世界概要 → world.md
  opening: 字串,                            // 劇本格，第一則 GM 事件
  cast: [{ id, name, color?, emoji?, role?: "main"|"support"|"memory"|"deity", appears?, public, private? }],
  bible: <與 vn.json 的 bible 同一格式（包 4 §2.1）>＋ chapter_lock?: bool,
  art?, locations?, cast[].appearance?, cast[].expressions_extra?, cast[].variants?, cast[].height_cm?   // 9b 消費；9a 原樣保存
}
```

與報告 §2.3 的差別（都為了直接重用 `parse_bible`／`normalize`，不另寫一套聖經解析）：
- `bible` 照搬 `vn.json` 的格式：`beats` 是字串不是陣列、`end_condition` 不叫 `end_when`、結局條件就是既有 `Condition` 序列化。
- 刪掉三項：`protagonist.mode: named`、`style.glossary`（用語規則寫進 `bible.style`）、`chapters[].locations／cast`（預生背景屬 9b）。
- `chapter_lock` 改掛在 `bible`。
- `pov` 改成平鋪字串。

`chapter_lock`〔作者裁決 2026-11-01 拍板 17〕：加進 `Bible`（serde 預設 false）。true 時劇本分頁與精靈 ② 的章數欄唯讀（看得到改不了），`vn_bible_save` 也拒收章數變更。

### 2.2 偵測與分流

- `probe_import` 多回 `vn_card: null | { supported, title, rating, warnings, cast }`：有 `data.extensions.table_tavern.vn` 且 `format == "tt-vn-card"` 就填；`version` 不是 1 時 `supported: false`。
- 前端兩個入口（一般匯入 `importFile`、精靈 ①「我有角色卡」）都先看 `vn_card`：
  - `supported`：跳分級確認框（作品名、分級、警示、「卡內含劇本與秘密的明文」），確定後走 VN 卡匯入；不問身分與桌型，一律開新 VN 桌，不匯進目前這桌。
  - 不 supported：提示「這張 VN 卡的格式版本不認得，改當普通角色卡匯入」，照普通卡流程。
  - 要確認 `hasRefactorCardChunk`／`looksLikeWebSave` 不會搶先吃掉 VN 卡 PNG。
- 退普通卡時，既有 `import/mechanism.rs` 遇到只有 `vn`、沒有 `version`／`rules` 的 `table_tavern` 不得報錯（施工時補測）。

### 2.3 匯入建桌（後端一支命令，整桌原子）

照網頁存檔匯入的模式：`create_world_exclusive`（加桌型參數，這裡建 `vn` 桌）拿到新桌獨占後一次建完，任何一步失敗就刪掉整桌目錄，不留半桌；成功回 `{ world_id, notices }`。

1. 解析：`parse_character` 同一套（PNG／JSON、卡名）。`vn` 用 serde 解成型別、未知欄位忽略。逐欄驗證：
   - id 照 `[a-z0-9_]{1,32}`；`cast` 1–8 人，超過取前 8；`cast.id` 重複時留第一個、後面的整筆丟掉＋notice（重對應表不得被覆蓋）。
   - 名字過 `validate_single_line`，色碼要是 `#rrggbb`，不合格就換預設色。
   - 文字欄夾斷：`public` 1,500、`private` 2,000、`world` 1,500、`opening` 2,000、`appears` 200；聖經照 `normalize` 既有上限。
   - 整個 `vn` 擴充序列化後超過 100,000 字元，先丟 9b 欄位再判；還超過才拒收。
   - 每項夾斷或丟棄都記一條 notice。
2. **id 重對應（信任邊界核心）**：卡內 `cast[].id` → `data::new_id()` 建表，套到：
   - `protagonist.id`：對不到就退 `player` 模式＋notice；
   - `bible.secrets[].holder`：對不到就設 null；
   - 世界書條目的 `visibility.characters`：對不到的 id 剔除，剔空了就退 GM 可見。

   `normalize` 收的 `cast` 是重對應後的（id, name）。
3. 寫入：
   - `world.md`；
   - 每個 cast 一張角色卡（`public_md`、`private_md`、色、emoji）；
   - 世界書條目，沿用既有卡片隨身書收編，用 `BookWrite::Strict`；
   - **套用聖經走包 4 同一條路**：把 `vn_bible::install` 的套用部分抽成可在建桌獨占內呼叫的函式，一次寫好 `vn.json`（`{spoiler_lock: true, bible, card}`，見 2.4）、旗標與 `revealed`／`chapter` 的狀態初值、機制規則、`mechanism.incremental`、受保護根與快照同步；不另寫一套。沒有聖經（`bible: null`）就只寫 `vn.json`；
   - 開場：照 `genesis::materialize` 寫一則 `opening: true` 的 GM 旁白事件，不走開場白面板；
   - 封面：PNG 卡的底圖過 `png_clean` 存 `vn/cover.png`，JSON 卡沒有封面。

   卡定主角另外寫 `player_card_id`＝主角的角色 id。
4. 不建「作品名」那個角色。
5. 前端拿到 `world_id` 後，精靈從 ③ 立繪開始（`VnWizardStart` 多一個起始步）；notices 在進精靈前一次顯示。

### 2.4 卡原件與「恢復卡內原版」

`vn.json` 多一欄 `card: null | { rating, pov, protagonist_fixed: bool, original_bible, extras }`：
- `lang`、`rating`、`pov`：卡頂層原值。
- `original_bible`：匯入時 normalize 後的聖經。
- `cast: { <桌內角色id>: { role, appears, extras } }`：逐角色的出場性質、出場限制白話、9b 美術欄位原樣 JSON（夾斷後）；以**桌內角色 id** 為鍵，角色刪除後該筆在匯出時略過。
- `extras`：卡層級的 9b 欄位（`art`、`locations`）原樣 JSON，夾斷後保存。

重對應雙向：匯入卡內 id → 新桌內 id（2.3 第 2 步），`card.cast` 的鍵用新 id；匯出桌內 id → `c1…cN`（2.7），`holder`、可見性、主角 id、`card.cast` 的逐角色欄位一起換。

VN 卡桌在精靈 ② 與劇本分頁的「重骰聖經」改成「恢復卡內原版」：在提交鎖內重驗未遊玩，然後走 2.3 同一支套用函式（聖經、狀態初值、機制規則、快照一起同步），不能只換 `bible`。私設重骰照常可用。

章數鎖一律以提交鎖內讀到的既有 `chapter_lock` 判定、儲存時保留原值；`vn_bible_save` 不接受請求順帶改 `chapter_lock`。

### 2.5 卡定主角（`protagonist_fixed`）

借玩家卡的槽位：主角＝`player_card_id`，所以既有邏輯自動把主角排出「登場角色」名單、不開角色線、選項事件記在主角名下。理由：總綱 §2「也不是玩家卡」是〔模型判斷·未裁決〕，借槽位省下一整套名單排除。差別只在下面三處，條件都是 `card.protagonist_fixed`：
- **GM system 總則**：`context.rs` GM 總則「名單角色與玩家各有扮演者，不要替他們代言」在固定主角桌改成「名單角色各有扮演者；主角由你寫台詞、行動與內心獨白，玩家只在選項點替主角做決定」（zh／en 兩版都改）。
- **GM system 玩家段**：段標改成「主角（由你寫台詞與內心獨白；玩家只在選項點替主角做決定）」，並附主角的私設。目前 `context.rs` 只附公開設定。
- **導演指示**：把 `vn_narrate_instruction` 的「不要替名單角色或玩家說話」換成「不要替名單角色說話；主角的台詞與獨白由你照劇本格式寫；選項＝主角這一拍的行動或台詞」。
- **前端**：玩家卡切換入口在 VN 卡定主角桌鎖住（顯示主角名、不可換）；若入口分散難鎖，列已知限制。

主角不在名單，解析器把主角對白當沒卡配角，舞台不放主角立繪（第一人稱 VN 慣例）。9b 再議要不要給主角立繪。

### 2.6 導演指示：`branching` 與人稱

- `vn_narrate_instruction` 多吃 `branching` 與 `pov`，來源是 `vn_bible::turn_inputs` 已讀的凍結聖經；沒有聖經就照現狀。
- `none`：加一句「選項只改語氣、節奏、好感，不改本章目標與結束條件」。AI 生成的 `none` 聖經也適用。
- `pov`：加一句旁白人稱。只有 VN 卡帶 `pov`，生成的聖經沒有就不加。
- `role: memory／deity` 與 `appears`（讀 `card.cast`）：進 GM 常駐段的「登場者出場限制」一行一人，只有有填的才列。

### 2.7 匯出成 VN 卡

VN 桌工具列「更多」→「匯出成 VN 卡」。流程：
1. 先跳明文提醒（「含私設與劇本，打開卡就看得到」）。
2. 存檔對話框，`.png`／`.json`。
3. 後端反向組卡：
   - `vn.bible` 用目前聖經（不是凍結副本），並帶上 `chapter_lock`；
   - `cast` 用現有、未封存、非主角的角色；卡定主角另外放進 cast，並寫 `protagonist.fixed`；
   - 角色 id 匯出時重新編成 `c1…cN`，聖經的 `holder` 與世界書可見性一起換；
   - `world` 用 world.md，`opening` 用第一則 `opening: true` 事件；
   - `extras` 原樣帶回；`rating`／`pov` 從 `card` 取，沒有就填 `general`＋空警示、`third`；
   - 世界書全部條目都進 `character_book`，帶可見性。GM 條目也帶，理由是卡本來就是明文（總綱 §13）、匯入時才不會遺失。
4. ST 標準欄位照 2.1 填。
5. 封面依序選：`vn/cover.png` → 匯出時讓玩家挑一張 PNG（可取消）→ `blank_png`。PNG 嵌入沿用 `export.rs` 的 `embed_chara_chunk`。
6. 沒有聖經的 VN 桌也能匯出，`bible: null`，匯入端照沒聖經處理。

### 2.8 落點（照 [STRUCTURE](../../docs/STRUCTURE.md)）

- 後端：
  - 新資料夾 `src-tauri/src/import/vn_card/`：`schema.rs`（型別、驗證夾斷）、`remap.rs`（id 重對應）、`materialize.rs`（建桌）、`export.rs`（反向組卡）。
  - 命令放 `commands/vn_card.rs`。
  - `data/vn.rs` 加 `card` 欄與 `chapter_lock`。
  - 導演指示改 `transport/vn_format.rs`，玩家段改 `transport/context.rs`。
- 前端：
  - `features/vn/card/`：分級確認框、匯出鈕的 hook。
  - `useImportController.ts`（771）與 `App.tsx`（1152）只加薄接線。
  - `VnWizard` 加起始步。
  - `BibleTab`／`VnBibleStep` 換恢復鈕、章數欄唯讀。

## 3 測試

- 單元測試：
  - schema：缺必填、壞 id、超量 cast、夾斷 notice、版本不認得、未知欄位忽略。
  - id 重對應：懸空 holder、主角、可見性。
  - 原子建桌：中途失敗不留桌目錄。
  - 匯出與匯入來回：聖經（含 `holder`）、角色、世界書可見性、開場、主角、`lang`／`rating`／`pov`、逐角色 `role`／`appears`／extras、卡層級 extras 逐欄比對一致；匯入後與恢復原版後的狀態初值與機制規則和包 4 生成路徑一致。
  - `cast.id` 重複：後者丟掉、主角／秘密／可見性不會接到別的角色。
  - 導演指示三種分支：none、fixed、pov；固定主角測**整份組裝後的 GM 提示詞**（總則、玩家段、回合尾三處都不再禁止代演主角）。
  - `chapter_lock` 拒收章數變更、請求帶 `chapter_lock: false` 不會解鎖；恢復原版只限未遊玩。
- repo 內小 fixture 放 `import/vn_card/` 測試目錄，2 角色、3 章。SunSmell 示範卡照報告附錄 A 組成 JSON 放 `TestCards/`（gitignore，不進 repo）。
- **測試通道真模型實測**（截圖主線親看）：
  1. SunSmell 卡匯入：分級框 → 建桌 → 精靈 ③ → 開演。
  2. 卡定主角：GM 寫主角台詞與獨白，主角不被點名接話。
  3. 遵從度：前兩章節拍有沒有走到、錨句有沒有原樣出現、`none` 下選反方向選項時章目標不變。
  4. 劇本分頁「恢復卡內原版」與章數鎖。
  5. 匯出成卡後重新匯入，內容一致。
  6. 版本不認得退普通卡。

## 4 已知限制

- 主角沒有立繪（9b 再議）。
- 玩家卡沒有切換入口，所以主角不必另鎖；在角色頁刪掉主角，那桌退回一般 VN 桌的寫法。
- 主角私設不經巨集代換。
- 匯出最多 8 人（含主角），超過的直接截掉、不提示；舞台卡與封存角色不帶。
- 聖經走包 4 `normalize` 的截短不出匯入提示。
- 整張卡超過 10 萬字時美術欄位整批丟掉，不逐欄截短。
- 唯讀的章數欄在 WebKit 仍顯示上下箭頭（點了無作用）。
- ST 重存是否保留我們的 extensions 未實測。
- 卡內文字照作者語言，不翻譯。
- 匯出不帶立繪、背景、紀錄（總綱 §13）。

## 5 觀察
- 9a 實測（luna）：第 1 章節拍全走到、兩句錨句原樣出現；到「踏上碼頭」後 GM 沒寫 `[chapter]` 直接演第 2 章（`[chapter]` 規範在包 5）。
- GM 的 `[bg]` 寫 `ferry`、`日嶼碼頭` 這類自由地名，沒對卡的 `locations` slug（9b 接背景時處理）。
- 角色線把私設寫進內心獨白（老婆婆 thought：「和也的孩子」），VN 播放時玩家看得到，等於提前揭露秘密；屬包 1 角色線規範，與 VN 卡無關。

