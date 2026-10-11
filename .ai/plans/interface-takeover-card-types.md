# interface-takeover-card-types：其他型別的卡驗證

**結案（2026-10-11）**：狀態欄型過、整頁型大致過；MVU 型與 RPGImmortal 待下列程式問題修好後再驗（問題已立案進 BACKLOG）。作者裁決維持接管做法、保留玩家重構選擇，重構加提醒文字〔作者裁決 2026-10-11〕。

interface-takeover-spike 待辦 2（首發必含〔作者裁決 2026-10-04〕）。西幻資料槽型已過，本檔記其餘卡型的驗證結果。露骨成人卡不拿來讓模型生成內容；MVU 前端型與整頁非 MVU 型改用非情色仿製卡驗〔作者裁決 2026-10-11〕。

## 分型（讀 `TestCards/` 原卡 chara chunk）

| 型別 | 卡 | 判別依據 | 露骨成人 |
|---|---|---|---|
| 資料槽型 ✅ | WestFantsy | `<GoldenRPG_UI>` 多群捕捉 | 有閘門式露骨段（已過，不重跑） |
| MVU 前端型 | bcd368…、HeroTrainingUnderSide、DongeonMaster | `<StatusPlaceHolderImpl/>`＋`[mvu_update]`／`[initvar]`＋酒館助手 MVU bundle | 全部是 → 用 `DongeonMaster-safe.png` |
| 整頁前端非 MVU | Transfur | `<gametext>` 整則換成 HTML＋捏人頁 | 是 → 用 `Transfur-safe.png` |
| 狀態欄型＋多人物 | NorthHall-structure | `<Status_block>` YAML、在場多人 | 否 |
| 狀態欄型＋小地圖 | RPGImmortal | `<Status_block>`＋開局頁＋BGM | 否 |
| 雲端載入器 | TrainEmperor | `云端版` 腳本 111 字元 | 是（只驗擋下） |

## 仿製卡（`TestCards/*-safe.png`，gitignored）

用原卡 PNG 換掉 `chara`／`ccv3` 兩個 tEXt chunk，圖不變；卡名加「（安全仿製）」。

- **DongeonMaster-safe**：全卡（世界書、MVU schema、initvar、`[mvu_update]` 規則、三支 regex HTML）套同一張字詞對照表，變數名一致改（`情欲值→疲劳值`、`恶堕值→恐惧值`、`精液_或_乳液储量→体力储备`、`繁殖区→培育区` 等）；五條房間衍生條目（培育／存储／休息／异化／欲望区）與開場白整段改寫成一般地城經營；其餘條目只改個別句子。
- **Transfur-safe**：捏人頁刪掉「性別特徵」卡片、「性徵特點」摘要、欲望值欄，兩個職業與三個技能改成一般職能，送出開局訊息前刪掉 `reproductive_organs`／`sexual_orientation`；Gametext 刪掉生殖系統區塊，`Lust` 改 `SP`（耐力值）；格式條目刪掉 genitalia／reproSystem 與 Lust；七條世界書（异化、局部异化、团地区、卡斯特罗镇、学术区、药物清单、物品）整段改寫，其餘條目只改個別句子。
- **殘留**：只剩不顯示、不送模型的 JS 識別字（`penis`、`sexualCharacteristics` 等變數名）與無害字（鹅卵石、哺乳、钟乳石）。模型讀得到的文字（世界書、開場白、送出的開局訊息）零命中。
- **零額度核對**（測試通道，ai-log 零派送）：與原卡一致——都判世界書卡、條目數 25／16、`card_interfaces` 腳本名與 mvu 旗標相同、匯入後自動打開介面、開場畫面畫得出。

## 零額度驗證（2026-10-11）

六張原卡各開新桌匯入：全部判成世界書卡、條目數與原卡一致、自動打開卡片介面、空桌開場畫面畫得出。TrainEmperor：`card_interfaces` 回 unsupported、匯入提示「要連到作者的網站」、按「重構」顯示「介面由外部網站載入…暫不支援重構」，零派送。

## 真模型驗證（2026-10-11，測試通道，claude CLI）

做法：重構用 sonnet（balanced），實聊切 haiku（fast）；同卡另開未重構的基準桌直玩 1 回合，接管桌 3 回合。開場白以 `post_opening` 貼出（等同挑選框「貼出這條」）。測試設定要帶 `cli_risk_accepted: true`，否則初判在派送前就失敗、畫面直接跳二選一且沒有建議。

### 結果（比照西幻表）

