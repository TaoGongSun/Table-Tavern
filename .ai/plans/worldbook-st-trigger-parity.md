# worldbook-st-trigger-parity 施工方案

交接見 [handoffs/worldbook-st-trigger-parity.md](../handoffs/worldbook-st-trigger-parity.md)。行號以分支起點 `7883052` 為準。網頁版公開前門檻（[web-version](web-version.md) D17）。

## 一、現況

### 桌面版觸發
- 唯一觸發函式 `active_worldbook_entries`（`src-tauri/src/transport/context.rs:8-32`）：未停用且（constant，或任一主鍵小寫後是最近 4 則事件小寫文字的子字串），依 `(order, uid)` 遞增排序。沒有次要鍵、掃描深度設定、全字比對、正則鍵、機率、遞迴、預算、計時、群組、插入位置。
- 條目精簡檢視 `WorldbookEntry`（`data/worldbook.rs:23-38`）只有 uid、標題、主鍵、內文、constant、order、停用、可見度、`is_person`、`locked`；`entry_view`（`:227-270`）讀原始 JSON，其餘 ST 欄位留在 `worldbook.json` 原檔裡沒人讀。
- 原始條目有兩種形狀：
  - 編輯器、重構新建的條目是 ST 物件形（`new_entry_value`，`worldbook.rs:389-428`）。
  - 角色卡路匯入的 V2 `character_book` 條目只改了四個欄名（`keys`→`key`、`secondary_keys`→`keysecondary`、`insertion_order`→`order`、`enabled`→`disable`，`data/worldbook/book_import.rs:243-258`），`position` 還是 `"before_char"` 字串，機率、計時、群組、深度等都還在 `extensions.*` 的 snake_case 底下。兩種形狀混在同一本書裡，讀的人分不出來。
- 匯出 `v2_entry`（`import/export.rs:230-307`）只處理 `position`、`case_sensitive`，其餘 ST 物件形欄位（`probability`、`sticky`、`group`…）留在條目頂層，ST 匯入時讀不到。

### 消費端（六個組裝點；直接呼叫 `active_worldbook_entries` 的有四處：`assemble.rs:113`、`turns.rs:194`、`turns.rs:325`、`context.rs:54`，兩條凍結 system 直接讀書）
| 路 | 位置 | constant | 命中的 keyword |
|---|---|---|---|
| GM 續聊線凍結 system | `transport/turns.rs:288-308` `gm_lane_system` | 全部（不看觸發，直接讀書） | — |
| GM 續聊線回合尾 | `turns.rs:311-345` `gm_lane_turn` → `state_view.rs:16` `gm_dynamic_block` | — | 尾段「世界書」段 |
| GM 單發（API） | `transport/assemble.rs:95-148` `assemble_gm_messages` | system（`context.rs:80` `gm_system_prompt`） | 尾段動態塊 |
| 角色共線凍結 system | `turns.rs:62-141` `chars_lane_system` | 只收 `Public`（不看觸發，直接讀書） | — |
| 角色回合尾 | `turns.rs:167-284` `chars_lane_turn` | `Characters` 限定的：hoist 時進 system，否則機密段 | `Public`→公開段；限定→機密段 |
| API 共線 | `assemble.rs:33-84` `assemble_shared_messages`（呼叫上兩者，單卡桌 hoist） | 同上 | 同上 |

- hoist 形狀（單人在場的 Claude、Agy 一角一線、單卡 API）見 `lanes/mod.rs:169-200`；Agy 回合後不抹尾段。
- 另外兩處重算觸發：`gm_prompt_full_entries`（`context.rs:48-70`，給「導演指示能不能點名格式條目」，呼叫點 `chat_assembly.rs:123`；`import/interface.rs:285` 是使用說明）；容量量測 `scene_budget/measure.rs:150-215` 照實送組裝各路徑。
- 角色側觸發看 `render_for_prompt(Side::Character)`（`turns.rs:192-194`），GM 側看 `Side::Gm`。
- 每次聊天呼叫都持有整桌寫入許可（`commands/chat.rs:79`、`:344`），同一桌的呼叫是序列的。
- 換幕摘要是帶 `SceneSummary` 標記的 `Narration`（說話者 `GM`），不是 `System`（`data/scene/lifecycle.rs:143-160`）。
- 旁白一律以 `GM` 為說話者；網頁存檔匯入的開場白、世界書路的角色訊息也存成 `Narration`／`GM`（`import/web_save/mod.rs:295-304`）。網頁版與 ST 這裡用卡名。
- 角色卡路的書：陣列形照索引展開、依此配 uid，重複 `id` 的兩條都留（`import/card.rs:200-215`、`book_import.rs:93-160`）；網頁版照 `entry.id` 決定順序、重複時後一條蓋前一條（`world-info-book.ts:189-197`）。
- 去重指紋只看標題、內文、兩組鍵、constant、`source_disable`（`book_import.rs:405-453`），機率、計時、群組、位置不同的條目也會被合併；再匯入命中時 `merge_entry_into` 只併可見度與來源卡（`:344-376`）。
- 巨集只有 `replace_st_macros`（`transport/messages.rs:85`，只換 `{{user}}`／`{{char}}`），呼叫點 25 處，見三之 7。
- 人物條目（`is_person`）：constant 的只進名冊行（`context.rs:408` `split_person_roster`），登場時全文由事件帶進歷史（`transport/arrivals.rs:20-50`）。

### 網頁版（照 ST 06bde939 完整做過，web-version 包 5，`c366dde`）
- 掃描核心 `web/src/features/sillytavern/world-info-scan.ts`（`checkWorldInfo`，含 Buffer、TimedEffects、群組篩選），條目轉換 `world-info-book.ts`（V2 陣列形照 `convertCharacterBook`、物件形照 `newWorldInfoEntryDefinition` 預設、`@@` 裝飾、ST 載入順序、`sortByOrder`），組提示 `web/src/features/chat/prompt.ts:107-160`（前／後固定段、範例上下、作者註記上下與依深度插入走 `injections.ts`、outlet 給 `{{outlet}}`）。
- 設定：ST 預設（深度 2、預算 25%、全字、不分大小寫、含名字、遞迴開）；載 MVU 的卡用 MVU 推薦值（D33：預算 100%、不全字、不含名字）。
- 只有單卡、單一說話者；沒有可見度、沒有多條注入路。
- 計時存檔契約：`src/shared/contracts/web-save/web-save.md` 第三節（sticky／cooldown 以穩定 ID 為鍵、單位是訊息則數、`last_message_id`、退回靠 start／protected）。

### 網頁存檔匯入（桌面版只存不用）
- 旁檔 `worlds/<id>/web-save.json` 原樣存 `world_info`，另附「穩定 ID→桌面 UID」「訊息 id→桌面事件 id」兩張表（`import/web_save/mod.rs:355-424`）。
- 逐字稿一則訊息對一則事件、全部進第 0 幕，不產生系統事件（`web_save/mod.rs:218`、`:295-304`）。

### 無法共用執行碼
網頁版是 TS、桌面版是 Rust，桌面版不跑 JS。共用的是**行為規格與對拍 fixture**：Rust 端照網頁版逐行移植，兩邊跑同一份案例（見四）。

## 二、ST 行為對照表

「本案做法」欄的「照網頁版」＝照 `world-info-scan.ts`／`world-info-book.ts` 同一行為移植到 Rust，並進對拍 fixture。

