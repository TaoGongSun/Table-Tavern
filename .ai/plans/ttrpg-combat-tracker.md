# ttrpg-combat-tracker：跑團戰鬥層（方案）

案別：**大案**，核心重估約 780 行（Rust 約 450、桌面前端約 240、房間協定與手機約 90），立案估的 150–250 偏低（理由見 §0 缺口；方案審第 1 輪補的 HP 專用驗證、寫入口連帶清理、拒收標記再加約 180）；模型輸出與房間協定是信任邊界輸入，方案與驗收都送審。全篇取捨未標來源者為〔模型判斷·未裁決〕。

已拍〔作者裁決 2026-11-07〕：模型回報事件、加減由 app 做；套不上照常遊戲、面板不動；先攻由 GM 在戰鬥開場排一次、玩家可手動上下調；多人房間不另做輪次（只在面板標「輪到誰」並餵 GM，送出照走房間回合列）；擲骰維持 app 真隨機底線。

## 0 開工重讀現況（範圍重估的依據）

**已存在、只要接上：**
- **「app 做加減」的骨架已在**：`FieldKind::Pair`（「現值/上限」）＋`UpdateMode::Delta` 走 `mechanism/apply.rs::apply_delta`——現值＋增量、夾在 `[min（預設 0）, 上限]`、超界記 `RecordKind::Clamped` 並回饋、`changes` 記 `-7`／`+5` 標記；模型用 replace 報結果會被擋下並回饋「請用增減量」。但 `tree::split_pair` 收任意浮點數、上限可為負（`clamp` 在 min > max 時 panic），戰鬥 HP 要另走專用驗證（§2.4），不沿用它的寬鬆解析。
- **狀態效果**＝清單欄，照 `items` 的作法（replace 整串字串陣列、`budget.rs` 夾項數與字數、`panel_target` 擋逐項寫），零新機制。
- **預設狀態欄容器**（table-status-panel）：角色格（以卡 ID 歸屬、改名搬格）、守門 `write_guard::panel_target`／`panel_patch`／`panel_fence`、預算、`set_panel_list`／`set_state_path` 面板手改、房間 `scene_panel` 廣播＋`cleanScenePanel`＋手機抽屜、桌面 `DefaultStatePanel`。
- **骰池在提交當下就在手上**：`chat_turn/gm.rs::gm_narrate` 的 `dice_pool: Option<String>`（`dice::begin` 產生），單人與房間同一條路，`turn.commit` 閉包抓得到。

**缺口（核心行數來源）：**
1. **數字欄的啟用規則其實不存在**：任務檔寫「沿用 table-status-panel 的啟用規則」，但該案把數字欄整個交給本案（其方案 §2.5「數字欄（HP 等）不在本案」），程式裡沒有任何 HP／先攻／數字欄種入。本案要自己定（§2.1）。
2. **Rust 沒有骰號 `(d20#2=19)` 解析**：只有前端 `src/features/dice/dice-pool.ts::inspectPool`；Rust 只有玩家骰鈕的 `XdY±Z`。綁骰要在 `dice.rs` 補池解析與骰號算式。
3. **已種過的預設桌不會補新欄**：`default_panel::sync` 只在第一次種時 `rules.extend(seed_rules())`；要加冪等的「補戰鬥欄」，且不能蓋掉玩家自訂的同名欄（§2.1）。
4. **Pair 驗證太寬、從空值起不來**：見上；戰鬥 HP 要專用驗證（§2.4）。
5. **守門只認 `characters`／`tracking` 兩根**：先攻放新根 `combat` 要一起進守門、預算、廣播白名單、伺服器驗證。`apply_insert` 碰到已存在節點會轉成替換，敵人 insert 要另擋（§2.7）。
6. **手改入口看不到樹**：`panel_hand_value` 只截字、`set_panel_list` 不跑連帶清理；成員驗證與清理要接進真正的寫入入口（§2.6）。
7. **同路徑變動標記會被蓋掉**：`apply.rs` 的 `changes.insert` 以路徑為鍵，同一則扣兩次只留最後一筆（§2.4）。
8. `panel_prompt::guide` 是凍結靜態文，不能改（改一字整條快取滅）；戰鬥另加一節。`default_panel::label` 要補顯示名。
9. 先攻、輪到誰、無卡敵人、拒收標記：全新。