| 卡 | 重構結果 | 基準回合 輸出 tok／原文字數 | 接管 1／2／3 回合 輸出 tok | 原文字數 | 劇情字數 | patch（送出／套用） | 畫面 |
|---|---|---|---|---|---|---|---|
| NorthHall-structure | 狀態欄骨架、8 欄、介面已套用（初判建議多角色，手選介面） | 6604／1525 | 2460／2145／2330 | 1620／1471／1835 | 601／617／815 | 4/3、3/3、3/3（第 1 回合 haiku JSON 缺 `}` 掉 1 條） | 與原卡同版面，時間地點、人物卡、物資跟動 |
| RPGImmortal | **介面沒套用**：「介面產物互相衝突（行動選項）」，連跑兩次都一樣 | — | 未跑 | — | — | — | — |
| DongeonMaster-safe（MVU） | 沒建殼（同 bcd368），`[mvu_update]` 輸出格式兩條被淘汰、更新規則被收成機制 | 3093／2353，9 條路徑全對 | 1770／2362／3730 | 1587／2172／3310 | 687／1112／2137 | 8/0、9/0、7/0（19 條路徑不存在被拒，5 條 insert 長出 `Dungeon` 等英文新枝） | MVU 前端照畫，但數值不會動 |
| Transfur-safe（整頁） | 骨架 55 個佔位符、介面已套用 | 7803／5267 | 1940／2207／1849 | 1427／1587／1267 | 688／1157／687 | 7/6、5/4、6/5（`神經狀態` 每回合下 delta 被拒） | 七分頁版面同原卡，時間地點跟動；正文只顯示第一段 |

### 實際用量（ai-log 計費估算）

| 卡 | 重構 sonnet | 實聊 haiku | 小計 |
|---|---|---|---|
| NorthHall-structure | 6 次，輸入 47.9K（快取 25.8K）、輸出 5.6K | 4 次，輸入 32.1K、輸出 13.5K | $0.35 |
| RPGImmortal | 8 次（兩輪），輸入 105.7K（快取 64.7K）、輸出 28.1K | 0 | $0.50 |
| DongeonMaster-safe | 3 次，輸入 150.4K（快取 91.5K）、輸出 4.9K | 4 次，輸入 113.7K、輸出 11.0K | $0.81 |
| Transfur-safe | 3 次，輸入 90.6K（快取 50.4K）、輸出 12.1K | 4 次，輸入 106.1K、輸出 13.8K | $0.72 |
| 合計 | | | **$2.37**（32 次呼叫）；Opus 補跑另 $0.96（3 次） |

### Opus 補跑（2026-10-11，中途停止）
DongeonMaster-safe 重構改用 gm_tier 高階檔：route 確認初判與盤點走 opus、展開仍走 sonnet（`refactor_expand_tier` 固定 balanced）。結果與 sonnet 版相同：沒建殼，`[mvu_update]` 輸出格式兩條照樣淘汰、更新規則照樣收成機制，另列「未接管機制 4 項」。3 次呼叫（opus 2、sonnet 1），輸入 152.1K（快取 91.9K）、輸出 6.7K，$0.96。未套用、未實聊；RPGImmortal 的 Opus 重構沒跑。作者隨即改為不深究 MVU 失敗原因、在重構加提醒文字〔作者裁決 2026-10-11〕，補跑停止。MVU 型維持原接管做法、保留玩家重構選擇，「重構本來就不保證可以玩」〔作者裁決 2026-10-11〕。

### 驗收判定
- **狀態欄型（NorthHall-structure）**：過。畫面一致、狀態跟動、輸出 token 比基準少約 65%、劇情字數高於基準。瑕疵：`行動選項` 三回合都沒更新、`天氣` 從未回報（guide 寫每回合必報）。
- **整頁非 MVU（Transfur-safe）**：大致過。版面一致、輸出 token 少約 75%；但正文只顯示第一段（見問題 3）、`神經狀態` 更新全被拒。
- **MVU 前端型（DongeonMaster-safe）**：不過。重構後 GM 失去變數結構，patch 全數失效；未重構時反而正常。
- **狀態欄＋小地圖（RPGImmortal）**：不過，接管套不上。