| 項目 | ST 怎麼做 | 桌面版現況 | 本案做法 |
|---|---|---|---|
| 條目欄位來源 | V2 卡內書經 `convertCharacterBook` 轉成物件形；獨立書檔本來就是物件形 | 兩種形狀混存，只讀精簡欄位 | 匯入時照 `convertCharacterBook` 轉成物件形落檔；讀取端一個 resolver 照 `fromWorldFile` 的預設補欄位；匯出照 `convertWorldInfoToCharacterBook` 收回 `extensions`（三之 5） |
| 載入順序、uid、重複 | 陣列形以 `entry.id`（缺則索引）當鍵，整數鍵遞增、其餘照出現順序，重複 id 後蓋前、位置留在前；物件形照 `Object.keys` | 照索引配 uid、重複都留；物件形照數字鍵、非數字鍵排最後 | 照網頁版 `stOrder` 決定順序並依此配 uid，重複照 ST 只留後一條；物件形非整數鍵要保留原檔出現順序（三之 5） |
| 排序 | `b.order - a.order`，Array.prototype.sort（穩定） | `(order, uid)` 遞增 | 讀取條目時 `order` 一律正規化：有限數字照用，缺欄或不是數字（字串、null、undefined）補 100，比較子因此是全序；Rust 用標準庫穩定排序，不移植 V8 TimSort〔作者裁決 2026-10-10：能簡單做得比 ST 好就不模仿〕；`sort-cases.json` 只放全序案例（含長度 >64、同值穩定性） |
| 主鍵比對 | 先代換巨集再 trim；`/樣式/旗標` 是正則，否則字串；全字比對 `(?:^\|\W)(詞)(?:$\|\W)`、多字詞改子字串；大小寫依條目或全域 | 小寫子字串 | 照網頁版。全字比對的 `\W` 一律寫成 `[^A-Za-z0-9_]`（JS 這裡沒有 `u`，中日韓字元算 `\W`；Rust `\W` 是 Unicode，照搬會讓中文鍵永遠比不中） |
| 正則鍵 | `parseRegexFromString`；`new RegExp` 失敗就當一般字串 | 無 | 照網頁版，經 JS→Rust 轉譯表（三之 1），引擎 `fancy-regex`（新增依賴） |
| 次要鍵 | `selective` 且有次要鍵才看；AND_ANY／NOT_ALL／NOT_ANY／AND_ALL | 無 | 照網頁版 |
| 掃描範圍 | 最近 `scanDepth` 則（全域 2，條目可覆寫）＋可選的全域欄位＋遞迴緩衝；`includeNames` 時每則前面加「名字: 」 | 最近 4 則事件 | 照 ST 預設 2 則（P3）〔作者裁決 2026-10-10〕。訊息清單＝先從原始事件排除 `System` 類與帶 `SceneSummary` 標記的換幕摘要，再交給該視角的 `render_for_prompt`（`arrivals.rs:179` 渲染時會清掉標記，先渲染再排除就認不出摘要；ST `coreChat` 也排除系統訊息；摘要佔名額的話換幕後第一輪 2 則就少一則）；名字規則見三之 2 |
| 全域掃描欄位 | `matchCharacterDescription` 等比對角色卡各欄 | 無 | 角色視角：描述、個性、角色深度提示比對該卡 `public_md`＋`private_md`，人設比對玩家卡 `public_md`，劇本與作者備註沒有對應欄位給空；GM 視角角色欄給空（P7）〔作者裁決 2026-10-10〕；因此觸發的條目照三之 3 走機密段 |
| constant | 直接觸發（仍受停用、計時、群組、機率、預算） | 直接觸發 | 照網頁版 |
| `@@activate`／`@@dont_activate` | 內文開頭裝飾 | 無（裝飾行原樣當內文送出） | 照網頁版：讀取時拆裝飾、內文去掉裝飾行 |
| 生成類型 `triggers` | 條目只在列出的生成類型觸發 | 無 | 照網頁版；桌面版沒有重新生成（`src/features/play/useChatController.ts:438`），一律傳 `normal` |
| 機率 | `useProbability` 且 <100 時擲；sticky 生效中不擲 | 無 | 照網頁版；亂數注入（正式用 `getrandom`，改為正式依賴；測試給固定序列） |
| 遞迴 | 觸發內容加入遞迴緩衝再掃；`excludeRecursion`／`preventRecursion`／`delayUntilRecursion` 分層；超預算停遞迴 | 無 | 照網頁版（最少觸發數、遞迴步數上限都是 0，不走） |
| 預算 | 25% × 本次提示預算（上下文上限－保留輸出）；超過就停，`ignoreBudget` 例外；內文在預算檢查前代換 | 無 | 照網頁版。上限＝該路徑 `scene_budget::limits` 的 `total - reserve`（agy 是 bytes，預算與計數都用 bytes）；`limits::resolve` 回 `None` 時照網頁版當作沒有上限；計數用 `scene_budget/estimate.rs` |
| sticky／cooldown／delay | `chat_metadata.timedWorldInfo`，單位訊息則數；值為 0 等於沒設 | 無 | 照網頁版（0 當沒有計時），存放見三之 4 |
| 群組 | `group`（逗號分隔）、`groupOverride`、`groupWeight`、群組計分；sticky 優先；計時中的移除 | 無 | 照網頁版（含 D31：重複移除不誤刪） |
| 插入位置 | 前／後、範例上／下、作者註記上／下、依深度（含角色）、outlet | 沒有位置，constant 進 system、keyword 進尾段 | 三之 3（P2、P5）〔作者裁決 2026-10-10〕 |
| 角色過濾 `characterFilter` | 依目前說話角色名／標籤包含或排除 | 無 | 網頁版也沒做；桌面版以可見度代替，不做（P8）〔作者裁決 2026-10-10〕 |
| 送模前 regex（WORLD_INFO placement） | 條目內容過卡內 regex | 桌面版沒有送模前 regex（web-version D24 記錄的既有限制） | 範圍外（見八） |
| 巨集 | 完整巨集引擎（含 `{{setvar}}` 等副作用、`{{outlet}}`） | 只換 `{{user}}`／`{{char}}` | 桌面版補齊到網頁版那套 ST 巨集行為，與網頁版對拍（P6）〔作者裁決 2026-10-10〕；三之 6–8 |
| 設定值 | ST 預設；裝 MVU 的 ST 用 MVU 推薦值 | 無 | 照網頁版兩組常數，由視角決定（P9）〔作者裁決 2026-10-10〕：角色視角看該卡、GM 視角看桌上任一張卡或世界書路的卡。「載 MVU」＝卡片介面 `mvu` 為真且 `unsupported` 為空（同網頁版 `play-card.ts:78`），不只看有沒有 MagVarUpdate（`import/interface.rs:206`） |

## 三、做法

### 1. 掃描核心：Rust 照網頁版移植成純函式（包 1）
- 新模組 `src-tauri/src/world_info/`：`entry.rs`（原始 JSON → `WiEntry`，含裝飾拆解）、`scan.rs`（`check_world_info`）、`timed.rs`（TimedEffects）、`settings.rs`（ST／MVU 兩組常數）、`regex_key.rs`（JS 正則轉譯）、`sort.rs`（穩定排序）、`js_semantics.rs`（JS 空白、trim、ToNumber、鍵順序、Math.round）。結構照網頁版一個檔對一個檔，方便逐行對照。
- 輸入：已排序條目、新到舊訊息、預算上限、全域掃描欄位、生成類型、計時表、代換函式、計數函式、亂數函式、設定。輸出：觸發條目（依加入順序）、各插入位置的分組、新計時表、outlet 內容、每條觸發條目的「觸發來源」（見三之 3 機密分流）。不碰檔案、不碰時間。
- 主鍵與次要鍵「先代換再 trim」、內文「在預算檢查前代換」的順序照網頁版，由 fixture 的呼叫紀錄鎖住。
- `WorldbookEntry` 精簡檢視不擴充；掃描端直接讀原始條目 JSON（`read_worldbook_value`）配上 uid。

**JS 正則 → Rust（`fancy-regex`）轉譯表**（`parseRegexFromString` 只收 `gimsuy` 旗標；每一列都進 fixture）：