## 1 玩家看到什麼

- 開了擲骰的預設桌（空桌、一句話開桌、房間一句話開世界、沒狀態欄格式的卡），每個角色格多「HP」與「狀態效果」兩欄（一開始空著）。擲骰開關、匯入提示與指南的文案都寫明這件事（§3）。
- 戰鬥開打：GM 照骰池擲先攻並排好順序，面板出現「戰鬥」區：先攻順序一列、目前輪到的人標亮；GM 臨時加的敵人（哥布林A、哥布林B）各一小格，有 HP 與狀態效果。收合列多一段「⚔ 輪到 雷克」；有順序但還沒設輪到誰時顯示「⚔ 戰鬥中」，先攻列下一句「點名字設定輪到誰」。
- 傷害／治療：GM 敘事裡寫骰號，HP 由 app 扣，格旁出現本則的淨變動（同一則扣兩次顯示 `-12`）；扣到 0 停在 0，補過上限停在上限。
- 玩家可在戰鬥區用 ↑↓ 調先攻順序、點名字改「輪到誰」、點 HP 直接改。
- 戰鬥結束：GM 清空先攻，戰鬥區與敵人格收起，留一行「戰鬥結束，收回上一則可復原」；角色 HP 與狀態效果留著（下一場延續）。
- 模型報錯（亂寫路徑、骰號對不上、報結果不報增量）：故事照常，那一筆不套；被拒的那一格出現小標記「GM 寫 X，未套用」，點開就能手改；下一輪 GM 也收到一句回饋。

## 2 後端（Rust）

### 2.1 數字欄何時開

- 條件：`mechanism.default_panel && state.dice_enabled`。擲骰開關已經是規則書／骰子記號偵測（匯入提示）與桌設定手動開關的同一顆（ttrpg-rules-system B 包），不另加設定。
- 時機：`default_panel::ensure` 每回合都跑（已有），`sync` 加一段：條件成立且 `Mechanism.combat_panel == false` → 補規則、設 `combat_panel = true`；之後每回合替每格補缺的 `hp` 葉（空字串）與 `conditions` 分支，`combat` 分支補 `order`／`turn`／`foes`。沒變動不寫檔（規則進系統提示，與上一案同一個快取紀律）。
- **同名衝突**：玩家已在某格用「＋欄位」加過 `hp` 或 `conditions`（精確規則優先於萬用規則）——保留原值與原規則，不覆蓋、不轉型、不補分支（補欄只在鍵不存在時插入，不用會把葉子換成空分支的 `branch_mut`）；那一格的該欄就照玩家的文字欄運作，不掛戰鬥驗證。之後「＋欄位」的保留名加入 `hp`／`conditions`／`combat`，新桌不會再撞。測試：有衝突的桌重跑 ensure 不改值、不寫檔。
- 新欄位 `Mechanism.combat_panel: bool`（落檔、serde default）。**種過就留**：之後關擲骰不收回欄位（收回＝丟玩家的 HP 資料）；關掉只停骰池，欄位與指引照舊。

### 2.2 樹形與規則（鍵固定英文，顯示才在地化）

```json
{
  "characters": { "<格鍵>": { "status": "", "items": [], "hp": "", "conditions": [] } },
  "combat": {
    "order": ["沃恩", "哥布林A", "雷克"],
    "turn": "雷克",
    "foes": { "哥布林A": { "hp": "7/7", "conditions": [] } }
  }
}
```

| 路徑 | kind | update | inject | 預算／驗證 |
|---|---|---|---|---|
| `characters.*.hp`、`combat.foes.*.hp` | pair | delta | turn | 戰鬥 HP 專用驗證（§2.4） |
| `characters.*.conditions`、`combat.foes.*.conditions` | list | replace（整串） | turn | ≤ 6 項、每項 ≤ 16 字 |
| `combat.order` | list | replace（整串） | turn | ≤ 12 項、每項 ≤ 24 字，去重 |
| `combat.turn` | text | replace | turn | 必須是 `order` 裡的一項 |
| `combat.foes.<名>` | （整筆） | insert／remove | — | ≤ 8 筆、名 ≤ 24 字、只准新增不存在的名字，值只收 `{hp, conditions?}` |

- 常數放 `data/default_panel.rs`（與既有預算同一處），伺服器另抄一份（§4）。
- 顯示名：`label` 補 HP／狀態效果／戰鬥／先攻／輪到／敵人（zh／en）。