### 發現的程式問題（只記錄，未修）
1. **多來源介面互相衝突就整組不套**：RPGImmortal 的「格式增强」與「美化状态栏」兩條都產出 `行動選項`，套用時判衝突、整組介面丟掉。兩次重跑結果相同。
2. **MVU 卡重構後 GM 不知道變數路徑**：`[mvu_update]变量输出格式`／`强调` 以 rule 1 淘汰，更新規則被收成機制，機制欄位名轉成繁體（`冒險者`），狀態樹仍是簡體（`冒险者`）。GM 自創英文路徑，replace／delta 全被拒，insert 長出孤兒枝。另外每回合輸入 16K→28K→47K，增長很快。
3. **骨架多行值填錯（已修，[takeover-text-multiline](takeover-text-multiline.md)）**：兩個不同根因。Transfur 骨架 `mainText|{{本回合.正文}}` 是一段一筆的記錄式格式，正文第二段起沒補 `mainText|` 前綴；RPGImmortal 骨架的鍵本身是佔位（`{{地圖類型}}:`），被當成純量加引號，YAML 解析失敗（不是縮排問題）。
4. **文字欄被下 delta（已修，[takeover-text-field-delta](takeover-text-field-delta.md)）**：Transfur 接管桌 GM 對 `神經狀態`（文字欄）每回合下 delta，3/3 被拒（與 refactor-statusbar-skeleton 待觀察 ② 同類）。
5. **開場白的原卡狀態欄被解析成多餘枝（已修，[takeover-opening-statusbar-branch](takeover-opening-statusbar-branch.md)）**：NorthHall 接管桌貼開場白後，狀態樹多出 `状态栏` 整棵（原卡 `<Status_block>` 的 YAML），與骨架欄位並存。
6. **初判失敗不留痕（已修，[refactor-probe-failure-hidden](refactor-probe-failure-hidden.md)）**：設定缺 `cli_risk_accepted` 時初判在派送前失敗，畫面只看到沒有建議的二選一，看不出原因。

## 重驗（2026-10-12）

介面接管相關修正進 main 後（db44fb45）重跑兩張不過的卡。測試通道、claude CLI；重構 `gm_tier=best`（初判與盤點 opus、展開 sonnet），套用後切 `balanced` 用 sonnet 實聊 3 回合；開場白以 `post_opening` 貼出（scene 取 `read_state.current_scene`）。本輪不跑未重構基準桌。卡片介面在 sandbox iframe 內點不到，分頁內容用 srcdoc 快照在瀏覽器點開看。

| 卡 | 重構結果 | 接管 1／2／3 回合 輸出 tok | 原文字數 | 劇情字數 | patch（送出／套用） | 畫面 |
|---|---|---|---|---|---|---|
| DongeonMaster-safe（MVU） | 照建議保留原卡玩法；重建 23 條，`[mvu_update]` 更新規則收成機制、輸出格式兩條淘汰（同上輪） | 2778／3919／3807 | 1629／2256／2102 | 713／898／859 | 8/8、5/5、4/4（含 insert 新房間、魔物、冒險者小隊） | 開局頁→迷宮之書照畫；地下構造圖出現新房間與「魔:2 隊:1」，小隊頁列出新小隊 |
| RPGImmortal | 照建議保留原卡玩法；「遊戲介面搬進 app」、重建 8 條，介面已套用（未接管機制 3 項，原文留在 GM 規則） | 4023／3445／3801 | 2896／2033／2776 | 676／627／585 | 13/13、8/8、10/10 | 開局頁照畫；狀態欄時間地點、行動選項每回合跟動；小地圖照元素座標畫出，在場 NPC 分頁跟動 |

用量（ai-log）：DongeonMaster-safe 重構 3 次 $1.17、實聊 3 次 $0.71；RPGImmortal 重構 4 次 $0.54、實聊 3 次 $0.43；合計 **$2.85**。

### 判定
- **MVU 前端型（DongeonMaster-safe）**：過。patch 全數套用，路徑對得上原卡簡體樹，介面數值跟動。
- **狀態欄＋小地圖（RPGImmortal）**：過。介面套上，三回合欄位全數套用、畫面跟動。

### 問題清單
①（擋住接管後能玩下去）：無。

②（瑕疵，只列不修）：
- DongeonMaster-safe 每回合輸入 21.7K→42.8K→59.5K，增長仍快（RPGImmortal 9.7K→16.4K→22.7K）；未查原因。
- DongeonMaster-safe insert 值裡的陣列（`魔物: []`、`["Mon_001","Mon_002"]`）存成索引分支（`{}`、`{"0":…,"1":…}`），布林 `false` 存成字串；介面目前照畫。
- DongeonMaster-safe GM 寫入的文字值是繁體，原卡樹與介面是簡體，混排。
- RPGImmortal 介面標題與主角名顯示字面 `{{user}}`（桌上沒放玩家卡）。
- RPGImmortal 骨架清單欄（`小地圖元素`、`在場NPC`、`行動選項` 等）GM 整段連縮排的 YAML 片段寫進文字值；介面解析正常。

證據（本機暫存，`/private/tmp/claude-501/-Users-<user>-GitHub-Table-Tavern/1b77609f-1ee3-454f-b35a-437005d7c286/scratchpad/recheck/`）：`dm/`、`rpg/` 為測試 root（`harness-ai.log`、`data/worlds/*/transcript/0.jsonl`、`state.json`）；截圖 `dm-0-opening.png`、`dm-1.png`、`dm-3-map.jpg`、`dm-3-squad.jpg`、`rpg-0-opening.png`、`rpg-1.png`、`rpg-3.png`、`rpg-3-minimap.jpg`。