| JS | 語意 | Rust 寫法 |
|---|---|---|
| 旗標 `g` | `test()` 用新建的 RegExp，lastIndex 從 0 起，不影響結果 | 忽略 |
| 旗標 `y` | 黏著：比對錨在 lastIndex 0；掃描文字以 `\x01` 開頭，所以幾乎永遠不中（Node：`/wolf/y.test("\x01wolf")` 為 false） | 整個樣式包成 `\A(?:…)` |
| 旗標 `i` | 不分大小寫（無 `u` 時是簡單大小寫轉換） | `(?i)`；與 JS 的細微差異見七 |
| 旗標 `m` | `^`／`$` 在行界也成立，行界含 `\n`、`\r`、U+2028、U+2029 | `^` → `(?:\A\|(?<=[\n\r\u{2028}\u{2029}]))`，`$` → `(?:\z\|(?=[\n\r\u{2028}\u{2029}]))` |
| 無 `m` 的 `^`／`$` | 只在整段頭尾 | `\A`／`\z` |
| 旗標 `s` | `.` 也比對行界 | `.` → `(?s:.)` |
| 無 `s` 的 `.` | 不比對 `\n`、`\r`、U+2028、U+2029 | `[^\n\r\u{2028}\u{2029}]` |
| `\d`／`\D` | 有無 `u` 都只認 ASCII 數字（Rust `\d` 會吃全形數字） | `[0-9]`／`[^0-9]` |
| `\w`／`\W` | 有無 `u` 都只認 `[A-Za-z0-9_]` | 明列 ASCII 類別 |
| `\b`／`\B` | 以 ASCII `\w` 判字界 | 用前後看組出 ASCII 字界 |
| `\s`／`\S` | 有無 `u` 都是 JS 空白集合：`\t\n\v\f\r`、空白、U+00A0、U+1680、U+2000–U+200A、U+2028、U+2029、U+202F、U+205F、U+3000、U+FEFF | 明列該集合 |
| 無 `u` 的多餘跳脫（`\/`、`\「`、`\-` 等） | 當字面字元 | 轉成該字元的字面（必要時 Rust 跳脫） |
| 有 `u` 的多餘跳脫 | 語法錯誤 → `new RegExp` 失敗 → 當一般字串 | 轉譯失敗 → 一般字串 |
| 無 `u` 的 `{`、`]` 字面（Annex B） | 不是合法量詞時當字面 | 轉成 `\{`、`\]` |
| `[^]`／`[]` | 任一字元／永遠不中 | `(?s:.)`／`(?!)` |
| `\uHHHH`、`\xHH`、`\cX`、`\0`、`\u{…}`（限 `u`） | 字元跳脫 | 對應的 Rust `\u{…}` |
| 反向參照 `\1`、`\k<n>` | 群組沒參與比對或在參照後面時當空字串比中 | 條件式 `(?(N)\N)`（有捕捉才比，否則空）；具名的照出現順序換成編號 |
| 具名群組 `(?<n>…)`、前後看 | 支援 | `(?P<n>…)`；前後看原樣 |
| 量詞序列 | `{min,max}` 的 max 小於 min；量詞後面再接量詞（懶惰 `?` 除外）、`^`／`$`／`\b`／`\B`／後看（與帶 u 的前看）後面接量詞、開頭或 `(`／`\|` 後面就是量詞：SyntaxError | 分詞時記住前一個記號能不能接量詞，不行就轉譯失敗（一般字串） |
| 不帶 u 的前看接量詞（Annex B） | 下限 0：那一輪空比對被丟掉，整個等於空（裡面的捕捉不留）；下限 ≥ 1：等於比一次 | fancy-regex 不收重複的斷言：下限 0 改寫成 `(?:\|(?!)前看)`（前看那一支永遠走不進去，回溯也帶不出捕捉；群組編號照留），下限 ≥ 1 去掉量詞 |
| 類別內的 `\c` | 字母；不帶 u 時數字與 `_` 也算（ClassControlLetter）；其餘是字面的反斜線 | 類別內另寫，不沿用類別外的規則 |
| 其餘 JS 收、Rust 收不下的語法 | — | 當一般字串，列已知差異 |
| 回溯爆量 | 網頁版不擋（D32） | fancy-regex `backtrack_limit`，超過算不命中（桌面版卡住的是後端執行緒） |

轉譯器是逐字元的 JS 正則分詞器，不是字串取代：已跳脫的字元（`\^`、`\.`）原樣保留；字元類別 `[…]` 內另一套規則——`.`、`^`（非開頭）、`$` 是字面，`\d`／`\w`／`\s` 展開成類別內可用的明列範圍（例：`[\d]` → `[0-9]`、`[\s]` → 明列空白集合），`\W`／`\S`／`\D` 在類別內不能寫成 `[^…]`，改成整個類別的「聯集＋差集」寫法（fancy-regex 支援類別集合運算 `[[…]--[…]]`，或展開成互補範圍）。

無 `u` 時 JS 以 UTF-16 碼元比對：`.` 與 `[^…]` 一次吃一個碼元，所以 `/^..$/` 比得中單一個星平面字元（emoji），Rust 以 code point 比對比不中。這點不轉譯，列已知差異並放案例。

### 2. 一次生成＝一個視角掃一次（包 5a）
對應 ST 群聊：每位說話者生成時各掃一次、條目依說話者過濾。
- GM 視角：條目池＝全部條目；訊息＝GM 側事件；設定看 P9。
- 角色 X 視角：條目池＝`Public`＋名單含 X 的 `Characters`；訊息＝角色側事件；預算用 X 的檔位上限；計時用 X 自己那份（P1）。條目池先依可見度過濾再掃，預算、群組、遞迴都只在 X 看得到的條目之間運作。
- 帶名字掃描的名字：玩家句用玩家名、角色台詞用 `speaker_name`；`GM` 旁白在世界書路的桌（GM 演那張卡）用原卡名（桌上已存原卡介面檔，取其卡名）；開場白事件在單角色桌用該角色名、世界書路用原卡名；其餘多角色桌的旁白與開場白用 `GM`，列已知差異，fixture 不收以卡名當鍵的多角色案例。網頁存檔匯入的桌照這條就和網頁版一致（單卡）。
- 掃描只算一次：
  - 實送：容量鎖（`commands/chat.rs:85-94`）與代落（`settle_previous_turn`）都過了之後才掃，掃描結果供組裝、導演指示點名格式條目、落地三者共用，不再重擲。
  - 量測（容量鎖、`scene_budget/measure.rs`）：掃描在組裝函式裡依「傳入的事件」做——固定部分是清空事件後重組（`measure.rs:226-229`），不能把本輪觸發的關鍵字條目帶進固定部分。量測一律「機率視為通過、群組抽選用固定種子 0」。量測**一律唯讀**：量測不持整桌寫入許可、會與回合並行（`commands/scene.rs:291-316`、`commands/chat.rs:85`、`measure.rs:237`），所以不寫計時、不還原 `pending`、不跑副作用、不更新 outlet；讀計時檔要在檔案鎖內讀；看到未結的 `pending` 時，只在記憶體裡以落地前的表計算，不寫回。
- `active_worldbook_entries` 刪掉；六個組裝點改吃掃描結果。

### 3. 放置、凍結快照與機密分流（包 5a，巨集部分包 5b）
照 ST 一次生成的語意做，不拿快取成本當不做的理由〔主線裁決 2026-10-10，第 1 輪審查〕。

**「穩定」的定義**（決定能不能進凍結 system／凍結快照）：觸發了、constant，並且同時滿足：
- 不擲機率（`useProbability` 關或 100）、sticky／cooldown／delay 都是 0 或沒設、不在群組、沒有 `delayUntilRecursion`、沒有裝飾、`triggers` 為空；
- 內文與標題只含「靜態巨集」：`{{user}}`、`{{newline}}`、`{{trim}}`、`{{noop}}`、註解 `{{// …}}`。含 `{{char}}`（依說話者而變）、`{{random}}`、`{{pick}}`、`{{roll}}`、時間日期類、變數類、`{{lastmessage}}` 系列、`{{input}}`、`{{model}}`、`{{maxprompt}}`、`{{outlet}}`、`{{original}}` 或任何其他巨集的，一律不穩定；
- 在角色共線上還要是 `Public`、且觸發來源不含私密內容（見下）。

不穩定的觸發條目放回合尾段。穩定條目某輪沒觸發（例如被預算擠掉）時，GM 線與一角一線的凍結 system 照既有補丁／重開機制（`lanes/mod.rs:602-711`）跟著變。

**Claude／Grok 角色共線的共用快照**（`lanes/mod.rs:199`，全角色共用一份）不得因角色而不同：
- 快照裡的條目＝靜態集合：啟用、constant、`Public`、符合上面穩定定義、而且位置是前／後／範例上下（0、1、5、6）的條目，不看任何一位角色的掃描結果；作者註記、依深度、outlet 位置的條目即使穩定也不收（它們照放置表在尾段或靠巨集代入）。每位角色掃描時這些條目照常參加、照常計入預算：溢出時靜態條目照樣保留在快照（不撤），但照網頁版設 `overflowed`，停掉排在後面的條目與遞迴（`world-info-scan.ts:431-452`）——不當成 `ignoreBudget`。這條改寫作用在所有角色共線的掃描上，已知差異照實寫在七。
- 快照裡的卡片公開設定一律以「中性脈絡」代換：只做靜態巨集與 `{{char}}`＝該卡自己的名字，不執行副作用；卡片公開設定含其他巨集時，整段移出快照，改在每位角色的回合尾以該角色視角代換（自己的卡完整執行、別人的卡中性），放在回合後會抹掉的段落（與機密段同一套抹除，標題仍寫公開設定）——共線回合後不抹的公開段會每輪疊一份。
- 測試：零溢出時連續切換角色，共用快照逐字不變。