### 2.3 模型怎麼回報（沿用增量協定，不開新文法）

「事件」＝對上表欄位的 `<UpdateVariable>` 補丁；誰打誰不報，只報結果欄的增減。

- **開打**：`replace /combat/order` 整串（照骰池擲的先攻排好）、`insert /combat/foes/哥布林A {"hp":"7/7"}`、角色 HP 第一次 `replace /characters/沃恩/hp "24/24"`。
- **傷害／治療**：`{"op":"delta","path":"/combat/foes/哥布林A/hp","value":-7,"dice":"d8#1+3"}`；`dice` 選填。
- **狀態效果**：`replace /characters/雷克/conditions ["中毒"]`（加一項也重寫整串）。
- **換人**：`replace /combat/turn "雷克"`。
- **收場**：`replace /combat/order []` → app 一併清空 `turn` 與 `foes`；敵人中途倒下或逃走可 `remove /combat/foes/<名>`。

### 2.4 「app 做加減」的確切語意（戰鬥 HP 專用驗證）

- 模型報**變化量**（delta），app 算結果；`replace` 報結果一律拒收回饋。
- **戰鬥 HP 專用驗證**（新，`mechanism/combat.rs::hp`，只套在 §2.2 兩條 HP 路徑，不改其他卡片 Pair 的語意）：合法 HP＝`現值/上限` 兩個非負整數、`0 ≤ 現值 ≤ 上限`、`1 ≤ 上限 ≤ 99999`；小數、非有限值、負數、越界一律不合法。四個入口共用：
  1. **初始化**：現值是空字串時，`replace` 收合法 HP 整個寫入；不合法拒收。
  2. **delta**：增量須為有限整數、絕對值 ≤ 99999；現值須合法（手改留下的非法值→安全拒收並回饋「HP 現值格式不對，請玩家修正」，不進 `clamp`，不會 panic）；結果夾在 `[0, 上限]`，超界記 Clamped 並回饋目前值。
  3. **改上限**：`replace "現值/新上限"` 只在現值等於目前現值、新上限合法時生效；現值不符就整筆拒收（不再「拒現值但照改上限」），回饋「請用增減量改現值，改上限時現值照抄」。
  4. **敵人 insert**：值裡的 `hp` 須合法，否則整筆拒收。
- 玩家手改（`set_state_path`）照舊放行任何值（可留非法值），之後模型運算碰到就走上面的安全拒收。
- **同一則多筆的淨變動**：`changes` 標記改為累加——同一路徑在同一次套用已有 `±N` 標記時，新 delta 加上去再寫回（`-5` 再 `-7` 標 `-12`）。累加的是每次成功寫入的「新值 − 舊值」（夾到上下限、取整之後的實際變化），被拒收的不算：HP 3/10 先 −5 再 +5 得 5/10，標 `+2`。這是對既有標記行為的修正，所有桌一體適用（舊行為在同一則多次增減時本來就標錯）。

### 2.5 綁骰池（`dice.rs` 新增約 80 行）

- `Patch` 加選填 `dice: Option<String>`（`parse.rs` 從物件取字串，≤ 64 字；其他型別拒收該筆）。只對戰鬥 HP 的 delta 有意義，帶在別種補丁上就忽略。連帶改約 55 處測試的 struct literal（純型別連帶，不計核心）。
- 算式文法：`dN#k` 以 `+` 相連，最後可接一個 `±整數`（例 `d8#1+d6#3+3`）；最多 10 顆。
- 驗法（GM 提交時，`gm_commit_apply` 多傳本輪池字串進 `apply_block_typed`）：
  1. 本輪沒有池 → 拒收該筆（不能引用不存在的池）。
  2. 每顆骰型／序號都要在本輪池裡；同一顆在本則回覆裡只能用一次 → 否則拒收。「已用骰號」集合只活在這一次整則套用裡、跨本則所有更新區塊共用，不另做持久化的耗骰帳；防重套靠既有 `live_turn` 與冪等落檔。
  3. 骰面一律取池裡的值（模型改不了面值），總和＝Σ骰面＋常數。
  4. **總和 ≠ |value| 時以骰面為準套用**：取 value 的正負號、套骰面總和，回饋「你報 -5，依骰面套 -7」，該格標記顯示實際套的淨值並附「GM 寫 -5」。〔作者裁決 2026-11-08〕
     邊界（方案審第 2 輪）：骰面總和（Σ骰面＋常數）為負 → 拒收（免得負號把傷害變治療）；總和非 0 但 value 為 0 → 方向不明，拒收；改算出的增量再過 §2.4 的 delta 驗證（有限整數、絕對值 ≤ 99999），全程用 i64 安全整數運算。
- **不帶 `dice` 的 HP 變動照收**（固定傷害、墜落、治療藥水、減半傷害都進得來）。〔模型判斷·未裁決；Fable 常識審同意〕
- 敘事正文的骰號對帳照舊由前端 `inspectPool` 標紅，兩者互不依賴。

### 2.6 先攻與「輪到誰」

- 存在狀態樹 `combat` 分支（不是 `WorldState` 欄）：跟著逐則快照；inject turn 讓 GM 每輪的「目前狀態」自動帶到先攻與輪到誰，餵 GM 不另寫管線。收回／復原要另驗（見 §5），不假設天生成立。
- **誰推進**：GM 每則旁白自己 `replace /combat/turn`；app 不自動 +1（一則旁白常演完好幾人的回合）。
- **成員驗證與連帶清理**集中成 `combat.rs::settle(tree) -> 是否變動`：`turn` 不在 `order` 就清空；`order` 空了就清空 `turn` 與 `foes`。接在每個真正的寫入入口，**同一次提交內、存最後一則快照之前**做完：
  - 模型入口：`apply_block_typed` 套完全部補丁與圍欄、產生最終快照之前跑一次。
  - `set_panel_list`（改 `combat.order`）：寫完清單、`set_last_transcript_state` 之前跑。
  - `set_state_path`（改 `combat.turn`）：在讀得到樹的那一段（`update_state` 閉包內）先驗新值 ∈ `order`，不在就回錯不寫；`panel_hand_value` 不負責這件事（它看不到樹）。
- **玩家手動調整**：↑↓ 呼叫既有 `set_panel_list` 重寫 `combat.order`；點名字呼叫 `set_state_path` 寫 `combat.turn`。不開新指令。
- `order` 收任意名字（角色格、敵人、沒 HP 的 NPC 都可），只夾預算與去重。

### 2.7 守門（信任邊界：模型輸出）