**機密分流**：
- **判定方式**：先照網頁版用整段掃描文字算出觸發集合——觸發集合本身不因來源追蹤改變。分類不另外重跑不動點（反覆重比不是單調運算：主鍵 `a`、NOT_ANY 次要鍵 `x`、內文含 `x` 的條目，公開觸發後把自己的內文加進緩衝，下一輪重比就會失敗，`world-info-scan.ts:407`），而是在實際掃描的每一輪就地留證明：遞迴緩衝的每一段標記公開或私密（依加入它的條目的分類）；某條目在第 k 輪觸發時，用第 k 輪「緩衝去掉私密片段」的文字、以掃描時已代換好的鍵字串（不再執行巨集，`world-info-scan.ts:387`），照同一套主鍵與次要鍵邏輯再判一次——成立＝公開，不成立＝私密。公開片段＝聊天訊息、玩家卡人設、本視角角色的 `public_md`、第 k 輪之前已判公開的條目內文；私密片段＝`private_md` 與私密或限定條目的內文。constant 一律公開；sticky 生效中的條目沿用它被記下時的來源（桌面版計時檔多存一個 `confidential` 布林，不影響網頁版契約）。
- **沿遞迴傳遞**：私密觸發或限定條目的內文進遞迴緩衝後算私密片段，被它觸發的條目再往下傳；「Public →（私密遞迴）→ 另一條 Public → outlet」整條鏈都算私密。主鍵、次要鍵本身經巨集代換讀到私密來源時，該條目也算私密觸發。
- 私密觸發的 `Public` 條目改放機密段（現行規則本來就不讓私設關鍵字替角色觸發條目，`turns.rs:191-192`）。
- 網頁版沒有觸發來源可以對拍：補 Rust 單元測試，並驗證開關來源追蹤時觸發集合逐條相同。
- **代換結果也照機密分流**：代換時追蹤「讀到私密來源」——`{{description}}`、`{{personality}}`、`{{charprompt}}` 等讀到 `private_md` 的卡欄位巨集，或 `{{outlet}}` 的組成裡有私密觸發或限定條目——讀到的那段文字只能放機密段（不 hoist 的線回合後抹掉），不得進共用快照或公開段。
- **變數不追蹤私密**〔作者裁決 2026-10-10〕：照 ST，變數全桌共用、不分是誰寫的；`{{getvar}}` 讀出的值不帶私密來源。條目與私設本身仍照可見度與機密分流處理。卡作者刻意用變數搬運秘密時會洩漏給別的角色，列已知差異。
- GM 視角看得到一切，不分流。

**放置表**：

| ST 位置 | GM（續聊線與單發） | 角色 X（公開觸發的 `Public` 條目） | 角色 X（限定條目、私密觸發的條目） |
|---|---|---|---|
| 前（0）／範例上（5）〔作者裁決 2026-10-10〕 | 穩定：system 世界書段（`登場角色` 之前，現有位置）；其餘：尾段 | 穩定：共用快照「你知道的世界情報」段；其餘：尾段公開段 | 穩定：hoist 時進 system，否則機密段；其餘：機密段 |
| 後（1）／範例下（6）〔作者裁決 2026-10-10〕 | 穩定：system 新增一段「世界書（續）」放在玩家角色段之後；其餘：尾段 | 同上（快照該段排在後） | 同上 |
| 作者註記上／下（2、3）、依深度（4，含角色） | 尾段（P2）〔作者裁決 2026-10-10〕 | 尾段公開段 | 機密段 |
| outlet（7） | 不直接放；內容由 `{{outlet::名稱}}` 代入卡寫的位置（P5）〔作者裁決 2026-10-10〕 | 同左 | 同左 |

- 範例上／下兩種位置併入角色設定的前／後兩組〔作者裁決 2026-10-10〕：桌面版沒有獨立的範例對話段（卡的範例在 `public_md` 裡）。
- **尾段順序**：前、後、範例上／下照網頁版的組內順序；接著把作者註記（深度 4、system，鍵 `2_floating_prompt`）與依深度條目（鍵 `customDepthWI_<深度>_<角色>`）照 `injections.ts:33-53` 的攤平結果排成一串：深的在前；同深度的閱讀順序是 assistant → user → system（該處先照 `ROLES`＝system、user、assistant 組好，插進新到舊的列表後整體 reverse，對照 `injections.ts:8` 的常數）；同深度、同角色的各條內容先依鍵名排序、各自 trim 後以換行串起，**整段只代換一次**（`:40-45`，setvar 的次數與順序才與網頁版一致）。全部放在尾段同一則文字裡，不拆訊息、不標角色。理由：P2 裁決的是「放在回合尾段」，深度與角色不再決定插入位置；照 ST 的攤平順序排，模型讀到的相對先後與 ST 插進歷史時相同（越深的越早出現）。
- 人物條目：照常參加掃描（佔預算、可被遞迴）；constant 人物條目觸發後照舊只進名冊行。
- **不抹尾段的線**（Agy 一角一線、單人在場的 Claude：`erase=false`，`commands/chat.rs:148`、`lanes/mod.rs:184-186`）：尾段會留在 session 歷史裡，每輪的世界書段會一份份疊上去。這兩種線的 system 本來就只屬於一位角色，所以把本輪所有觸發的世界書內容（含不穩定與限定條目）都放進 hoist 那塊 system、尾段不放世界書。hoist 的世界書內容一變就**強制重開**（Agy 本來就重開）：單人 Claude 線在快取時效內原本走補丁，補丁接在尾段前面、回合後不抹（`lanes/mod.rs:705-711`、`:1057-1059`），舊世界書仍留在歷史裡，所以這種變動不走補丁。測試驗連續三輪換世界書後 session 裡只有本輪那份。

**格式條目判定**（`gm_prompt_full_entries`）：維持「本輪實際送入全文」——取放置結果裡全文確實進了提示的條目（outlet 條目只有在 `{{outlet}}` 真的被代入時才算；只進名冊的人物條目不算），再併入現行的人物登場全文（`context.rs:62-68`），不直接拿觸發清單。

### 4. 觸發狀態的存放與落地（包 4）
- **分視角**（P1）〔作者裁決 2026-10-10〕：GM 一份、每位角色各一份，形狀同網頁存檔契約的 `timed`，鍵是 uid 字串；start／end 用有號整數（i64）。
- **分幕存放**：`worlds/<id>/world-info/<幕號>.json` = `{ "version": 1, "perspectives": { "gm": {…}, "char:<角色 id>": {…} }, "pending": … }`，讀寫都在檔案鎖內，寫入走 `commit_world_write_atomic`。檔案讀不了或解析失敗就回錯，不當成空表覆寫（照 `card_vars` 的做法）。
- **則數**＝本幕事件扣掉 `System` 類與 `SceneSummary` 摘要後的數量（與掃描清單同一個排除規則，在 `render_for_prompt` 之前就排除，各視角算出來一樣）。
- **換幕**（P4）〔作者裁決 2026-10-10〕：在 `begin_next_scene_tx` 裡，排在開頭的 `refuse_during_turn`（`data/scene/lifecycle.rs:135`，會先把沒落檔的 GM 半截代落進逐字稿）之後、寫 `current_scene` 之前：先對舊幕的 `pending` 跑「結算 pending」（見下），再把各視角未到期的計時平移寫成新幕的檔——start、end 各減去舊幕則數（新幕從摘要之後算起，摘要不計），一律覆寫殘留（沒有計時也寫空檔）。
- **分岔**：在 `fork_scene_tx` 裡、寫 `current_scene` 之前，先對來源幕的 `pending` 跑「結算 pending」，再把結算後的計時檔複製成新幕的檔（不帶 `pending`），一律覆寫殘留。
- **退幕**：父幕的檔原封不動；子幕的計時檔與逐字稿一起刪（`lifecycle.rs:225-233`，同樣盡力而為）。
- **落地時機**：掃描結果（與巨集副作用，三之 8）在「真的把請求交給傳輸層」那一刻才寫，仍在整桌寫入許可內。試掃、量測、送出前失敗的不寫。
- **失敗回滾**：落地時同時記 `pending = { turn_key, 視角, 階段, 落地前該視角的表, 變數日誌 }`（變數日誌見三之 8）。**成功＝這個回合的正文確實落檔**；中止後有半截正文落檔也算成功（與 ST 一致：停止生成時半截訊息留在聊天裡），沒有正文落檔一律算失敗。
  - GM 回合：GM 正文本來就經 `append_transcript` 帶 turn_id／turn_part 落檔（`commands/scene.rs:11-28`、`data/scene/transcript.rs:180-207`），在這次 append 裡清 `pending`。成功的 `TurnGuard.commit` 只把回合狀態改成 AwaitingAppend，之後 `Drop` 照樣呼叫 `finish_turn(None)`（`commands/chat.rs:636-643`、`data/message_vars/write.rs:402-414`），所以 `finish_turn(None)` 不能一律還原：回合狀態是 AwaitingAppend、或還有非空半截等著落檔時，`pending` 保留，交給落檔或下一次結算處理；只有確定失敗——Generating→Aborted 且沒有任何正文要落檔——才當場還原。
  - 角色回合：回覆是前端在 `chat_with_character` 回傳、許可釋放之後另外 append（`commands/chat.rs:166-169`、`src/features/play/useChatController.ts:584`），事件沒有 turn_id、`action_id` 多個回合共用、`message_vars/turn.rs` 的回合狀態重啟就消失。角色回覆另開一條明確的落檔路：`append_transcript` 加 `character_turn: Option<String>`（填這次呼叫的 turn_id），內部仍傳 `turn=None` 走一般事件路——照一般事件先代落（`write.rs:349-354`），保住「GM 正文排在角色回覆之前」；不進 GM 的冪等路徑（帶 `turn_key` 會進 `prepare_turn_append` 比對 GM 回合紀錄而回 turn-mismatch，`write.rs:519-528`）。這條只把 turn_id 寫進事件的 `turn_key` 並清對應的 `pending`。前端與 `src/shared/contracts/backend-contracts.ts:106` 的參數一起改（包 4）。
  - 正文追加與清 `pending` 分在兩個檔（逐字稿與計時檔），`transcript.rs:204` 也已是兩次寫檔，做不成同一個原子提交，所以 `pending` 記著回合鍵，靠下面的「結算 pending」恢復。看逐字稿裡有沒有帶這個回合鍵的事件，是唯一允許看逐字稿判斷成敗的地方，且只認回合鍵，不從內容推斷。
  - **結算 pending**（下一次持許可的實送掃描之前、換幕、分岔都跑同一支；量測不跑）：
    - 階段＝寫入中（還沒送出就中斷）：計時還原；變數意圖照三之 8 的四種情形逐筆處理，不得整筆丟掉日誌；處理完（含回報）才清 `pending`。
    - 階段＝已送出：逐字稿裡有這個回合鍵＝成功，清 `pending`；沒有＝失敗，還原計時、清 `pending`（變數副作用保留，見三之 8）。換幕時 GM 半截已由 `refuse_during_turn` 代落，所以結算排在它之後。
    - 其餘失敗（呼叫回錯、確定失敗的中止、前端沒落檔）：持許可的路徑能當場判定確定失敗就當場還原；其他都留給結算。
  - 時序約束：前端在發下一次聊天呼叫之前就要落好上一則回覆（現行流程如此）。`pending` 已還原之後才到的落檔，回合鍵對不到就不動計時。
  - 效果：別的視角先追加了事件、則數已經大於 start，失敗的效果也會撤掉（只靠「則數 ≤ start」撤不到，`world-info-scan.ts:211`）。
- **退回**：刪事件（`pop_transcript`）後重送靠「則數 ≤ start 且未受保護就撤」，與 ST 相同。編輯事件文字不動計時。
- **同一 uid 的兩個穩定 ID**（網頁存檔裡兩條經去重對到同一 uid）：同一類計時取 end 較大的那筆，end 相同時 `protected` 為真者優先。
- **刪條目清計時**：掛在 `write_worldbook_value`（`data/worldbook.rs:65`）——比對寫入前後的 uid 集合，消失的 uid 在寫完書之後從各幕計時檔與 `pending` 的落地前表裡一起刪掉，一處涵蓋所有刪除路徑（單刪、清重複、撤銷匯入、重構套用與撤銷）。寫完書、清計時之前崩潰的話，讀計時檔時再拿書裡現有的 uid 對一次，已經消失的照樣清（量測讀的時候只在記憶體裡對、不寫回）。uid 重用的崩潰情境——刪掉最大 uid、書已寫入、清計時前崩潰，重啟後先新增一條拿到同一個 uid，再讀計時時舊計時就被當成有效——靠另一條規則擋：新增或匯入條目要發布新書之前，先拿「寫入前的 uid 集合」對各幕計時檔與 `pending` 的前像清一次，消失的 uid 先清掉才發布。uid 以「最大值＋1」配發（`worldbook.rs:304`），不清的話刪掉最大 uid 再新增，新條目會繼承舊計時。
- **原子替換**：計時檔（含 `pending` 與變數日誌）一律用現成的 `commit_world_write_atomic`（`data/world_file.rs:789`，內部呼叫 `write_atomic`，`:703`，暫存檔＋rename）。不改 `commit_world_write`（`:785`，直接覆寫，全域共用，改了會波及逐字稿與狀態檔）。測試直接驗這兩支在寫到一半失敗時原檔完整。

### 5. 條目形狀、順序與去重（包 2）
- **轉換**：`normalize_imported_entry`（`book_import.rs:233`）的 `character_book` 分支改成完整的 `convertCharacterBook`，欄位清單與預設值以網頁版 `fromCharacterBook` 為準（`world-info-book.ts:99-140`）。V2 缺 `selective` 時寫明 `false`（物件形預設是 `true`，`:149`，不能因轉換翻轉）；`enabled` 缺欄或 null 算啟用，其餘照 JS 真假值（1、"yes" 啟用，0、"" 停用），不再有「壞條目」與 `invalid` 計數，嚴格模式也不因此報錯〔作者裁決 2026-10-10，網頁版同步改〕；`order` 缺或不是有限數字補 100〔作者裁決 2026-10-10〕。`extensions` 其餘鍵（含 `table_tavern`）原樣留著。
- **順序與 uid**：陣列形照網頁版 `stOrder`（`world-info-book.ts:189-197`）決定順序並依此配新 uid，重複 `id` 照 ST 只留後一條、位置在前一條；物件形照 `Object.keys` 順序（整數鍵遞增、其餘依原檔出現順序）——`serde_json` 沒開 `preserve_order` 會把鍵排序，讀卡內書時改用保留順序的解析（只用在這一處，不全域開 `preserve_order`）。匯出的 `v2_entry` 補上 `id`（＝uid）。
- **從原始文字保序**：三個入口都要從原始位元組／文字抽出 `entries`，保住原順序直到落檔——角色卡路（`import/card.rs:252` 已先解析成鍵排序過的 `Value`、`:373` 又重新序列化）、世界書路（`import/files.rs:72` → `worldbook_json`）、網頁存檔（`import/web_save/mod.rs:192` → `worldbook_json`）。做法：在卡 JSON 原文上用保序的解析（只抽 `character_book.entries`／書的 `entries`，產出有序的 `(鍵, 原文值)` 清單）交給 `import_worldbook_as`，不經過已排序的 `Value`。
- **去重指紋**（`book_import.rs:445`）：改成以 resolver 讀出的 `WiEntry` 計算，納入所有影響觸發的欄位——主鍵（含順序）、次要鍵、`selective`、`selectiveLogic`、constant、`position`、`depth`、`role`、機率兩欄、群組四欄、`scanDepth`、大小寫／全字／群組計分、sticky／cooldown／delay、三個遞迴欄位、`triggers`、`ignoreBudget`、`outletName`、`match*`、裝飾，加上標題、內文、`source_disable`；仍不含 `order`、停用（玩家會改，前案裁決）。先正規化再算：`delayUntilRecursion` 的 `false`／`0` 都當 0、`true` 當 1，計時的 `null`／`0` 都當 0，V2 與物件形的同一條目才算出同一指紋。重構用的身分指紋（`identity_text`）不動。
- **再匯入命中**：指紋相同＝觸發行為相同，所以只併可見度與來源卡（`merge_entry_into` 現行），`order`、停用保留桌上的值。
- **匯出**：`v2_entry` 反向照 ST `convertWorldInfoToCharacterBook` 把物件形欄位收回 `extensions` 的 snake_case，`position` 規則沿用現有。
- 包 2 施工時的實作決定〔模型判斷·未裁決〕：去重指紋標題、內文、鍵一律用原文（不 trim），缺 `key` 與空陣列同指紋，沒有次要鍵時不算 `selective`／`selectiveLogic`（不影響觸發，V2 缺欄 false、物件形缺欄 true）；`id` 重複被蓋掉的條目在 `placed` 映到留下那條的 uid，`worldbook_entries` 以不重複的 uid 計；物件形匯出補讀取端預設（缺 `selective` 寫 true，`insertion_order` 缺或非有限數字寫 100）；網頁存檔的 PNG 只在 `Value` 相等且物件形條目鍵順序一致時才採用；編輯器的 order 視圖與讀取端同一規則（缺或非數字 100、小數四捨五入顯示），沒改 order 的存檔不動原值。
- 舊桌裡已經是混合形的條目不轉換（舊桌不相容，限發佈前）〔模型判斷·未裁決〕：讀取端照物件形預設補，V2 `extensions` 裡的值讀不到。