新模組 `mechanism/combat.rs`（不擠進 `write_guard.rs`），由 `panel_target`／`panel_patch`／`panel_fence` 把 `combat` 根轉過去：
- `combat` 整支、`combat.order`／`turn`／`foes` 以外的表外鍵：拒收（同上一案措辭）。
- `combat.foes.<名>`：insert **只准新增不存在的名字**——同名已存在就整筆拒收、回饋「已有哥布林A，扣血請用 delta；新敵人請取不同名字」，在進 `apply_insert`（它會把已存在節點轉成替換、蓋掉扣過的 HP）之前擋下。另驗名 ≤ 24 字、不含 `.`／`*`／底線開頭、未滿 8 筆、值形狀 `{hp, conditions?: [字串]}`（其餘鍵丟掉、`hp` 走 §2.4）。還沒有非空的先攻順序時整筆拒收並回饋（先排順序再加敵人）。remove 收；replace／move 整筆拒收。敵人底下只收 `hp`（§2.4）與 `conditions`（整串 replace），且該敵人要已存在——新敵人只能走整筆 insert。
- `characters.*.hp`／`conditions` 走既有角色格守門（模型照樣開不了格）。
- 圍欄入口（```state 裡巢狀寫 `combat`）：只收 `order`、`turn`、既有敵人的 `conditions`；HP 只走補丁（圍欄寫 HP 拒收回饋「HP 請用 delta」）。開新敵人只走補丁 insert。

### 2.8 拒收標記

- `TableState` 加 `refused: BTreeMap<路徑, 一行>`（照 `jumps`：只給玩家看、不進提示詞、每次套用重算、跟著逐則快照）。範圍只限戰鬥欄位（兩種 HP、兩種狀態效果、`order`、`turn`、敵人整筆）：被拒收時記「GM 寫 X，未套用：原因」（骰面改套也記「GM 寫 -5，依骰面套 -7」）。既有的模型回饋句照舊進 `notes`。
- 標記不另做清除：下一則套用重算、玩家手改那一欄就刪那一條。

### 2.9 提示詞

- 不動 `panel_prompt::guide`。另加 `panel_prompt::combat_guide(lang)`（靜態 zh／en，進凍結快照），只在 `combat_panel` 桌接在 guide 之後：上表各欄是什麼、HP 一律報增量並在有引用骰池時帶 `dice`、有減半等調整就不帶、開打排先攻與加敵人、**同種敵人要取不同名字（例如哥布林A、哥布林B）**、同名不能再 insert、每則更新 `turn`、收場清空 `order`、各欄上限。
- 範例：`combat_panel` 桌在固定範例後多一行 `{"op":"delta","path":"/characters/<第一格>/hp","value":-7,"dice":"d8#1+3"}`。
- 開擲骰那一刻系統提示會變一次（指引進凍結快照）：與擲骰開關本身已觸發的 system-changed 重開同一刻，不多滅一次快取。

## 3 桌面版面板（`src/features/table-state/`）與擲骰文案

- 角色格：status 下多「HP」一行（顯示 `23/40`，有 `changes` 標記就顯示本則淨值；點改照既有 `renderLeaf`）與「狀態效果」chip 列（照 items）。
- 「戰鬥」區（`combat.order` 非空才出現，放在角色格之上）：先攻一列、每項 ↑↓ 鈕與「輪到」標亮，點名字一鍵設輪到；`turn` 空時列下一句「點名字設定輪到誰」。敵人小格（名、HP、狀態效果），HP 點了就能直接改。
- 拒收標記：有 `refused` 的格旁一個小記號（照骰池「GM 寫 X，實際是 Y」的樣子），點開顯示那一行並直接進該欄的手改。
- 收合摘要：戰鬥中「⚔ 輪到 {名}」，`turn` 空時「⚔ 戰鬥中」；不出現「輪到 」後面空白。
- 戰鬥收起：前一則有戰鬥、這一則 `order` 空了，戰鬥區位置留一行「戰鬥結束，收回上一則可復原」（比對上一則快照即可，不另存狀態），下一則旁白後消失。
- `combat`／`hp`／`conditions` 進 `RESERVED`（＋欄位不得重名）。
- **擲骰文案**（十語系，不計核心）：`dice-copy.json` 的 prompt／roomPrompt／enabledSolo／enabledRoom／intro、匯入偵測提示、規則指南，都補一句「開啟後角色格會多出 HP 與狀態效果，戰鬥時有先攻順序；這些欄位種下後會保留，之後關掉擲骰也不收回」。
- 檔案：戰鬥區拆成 `CombatSection.tsx`，`DefaultStatePanel.tsx` 只掛它；i18n 十語。

## 4 房間（協定＋手機抽屜）

- `gm.rs::scene_panel` 白名單加：每格 `hp`（字串）、`conditions`（字串陣列）；頂層 `combat: {order, turn, foes: {名: {hp, conditions}}}`。`combat` 只在 `order` 非空時附。仍是整份快照、仍只在預設桌送；seed／sync 已共用同一函式。`refused` 不廣播（房間玩家唯讀，標記只給房主桌面）。
- 伺服器 `party/server/core/validate.ts::cleanScenePanel`（信任邊界：房間協定）：`hp` 須符合 `^\d{1,5}/\d{1,5}$` 且現值 ≤ 上限、上限 ≥ 1，否則丟該鍵；`conditions` ≤ 6×16；`order` ≤ 12×24；`turn` 不在 `order` 就丟；`foes` ≤ 8、名 ≤ 24、每筆同角色格驗法；型別不對丟該鍵、全丟不附。常數加進 `PANEL_LIMITS`，`types.ts::ScenePanel` 補型別。
- 手機抽屜（`party/src/features/room/scene-status.tsx`）：角色格加 HP 與狀態效果；有 `combat` 時頂端一塊先攻列（輪到者標亮）與敵人格，唯讀。收合列同桌面：「⚔ 輪到 {名}」／「⚔ 戰鬥中」。
- 可見性：v1 敵人 HP 對全房公開（與單人桌面一致）。〔模型判斷·未裁決；Fable 常識審同意〕

## 5 分包與驗收（各包獨立 commit、各送一次驗收）

- **包 1 後端**（Rust，核心約 450）：§2 全部＋`scene_panel` 擴充。測試：補戰鬥欄冪等不寫檔、關擲骰不收回、**玩家自訂同名 `hp`／`conditions` 保留原值與規則且不寫檔**；HP 四入口：初始化收合法／拒小數負數越界、delta 夾 0 與上限、非法現值安全拒收不 panic、改上限現值不符整筆拒收、敵人 insert 帶非法 HP 拒收；**同名敵人 insert 拒收且原 HP 不變**；replace 報結果拒收；同一則兩次 delta 標記為淨值；`dice` 有效套用、骰號不存在／跨區塊重用／無池各拒收且樹不動、總和不等依骰面套用並回饋、記標記；不帶 `dice` 照收；`turn` 不在 `order` 拒收；`settle` 在模型入口、`set_panel_list`、`set_state_path` 三處都在存快照前生效（最後一則快照即清理後的樹）；**收回後先攻／HP／敵人回到前一則、復原再回來**；`refused` 只記戰鬥欄、手改即刪；表外鍵與圍欄寫 HP 拒收；卡格式桌 `rule_for` 不變；實送提示含戰鬥指引（含敵人取不同名）與範例；`scene_panel` 帶 combat、不帶 refused。`cargo test` 綠。
- **包 2 桌面＋房間**（TS／CSS，核心約 330）：§3、§4。測試：戰鬥區只在 order 非空出現、↑↓ 呼叫 `set_panel_list` 順序正確、點名字呼叫 `set_state_path`、`turn` 空時摘要與提示句、拒收標記點開進手改、戰鬥結束提示只在前一則有戰鬥時出現；`cleanScenePanel` 邊界（hp 形狀、上限 0、turn ∉ order、foes 上限）；抽屜渲染。測試通道截圖：側欄戰鬥區（含拒收標記、turn 空）、戰鬥結束那一行、窄視窗、手機 375×812 抽屜（主線親看＋Fable 截圖審）。
- 收尾：`.ai/reference/MECHANISM-FORMAT.md` 預設狀態欄節補戰鬥欄；交接檔改寫；process-log 記一條；`npm run verify` 全綠；實測佇列加一項（真模型跑一場戰鬥，費用先報作者）。

## 5a 施工結果

- **包 1 後端**（2026-11-08）：`data/default_panel/combat.rs`（規則、種入、`settle`、`in_order`）、`mechanism/combat.rs`（HP 四入口、綁骰、敵人整筆、輪到誰、拒收標記、圍欄拒收）、`dice.rs::resolve`、`apply.rs` 淨值標記、`ledger.rs` 收尾 settle＋refused、`write_guard.rs` 接 `combat` 根、`commands/state.rs` 兩個手改入口、`gm.rs` 本輪池帶進提交與 `scene_panel` 戰鬥區、`panel_prompt.rs` 戰鬥指引與範例。核心（非空白、非註解、非型別連帶）約 811 行，其中 rustfmt 把拒收句拆行與提示詞文字約佔 1/3；測試 615 行。比估計 450 多，主要是每個拒收都帶給模型的具體回饋句。
- **包 2 桌面＋房間**（2026-11-08）：`src/features/table-state/CombatSection.tsx`（先攻列 ↑↓、點名字設輪到誰、敵人格、戰鬥結束一行、拒收標記元件）＋`combat.css`；`StateBar.tsx` 收合列戰鬥段、HP 淨值標記、拒收標記點開手改；`DefaultStatePanel.tsx` 角色格 HP／狀態效果；`useTableStateController.ts` 讀 `changes`／`refused`、`setPanelList`／`setLeaf`；房間 `validate.ts::cleanScenePanel` 驗 HP、先攻、輪到誰、敵人（同一組上限）、手機 `scene-status.tsx` 抽屜與收合列；擲骰文案十語系補一句。核心約 323（TS）＋CSS 79，測試 211，i18n 250 不計。戰鬥結束一行由後端在 `ledger` 判定同一幕裡先攻非空→空，記在 `TableState.combat_ended`（場景號），前端只在等於目前場景時顯示，悔棋與還原跟著快照走。
- 包 1 驗收第 1 輪修正：敵人子欄要敵人已存在才收（新敵人只走整筆 insert）；沒有非空先攻順序時新增敵人整筆拒收並回饋；`[值, 說明]` 的帶號標記也走 `add_delta_mark` 累加；圍欄戰鬥拒收句只算一次。
- 包 2 修正（Sol／Fable 驗收）：沒先攻時的拒收標記照樣顯示；清單欄（含角色與敵人的狀態效果）加整串編輯，點拒收標記也打開，存檔走 `set_panel_list`；收合列拿掉 ⚔、只留橘色「輪到 X／戰鬥中」；依骰面改套改灰字「依骰面 -7（GM 寫 -5）」、⚠ 只給真的沒套上；手機收合列各段不斷行、戰鬥段排第一；手機敵人一行一隻、角色格加淡底框。
- 與方案的差異：圍欄連既有敵人的 `conditions` 也不收（敵人一律只走補丁，比方案嚴）；HP 現值是玩家手改留下的非法值時，`replace` 合法 HP 可直接蓋過（同空值，免得卡死）；依骰面改套那筆以 Rejected 記錄（才會進下一輪回饋），面板仍視為已套用；`set_state_path` 變數模式那條分支（MVU 卡桌）不接戰鬥驗證（預設桌不會進那條）。

## 6 已知限制〔模型判斷·未裁決〕

- 只做預設狀態欄桌；卡自帶狀態欄格式（MVU 等）的桌照卡，不掛戰鬥層。
- 數字欄種過就留，關擲骰不收回（文案已寫明）。
- 玩家自訂過同名 `hp`／`conditions` 的格，該欄照文字欄運作，不掛戰鬥驗證。
- 有減半、抗性等調整的傷害不能帶 `dice`（不綁骰照收）；app 只驗骰面與總和，不驗命中、AC、規則裁決。
- 狀態效果沒有持續回合數，由 GM 自己移除。
- 無回合數（round）計數；app 不自動推進輪到誰。
- 房間玩家不能調先攻、看不到拒收標記（房主桌面可以），與「房主端為權威」一致。
- 模型不照協定報（例：只在正文寫扣血、不送補丁）時面板不動，只能靠玩家手改。

## 7 拍板紀錄

- 骰面總和對不上：以骰面為準套用（取模型正負號）並回饋。〔作者裁決 2026-11-08〕
- 拒收標記只限戰鬥欄位。〔主線同意 2026-11-08，模型判斷·未裁決〕
- 不引骰號的 HP 變動照收；v1 敵人 HP 公開；追蹤無卡敵人（`combat.foes`）。〔模型判斷·未裁決；Fable 常識審同意〕

## 8 後續候補（v1 不做）

- 敵人 HP 改成「無傷／受傷／重傷／瀕死」四檔的隱藏版（房間玩家只看檔位、房主看數字）。〔Fable 常識審建議，模型判斷·未裁決〕

## 9 觀察（範圍外，不立案）

- 拒收標記曾露原始 JSON（`GM 寫 {"hp":"7/7"}`），主線判屬本案已修成白話原因；先攻 `combat.order` 整串被拒時仍顯示 GM 原文（罕見，未改）。

## 結案（2026-11-08）
- 包 1 後端：戰鬥欄規則與種入、HP 專用驗證四入口、綁本輪骰池（骰面為準）、敵人整筆、輪到誰、拒收標記、淨值標記、提示詞指引與範例、房間 panel 帶 combat。opus-medium 施工，Sol 方案審 2 輪＋驗收 2 輪 PASS。
- 包 2 桌面＋房間：戰鬥區、收合列「輪到 X」、清單整串編輯、拒收白話標記（原因代碼由後端給）、依骰面灰字、`combat_ended` 幕號判戰鬥結束、手機抽屜（收合列各段不斷行、敵人一行一隻、角色格 HP 在狀態下方）。opus-medium 施工，Sol 驗收 2 輪 PASS，Fable 常識審 3 次，截圖主線親看。
- 已知限制見 §6；真模型照不照協定報與實機未測（實測佇列梯 2 第 34 項）。
- 2026-11-08 真模型實測：GM 收到玩家一句「好」（只是要它接著跑敵人）時，曾連玩家的行動一起演完；只見一次，先記著。
- 2026-11-08 真模型實測：戰鬥中換幕，先攻與敵人整組帶到新幕。
- 2026-11-08 真模型實測：第一次設定 CLI 存檔成功後，仍顯示「有 1 項修改未儲存」。