### 6. 巨集引擎（包 3，P6）〔作者裁決 2026-10-10〕
- Rust 照網頁版 `web/src/features/sillytavern/` 的 `macro-parser.ts`、`macro-engine.ts`、`macro-library.ts`、`macro-time.ts`、`substitute.ts`、`variables.ts`、`seedrandom.ts` 逐支移植成 `src-tauri/src/st_macros/`。求值分兩種模式：完整（副作用記成操作序列）、中性（只輸出、不記副作用）。代換同時回報「讀到私密來源」（三之 3）。
- 巨集脈絡的桌面版對應：`{{user}}`＝玩家名；`{{char}}`＝本視角角色名（GM 視角：世界書路用原卡名，其餘照現行）；卡欄位巨集照 P7 的對應；`{{lastmessage}}` 系列與歷史＝本視角非系統、非摘要事件，舊到新，保留時間與是否玩家（同網頁版 `st-text.ts:39`）；`{{model}}`＝該路徑模型字串；`{{maxprompt}}`＝該路徑上限；`{{outlet::名稱}}` 見三之 7；`{{pick}}` 的種子照網頁版 `hash(hash(chatId), 內容雜湊, 全域位移)`（`macro-library.ts:212`），只把 chatId 換成桌 id、其餘兩段照算（網頁版 chatId 是 `web-<存檔 id>`，匯入的桌結果會不同，列已知差異）。
- 變數巨集讀寫桌面版既有的卡片變數層（chat、global，`data/card_vars.rs`），落地見三之 8。
- 代換順序照網頁版 `prompt.ts` 的先後；桌面版沒有的段落（範例對話、jailbreak）跳過。
- 巨集案例搬進 `src/shared/contracts/st-macros/`（`st-macros.json` 與 `st-macro-cases.json`），一起改引用處：`src-tauri/src/refactor_ai/result_parse.rs:119`、`src/features/refactor/refactor-shell.ts:10`、`web/src/features/sillytavern/macro-engine.test.ts:2`。

### 7. 巨集呼叫點與副作用時機（包 5b）
- **只在本視角執行**：完整求值（含副作用）只用在本視角擁有的文字——自己那張卡（角色視角）、world.md 與世界書路原卡的內容（GM 視角）、本視角條目池裡的條目。其他卡的欄位（GM 的全卡段、角色共線的別人公開設定）用中性模式。
- **中性模式**：在變數層的隔離副本上照樣執行變數操作（所以同一段裡 `{{setvar}}` 後的 `{{getvar}}` 輸出與完整模式相同），段落結束就整份丟掉，不進操作序列、不落地（對應 `variables.ts:111` 的寫入點）。
- **信任邊界**：狀態值來自模型輸出，`mechanism/triggers.rs:42`、`transport/state_view.rs:179`、`:432` 維持只換 `{{user}}`／`{{char}}`，不接引擎（接上等於讓模型輸出能觸發 setvar）。
- **逐點時機**：

| 呼叫點 | 性質 | 模式與副作用 |
|---|---|---|
| `transport/context.rs:120-173`、`turns.rs:103-229`、`state_view.rs:31-32`（組裝） | 生成組裝 | 本視角文字完整、其他卡中性；副作用在實送落地 |
| `mechanism/triggers.rs:42`、`state_view.rs:179`、`:432` | 模型輸出 | 維持舊代換 |
| `commands/world.rs:184`（開場白清單顯示） | 顯示 | 中性，不寫變數 |
| 開場白落檔（`commands/scene.rs:84` `post_opening`） | 送出後落地 | 前端改傳開場白序號，後端以原文完整求值、落檔與副作用同一次提交；玩家貼的是翻譯版時，正文用翻譯版、副作用照原文 |
| 登場事件（`transport/arrivals.rs:47`、`:86`、`:104`） | 回覆後落檔進逐字稿 | 中性（桌面版特有機制，不在 ST 生成流程裡），在回合提交時求值一次寫死 |

- **前輪 outlet**（網頁版 `prompt.ts:81`）：掃描前就代換的文字（卡欄位、主鍵、次要鍵）讀到的是「上一輪的 outlet」，掃完才換成本輪值。桌面版每個視角各存一份上一輪 outlet（記憶體，以桌 id＋視角為鍵）；試掃與量測讀它但不更新；實送落地時才換成本輪值；app 重啟、換幕、退幕、分岔都清空（ST 換聊天也清空，outlet 不持久化）。

### 8. 落地：計時、變數副作用與衝突（包 4 計時、包 5b 變數）
- **落地順序**：先寫計時檔與 `pending`（階段＝寫入中）→ 每一層（chat，再 global）照「先記意圖、再寫層、最後記結果」→ 把 `pending` 階段改成已送出 → 交給傳輸層。
  - 意圖：層、預期 rev（寫入前）、寫入前內容、操作序列；寫進 `pending` 並落檔後才動該層。
  - 結果：寫入成功後把寫入後 rev 補進該筆意圖並落檔。
- 任何一步失敗就不送出並撤回：計時檔直接還原（持許可，沒有別人會寫）；已寫成的變數層各自以「寫入後 rev」做 compare-and-set 還原成寫入前內容——global 也一樣（`card_vars.rs:385`）；還原時發現有人在之後寫過，就保留別人的寫入、不還原，並連同原錯一起明確回報玩家（比照 `web_save_cleanup_incomplete`）。
- **崩潰恢復**：下一次持許可的實送掃描看到階段＝寫入中的 `pending`：計時還原；逐筆處理變數意圖——
  - 有結果、目前 rev 等於寫入後 rev：compare-and-set 還原成寫入前內容。
  - 有結果、目前 rev 不同：有人在之後寫過，保留並回報。
  - 沒有結果、目前 rev 等於預期 rev：沒寫成，不動。
  - 沒有結果、目前 rev 不同：分不出是不是自己寫的，保留並回報。
  - 回報：要回報的結果先寫進該桌的待回報檔 `worlds/<id>/world-info/notices.json`（`commit_world_write_atomic`），落地之後才清掉 `pending`——換幕、分岔沒有聊天回傳可以附，所以一律走這個檔。聊天呼叫、換幕、分岔的回傳與開桌時都讀這個檔，前端提示玩家一次後呼叫確認指令刪掉該則。不保留日誌，下一輪不會重複撤回。
  - 階段＝已送出的 `pending` 照三之 4 的成功／失敗規則處理（失敗只還原計時）。
- **變數衝突**：`write_layer` 會回 `Ok(Stale)`／`Ok(Rejected)`（`card_vars.rs:385-436`），而卡片介面寫變數層不受整桌寫入許可擋，global 層又是跨桌共用的。所以：求值時記下操作序列（setvar、addvar、incvar…，依發生順序）；落地時讀最新的表與 rev，在最新表上重放操作序列，以 rev compare-and-set 寫入；`Stale` 就重讀重放，最多 3 次；`Rejected` 或重試用完＝落地失敗。每次重試之前，先把該筆意圖的預期 rev 與寫入前內容更新成這次讀到的值並落檔；撤回一律用最後一次的前像，否則會抹掉介面在重試之前寫進去的值。提示裡 `{{getvar}}` 顯示的是求值當下的快照值，重放只決定最後寫進去的值。
- 送出之後才失敗：計時照三之 4 回滾；變數副作用保留（ST 組提示時就執行了巨集，生成失敗也不撤）。

## 四、測試

### 1. 與網頁版對拍
- 契約目錄 `src/shared/contracts/world-info/`：
  - `scan-cases.json`：每案＝條目（物件形原始 JSON＋穩定 ID）、新到舊訊息、設定、計時表、預算上限、生成類型、亂數序列、全域掃描欄位 → 預期的觸發 ID 順序、各位置分組內容、新計時表、outlet、**代換呼叫紀錄**（代換函式是會改寫文字並記錄呼叫的非恆等函式）與**亂數消耗次數**。鎖住「主鍵先代換再 trim」「內文在預算檢查前代換」的順序。計數規則固定為「Unicode code point 數」，含一個星平面字元（例：emoji）的案例。
  - `regex-cases.json`：三之 1 轉譯表每一列各至少一案（含 `y` 不中、`\d` 不吃全形、`\s` 吃 U+3000、`.` 不吃 U+2028、多餘跳脫、`u` 下多餘跳脫變一般字串），另含字元類別內的 `[\d]`、`[.]`、`[\W]`、`[\s\d]`、已跳脫的 `\^`，以及無 `u` 的 `/^..$/` 對單一 emoji（標註已知差異）。
  - `sort-cases.json`：`sortByOrder` 的穩定排序結果（order 都是有限數字，含大量同值、長度 >64）。
  - `entry-cases.json`：V2 陣列形與物件形原始條目 → 預期 `WiEntry`；重複 `id`、`stOrder`、物件形非整數鍵順序、缺 `selective`；角色卡路匯入落檔後再讀出同一個 `WiEntry`。
  - `integration-cases.json`（跨巨集與掃描兩個引擎）：卡欄位、聊天、變數、上一輪 outlet → 掃描時的代換結果、組裝時再代換的結果、本輪 outlet、副作用操作序列（依順序）、每段送出文字。比對的是這些中間產物，不比整份提示（兩邊組法本來就不同）。
  - `web-save-next-turn.json`：端對端預期值（四之 4），與 e2e 會重新產生的 `web-export-world-info.json` 分開放。
  - `world-info.md`：欄位說明與「改 fixture＝兩邊同一筆 commit」。
- 產生預期值的腳本 `web/scripts/gen-world-info-fixtures.mjs`（輸入在 `web/scripts/world-info-fixture-cases.ts`）用網頁版實作跑出、人工核對後提交；兩邊的測試（網頁版 `web/src/features/sillytavern/world-info-parity.test.ts`、Rust `src-tauri/src/world_info/parity_tests.rs`）只比對、不改寫 fixture。
- 巨集：`st-macro-cases.json` 兩邊全過；另由網頁版補跑出時間（固定時鐘）、`{{pick}}`（固定 chatId）、`{{lastmessage}}` 系列、卡欄位、歷史的案例。
- 案例至少涵蓋第二節每一列；另含中日韓全字比對、D31、sticky 到期接冷卻、退回、delay、0 值計時、預算溢出停遞迴、`ignoreBudget`、群組計分與權重、`@@` 裝飾、order 同值的載入順序。

### 2. 桌面版組裝
- 各注入路放置：GM 續聊線、GM 單發、角色共線（Claude／Grok）、單人在場 Claude、Agy、API 單卡與多卡。
- 共用快照：零溢出時連續切換角色逐字不變；含 `{{char}}`、`{{random}}`、`triggers` 的 constant 條目不進快照；卡片公開設定含動態巨集時移出快照。
- 機密分流：只因 `private_md` 或限定條目遞迴而觸發的 `Public` 條目進機密段；`{{description}}` 讀到私設、`{{outlet}}` 含限定條目時，代換結果只在機密段且回合後抹掉。
- 不抹尾段的線：世界書內容在 hoist system、尾段沒有；單人 Claude 線世界書一變就重開不走補丁，連續三輪 session 裡只有本輪那份。
- 共用快照：作者註記、依深度、outlet 位置的穩定條目不進快照；靜態條目計入預算、溢出時照樣停掉後面的條目與遞迴；卡片公開設定移到尾段時放在會抹掉的段落。
- 尾段順序：同深度 assistant → user → system；同深度同角色整段只代換一次（`{{setvar}}` 次數與網頁版一致）。
- 機密分流單元測試（Rust）：開關來源追蹤觸發集合逐條不變；分類正確——只命中 `public_md` 的條目是公開、只命中 `private_md` 的是私密、私密遞迴再觸發的 `Public` 條目是私密、私密沿遞迴傳到 outlet；重比不執行巨集（副作用計數不變）；反例「主鍵 `a`、NOT_ANY 次要鍵 `x`、內文含 `x`」分類穩定為公開、不震盪。
- 中性模式：同段 setvar→getvar 輸出與完整模式相同，變數層不變。
- 可見度：X 的掃描看不到 GM 條目與別人的限定條目。
- 掃描只算一次：導演指示點名與實送一致；格式條目只在全文確實送出時點名（outlet 未被引用不算、名冊不算、登場全文算）。
- 量測：固定部分不含本輪關鍵字條目；量測不寫任何檔、不還原 `pending`、不跑副作用、不更新 outlet；機率視為通過；與回合並行時讀到一致的計時檔。
- 換幕摘要在渲染前排除，不佔掃描深度、不計入則數。
- 匯入保序：三個入口（角色卡路、世界書路、網頁存檔）的物件形非整數鍵與陣列形 `id` 順序都保到落檔。
- 巨集信任邊界：狀態值裡的 `{{setvar}}` 不執行。開場白清單顯示不寫變數，落檔才寫。
- `lanes/scaffold_baseline/*.txt` 隨位置分段更新。

### 3. 計時與副作用落地
- 實送才寫；量測、送出前失敗不寫。
- 成功依據：GM 正文經 `append_transcript` 落檔才清 `pending`，`finish_turn(None)` 與成功 commit 後的 Drop 不清；角色回覆走 `character_turn` 路落檔才清，且不觸發 turn-mismatch、不改變代落；中止但有半截正文落檔算成功；前端沒落檔、呼叫回錯算失敗。
- 正文落檔後、清 `pending` 前崩潰：恢復時看到逐字稿裡有該回合鍵，判成功、不回滾。
- 原子替換：直接驗 `commit_world_write_atomic`／`write_atomic` 寫到一半失敗時原檔與恢復資料完整。
- 變數意圖日誌：四種恢復情形各一案；回報一次後 `pending` 清掉，下一輪不重複撤回。
- 刪條目清計時：單刪、清重複、撤銷匯入、重構撤銷四條路都清到各幕計時與 `pending` 前像。
- 同一視角失敗後重送：失敗那次的效果已回滾。跨視角：GM 落地後失敗、角色先追加事件，GM 下一次掃描時失敗的效果已撤掉。崩潰留下的 `pending`（寫入中、已送出兩種階段）由下一次持許可的實送掃描還原。
- 部分寫入：計時、chat、global 任一步失敗都完整撤回（global 也以 rev 還原）；寫入中崩潰照日誌恢復。
- 換幕與分岔先結算 `pending`：舊幕逐字稿有回合鍵（含 `refuse_during_turn` 剛代落的 GM 半截）就清掉後平移／複製，沒有才還原；階段＝寫入中時變數意圖照四種情形處理。分岔的新幕檔不帶 `pending`。
- `finish_turn(None)`：AwaitingAppend 或有非空半截待落檔時 `pending` 保留；Generating→Aborted 且沒有正文才還原。
- 角色落檔路先代落：GM 正文排在角色回覆之前；不觸發 turn-mismatch。
- 刪條目清計時：寫完書、清計時前崩潰，下次讀計時檔時照現有 uid 清掉（量測只在記憶體裡清）；刪掉最大 uid、清計時前崩潰、重啟後先新增一條拿到同一 uid，新條目不繼承舊計時。
- 結算回報：換幕、分岔時「寫入中」的回報落進 `notices.json` 之後才清 `pending`；回報落檔失敗時 `pending` 保留。
- CAS 重試競態：介面在第一次與第二次嘗試之間寫入，撤回時保住介面的值。
- `pop_transcript` 後重送撤掉未受保護的計時；受保護的冷卻留著。
- 分視角：GM 觸發帶冷卻的條目，不擋 X 自己的掃描。
- 換幕平移（以摘要之後為起點、start 可為負）、退幕刪子幕檔、分岔複製、殘留覆寫；計時檔壞掉時回錯不覆寫。
- 變數：卡片介面在求值與落地之間寫了同一層 → 重放後兩邊的寫入都在；重試用完 → 不送出、明確回報、chat 層已還原；還原時被別人寫過 → 保留別人的值並回報。

### 4. 網頁存檔端對端
- 匯入 `web-export-world-info.json` 後，角色 X 下一輪的觸發結果等於 `web-save-next-turn.json`（網頁版在同一份存檔下一輪的結果）。
- 對不到 uid 的計時丟掉、世界書路給 GM、`last_message_id: null` 不平移、兩個穩定 ID 對到同一 uid 時照合併規則。

### 5. 驗收
- `npm run verify` 全綠、`cargo test` 全過、網頁版 vitest 全過。
- 測試通道（`npm run harness:build`＋`scripts/harness.mjs`）用 `TestCards/` 裡帶 sticky／機率／次要鍵的卡各跑一桌：匯入、送三輪，檢查世界書計時檔與送出的提示（假模型即可）。

## 五、已拍板（P1–P9）〔作者裁決 2026-10-10〕

- **P1 計時分視角**：GM 一份、每個角色各一份。角色卡條目的 sticky／cooldown 只看自己的發言節奏；網頁存檔的狀態交給演這張卡的人。
- **P2 作者註記與依深度插入一律放回合尾段**：所有路徑一致、快取前綴不受影響；深度與角色不再決定插入位置（尾段內的先後見三之 3）。
- **P3 掃描深度照 ST 預設 2 則**：與網頁版、ST 一致；多人回合裡較早的發言比較快掃不到，要持續生效靠 sticky 或條目自己的掃描深度。
- **P4 換幕時未到期的計時平移帶進新幕**：sticky 與冷卻跨幕延續。
- **P5 補 `{{outlet::名稱}}` 巨集**：照 ST。
- **P6 巨集引擎併進本案**：桌面版補齊到網頁版那套 ST 巨集行為，與網頁版用 fixture 對拍。
- **P7 比對角色描述類旗標對卡的 `public_md`＋`private_md`**：細節見第二節對照表；描述與個性兩個旗標比對同一段文字，範圍比 ST 寬。
- **P8 `characterFilter` 不做，以可見度代替**：與網頁版一致。
- **P9 MVU 設定照視角判斷**：角色視角看該卡、GM 視角看桌上任一張卡或世界書路的卡；MVU 卡與一般卡同桌時 GM 線用 MVU 設定。

## 六、分包

| 包 | 內容 | 建議模型 | 依賴 |
|---|---|---|---|
| 1 掃描核心＋對拍 fixture | 三之 1：`src-tauri/src/world_info/` 逐行移植網頁版（含正則分詞轉譯、穩定排序、觸發來源與其單元測試）；`fancy-regex` 新增、`getrandom` 改正式依賴；`scan-cases`、`regex-cases`、`entry-cases`（讀取部分）與產生腳本；兩邊 parity 測試。不接組裝點 | Opus（正則與排序語意細節多，錯了會產出看似合理的結果） | — |
| 2 條目形狀、順序與去重 | 三之 5：匯入完整轉換、`stOrder` 配 uid 與重複處理、三個入口從原文保序、去重指紋、匯出反向與補 `id`；`entry-cases` 落檔案例；既有匯入／匯出／去重測試跟著改 | Sonnet（規格已定、照網頁版欄位對照） | 1 |
| 3 巨集引擎 | 三之 6：`src-tauri/src/st_macros/` 移植、完整與中性兩種模式、操作序列、私密來源回報；巨集案例搬家與補案例、改三處引用；Rust parity 測試。不接組裝點 | Opus（解析與求值順序，錯了會產出看似合理的結果） | — |
| 4 計時存放與網頁存檔消費 | 三之 4：`world-info/<幕>.json` 讀寫（檔案鎖）、則數規則、落地與 `pending` 回滾 API（含階段與變數日誌欄位）、GM 正文 append 清 `pending`、角色回覆 `character_turn` 落檔路（先代落；含前端與 `backend-contracts.ts:106`）、`finish_turn(None)` 依回合狀態保留或還原、結算 pending（實送前、換幕、分岔）、計時檔走現成原子寫入、換幕平移／退幕刪檔／分岔複製（進各自的 tx）、刪條目清計時（含發布新書前以寫入前 uid 集合預清）、待回報檔 `notices.json`、網頁存檔匯入轉成第 0 幕計時檔、`web-save.md` 與 `web-version.md` 敘述改寫。只交出資料層 API 與單元測試 | Opus（資料安全與落地時機） | 1 |
| 5a 掃描接線 | 三之 2、三之 3（不含巨集）：視角掃描、單次共用、六個組裝點、穩定定義、共用快照、機密分流、不抹尾段的線、尾段順序、格式條目判定、量測、預算接上限、P9、名字規則；接包 4 的落地與回滾；巨集先沿用舊代換；端對端 `web-save-next-turn` 對拍；scaffold baseline | Opus（快取前綴、可見度與 hoist 的交互） | 1、2、4 |
| 5b 巨集接線與副作用落地 | 三之 7、三之 8：所有呼叫點換成新引擎（信任邊界三處維持）、中性模式隔離副本、穩定定義補上巨集條件、代換結果機密分流、前輪 outlet、開場白落檔改傳序號、變數操作序列落地與衝突重放、部分失敗與崩潰恢復；`integration-cases` 對拍 | Opus（資料安全與機密分流） | 3、5a |

逐包送審、逐包合併到本分支，結案時整理成一包一筆進 main。

## 七、已知差異（施工後仍與網頁版／ST 不同處）
- 刻意不模仿 ST〔作者裁決 2026-10-10〕：`order` 缺或不是數字補 100（ST 是 NaN，排序結果取決於演算法）；V2 條目缺 `enabled` 或為 null 算啟用（ST 算停用）；網頁版同步改成這兩條。
- 去重會把只差 `order` 或停用狀態的兩條併成一條（指紋不含這兩者，前案裁決）；ST 不去重，兩條都會在〔模型判斷·未裁決〕。
- 正則：JS 收、`fancy-regex` 收不下的語法在桌面版當一般字串（例：落單的代理 `/\uD83D/`）；回溯超限算不命中，同一次掃描裡撞過上限的鍵之後直接算不中。
- 正則的 `i` 旗標：類別外的 `\w`、`\d`、`\s`、`\b` 包了 `(?-i:…)` 與 JS 相同；類別內做不到——`/[\w]/i` 在桌面版會中 ſ、`/[a-z]/i` 會中 K（U+212A），JS 不中。
- 量詞界限超過 2^31：V8 把界限飽和後再比上下限（`/(?=a){2147483648,2147483647}a/` JS 解析成功），桌面版照實際數值比、判轉譯失敗當一般字串；實際卡不會這樣寫，不追。〔模型判斷·未裁決〕
- 重複群組的捕捉：JS 每一輪開始會清掉群組裡的捕捉，fancy-regex 不清（例：`/^(a\1)+$/` 比 "aa"、`/^(?:(a)|b)+\1$/` 比 "ab"，JS 中、桌面版不中），難以轉譯。
- Unicode 屬性名：Rust 收簡寫（例：`\p{Han}`），JS 要寫 `\p{Script=Han}`，JS 當一般字串的鍵桌面版會當正則。
- token 計數：網頁版用模型 tokenizer，桌面版用估算，預算邊界的條目可能一邊進一邊不進（fixture 用固定計數規則所以對得上）。
- Claude／Grok 角色共線：快照裡的靜態條目在所有角色掃描中都不會被預算撤掉（溢出時照樣停掉後面的條目與遞迴）；ST 在溢出時可能撤掉其中排序較後的條目。
- 多角色桌的旁白與開場白掃描時名字是 `GM`，以卡名當鍵的條目觸發不到。
- `{{pick}}` 的種子用桌 id，網頁存檔匯入後同一段文字選出的項目會與網頁版不同。
- ST 以內容雜湊認條目、改內文會重置計時；桌面版以 uid 認條目，改內文不重置。
- 範例上／下併入角色設定的前／後〔作者裁決 2026-10-10〕；作者註記與依深度一律在尾段（P2）。
- 無 `u` 的正則以 UTF-16 碼元比對（`.` 吃半個星平面字元），桌面版以 code point 比對。
- 變數不追蹤私密〔作者裁決 2026-10-10〕：卡作者用 `{{setvar}}` 把私設或限定條目的內容存進變數，別的角色 `{{getvar}}` 讀得到（與 ST 相同，但桌面版其他地方有可見度，這裡會漏）。
- 卡欄位巨集照 P7 的對應，描述與個性回傳同一段文字；桌面版沒有的提示段落（範例對話、jailbreak）不代換。
- 送出後才失敗時，變數副作用保留（與 ST 相同），計時則回滾。

## 八、範圍外發現
- 桌面版沒有送模前 regex，條目內容不過卡內 WORLD_INFO regex（web-version D24 已記）。
- 世界書編輯器只能編標題、主鍵、內文、常駐、順序、停用、可見度；次要鍵、機率、計時、群組、位置等 ST 欄位只能從匯入的卡帶進來，玩家無法在桌面版調整。
- 網頁版世界書路的書來自頂層 `entries` 或人設欄時不觸發（照 ST），桌面版同一張卡走世界書路時這些條目照常觸發（桌面版的世界書就是這本），兩邊續玩時觸發來源不同。
