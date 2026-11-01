# vn-card-sunsmell — 以 SunSmell 檢驗 VN 總綱並設計「初始 VN 卡」

研究報告（2026-11-01）。對象：SunSmell（Unity＋Naninovel，作者自寫的完整 VN 文本）。對照 [vn-mode](vn-mode.md)（下稱總綱，行號 L 對該檔）。本檔所有建議皆〔模型判斷·未裁決〕；「事實」附檔案路徑或行號，「推測」另標。路徑以 `~/GitHub/SunSmell/` 為根，下文省略為 `SunSmell/`。

## 0 異議與前提

1. **字數前提要更正（事實）**：交辦說 `.nani` 約 36 萬字元；實測 `SunSmell/Assets/Scenario/*.nani` 共 306,718 **位元組**（UTF-8），**120,506 字元**、9,891 行，37 個節檔（CH00–CH15，不含 Title）。`Assets/Story/Source/` 草稿另有 190,043 字元（比定稿長，Part3 的章號與 `.nani` 不同：Source 的 CH11–CH13 對應 `.nani` 的 CH11–CH15）。第 4 節的估算以 12 萬字元為準。
2. **SunSmell 是壓力測試，不是代表性案例（事實＋判斷）**：`grep @choice` 零命中＝零選項；單一結局（`Planning/NewOutlineCombinedV2.md`「溫暖的結局」）；第一人稱主角敘事，主角內心獨白 294 行（`Seigo: <color=lightblue>…`）；Naninovel 指令只用到 `@stop/@back/@hideAll/@char/@goto/@wait`，`@back` 全書 3 次、`@char` 1 次、沒有 BGM／音效／轉場——演出層幾乎沒做，「未完成」指的是演出與美術，文字已寫完（`Planning/ChapterBreakdown_Part3.md`「全書撰寫完成」）。這種「全文已定稿、線性、單結局」的作品，最貼切的發行形式其實是「文字照播＋美術即時生成」，不是 AI 即興演。拿它當初始 VN 卡範例仍有價值：它逼出總綱沒想到的「有作者的主角」「體型差」「細表情」「伏筆表」「只暗示不揭露的秘密」，但不能用它推論「一般 VN 卡長怎樣」。
3. **建議多一條路，不擋本案**：全文 12 萬字元塞進 PNG tEXt 技術上可行（實測 306,718 位元組 UTF-8、base64 後約 410 KB〔推測：未實測 ST 讀大卡的表現〕），所以「卡只帶文字」並不排除「全文照播、AI 只生美術」這種玩法。它和本案的「AI 照聖經即興演」是兩個產品：前者是預寫劇本播放器（近似 §14 重播模式但資產即時生），後者是本案。建議本案只做後者，前者列為後續選項（第 4 節選項 D）。
4. **「PNG 帶不了美術」的釐清（事實）**：PNG tEXt 能塞任意 base64，技術上能帶圖；CCv3 規格另有 `.charx`（zip）可帶資產。成立的是分發慣例與體積——ST 生態的通行單位就是「一張圖＋嵌入 JSON」。作者的約束照收：初始 VN 卡＝封面圖＋文字，不帶立繪、背景、UI。
5. **成人內容（事實）**：類型標明「BL／雄孕」（`Bible/Settings.md` 開頭），劇情含性場景（CH06-2 獻身、CH11-2c 洞穴結合，見 `Planning/ChapterBreakdown.md`）與真相揭露後的父子關係。後果：卡需要分級欄位；生圖供應商對成人或擦邊內容可能拒絕〔推測：依各家政策，未實測〕，立繪與背景管線要有「被拒→退路鏈」的處理。本報告的示範卡只寫到大綱層級，不寫露骨內容。
6. **主線對「VN 卡 vs 作品包」的區分我同意**：VN 卡＝劇本源（玩之前、要模型、美術現生）；作品包（總綱 §14 L192–200）＝演出成品（玩之後、不要模型、帶美術）。兩者不共用格式。

## 1 相符性對照表

「覆蓋」＝總綱已能處理；「缺」＝SunSmell 有、總綱沒想到；「多餘」＝對這類作品不適用。

| 總綱 | SunSmell 的事實 | 判定 | 說明 |
|---|---|---|---|
| §1 L11–12 入口與匯入選桌型 | 作品要以 VN 方式玩 | 缺 | 匯入時沒有「這張就是 VN 卡」的分支；VN 卡應自動選 VN 桌、跳過聖經生成（第 5 節 #1） |
| §2 L20 GM＋角色線、GM 不替角色說話 | 主角誠吾 764 行台詞＋294 行獨白（grep 統計），全書第一人稱 | 缺 | 總綱 L55 主角＝玩家卡／只填名字／無名，沒有「作者設定好的主角」；純選項制下主角台詞與獨白只能由 GM 線寫，GM 要拿主角全檔（第 5 節 #3） |
| §3 L28–37 劇本格式 | 旁白＋對白為主；獨白以顏色區分；演出指令幾乎沒用 | 大致覆蓋，缺獨白格 | 對白格、`[bg]` 夠用；第一人稱內心獨白沒有格型（第 5 節 #4） |
| §3 L29 表情枚舉 5 種（拍板 7） | `Assets/Characters/Lak/` 16 張（Alarm、Guarded、Guilt、Possessed、Prayer、Protective…）、`Seigo/` 18 張 | 缺 | 這部的情緒戲靠細表情（迴避、內疚、警戒），5 種會明顯降級；`Possessed`、`Prayer` 其實是姿態，對應總綱的變體（L30） |
| §4 L56 精靈 ② 聖經由 AI 生成 | 作者有完整聖經：`Bible/WorldBible.md`（16,835 字）、`Settings.md`、大綱、節拍表 | 缺 | 要有「聖經由卡提供」的起點；AI 生成只當補空 |
| §4 L60 匯入 ST 卡 → AI 讀卡生聖經 | — | 覆蓋（對普通卡） | VN 卡走另一條（卡內已有聖經） |
| §4 L62 聖經放 system 前綴、章邊界才生效 | 每章場景與伏筆差異大 | 覆蓋且有用 | 正好支撐「每章換入當章節拍」（第 4 節），不是新規則 |
| §5 L66–71 三層秘密 | 拉克的身世、和也的真相、主角懷孕 | 大致覆蓋 | 「主角自己不知道的真相」放第三層（GM-only）即可 |
| §5 L75 揭露時機（旗標或章數） | 拉克「不老、古神後裔」寫明**不得在故事中明說**、只能暗示（`Bible/WorldBible.md`〈拉克·桑加【隱藏設定】〉） | 缺 | 需要 `reveal: never`（只暗示、永不揭露）；另需伏筆「埋設章」（`Planning/TrackingSheet.md` 整份就是埋設／回收表） |
| §6 L83 背景＝地點 slug、同地點重用 | `References/BackgroundStyle/MinimalBackgroundList.md`：8 主景＋6 光線變體＋4 選配，明定「光線變體優先於新背景」 | 覆蓋地點，缺時段 | `[bg: 地點 \| 時間]` 有時間欄（L32），但背景庫鍵只有地點 slug（L83）；同地點早／晚／夜要分圖、且以基底圖改光線保持一致（第 5 節 #7） |
| §6 背景畫風 | `BackgroundStyleGuide.md`：粗水彩＋厚塗、非焦點刻意粗略、道具 3–5 件、拉克空間要誇大尺度 | 缺 | 卡要能帶背景風格指引與每地點的尺度備註 |
| §7 旗標表 | 無旗標（線性） | 多餘（部分） | 全線性作品不需要分岔旗標；但「線索已取得」類旗標可拿來當章結束條件（示範卡用了） |
| §8 L100–108 章、上限 M、結局表、`default` | 16 章（CH00–CH15）、單結局 | 覆蓋章；結局表多餘 | 章上限應由卡給定並可鎖定，不是模型推薦（L102）；結局表只有作者那一條＋引擎 `default` |
| §8 L107 結局一覽、新周目 | 單結局 | 多餘 | 對單結局作品無意義，但不礙事 |
| §3 L35／L47 選項與「換一組」 | 零選項 | 需重定義 | 線性作品的選項只能改「怎麼走」不改「走到哪」：語氣、節奏、好感，不分岔主線（第 5 節 #9） |
| §9 L113–114 立繪七分身、底部對齊統一比例 | 拉克 350 cm、誠吾 190 cm，體型差是主題（`Settings.md`；`BackgroundStyleGuide.md` Scale Notes） | 缺（關鍵） | 統一比例會抹掉主題；卡要帶角色身高，舞台照比例縮放（第 5 節 #6） |
| §9 L124 上下場 | 和也已過世、只在 CH00 回憶出場（`Kazuya:` 4 行）；赤山王、嶺鬼只在夢境與洞穴顯現（`Aka:` 26、`Lingui:` 34 行） | 缺 | 角色要有「回憶限定」「神祇」這類出場性質，避免 GM 讓死者在現實場景上場 |
| §9 L131 標題畫面 | `Story/TitleScreen/main_menu_title_v1.png`（1672×940，2.5 MB） | 覆蓋 | 這張就是 VN 卡的封面候選 |
| §14 作品包 | — | 不適用 | 作品包是玩後成品；VN 卡是玩前源頭，見 §0-6 |
| 總綱沒有的 | 文風規則（`Style/StyleCore.md`、`AntiAI_StyleGuide.md`）、伏筆表、語言處理規則（島民話一律稱「方言」）、獸人描寫準則（`WorldBible.md` 開頭） | 缺 | 卡要有「文風與寫作禁忌」欄，進 GM 與角色線的穩定前綴 |
| 音樂、音效 | 全書沒有 BGM／音效指令 | 不影響 | 與總綱 v1 不做音訊一致 |
| 分級 | 成人內容 | 缺 | 卡要有分級與警示（第 5 節 #12） |

## 2 VN 卡 schema

### 2.1 相容策略

- **容器**：PNG＋tEXt `chara`（V2，base64 JSON），選配同內容的 `ccv3`。理由：現有匯入就是讀這兩個（`src-tauri/src/import/card_io.rs` L26–35：優先 `chara`、沒有才 `ccv3`）。也接受同內容的 `.json`（現有匯入本來就收）。
- **ST 端**：標準欄位填成一張可在 ST 開的「劇情卡」（`name`＝作品名，常見的 scenario card 慣例），ST 開起來是自由 RP。VN 專屬內容全放 `data.extensions.table_tavern.vn`；V2 規範要求應用保留不認得的 extensions（規範事實；ST 實際重存是否保留未實測）。
- **我們端（事實）**：`parse_character`（`card.rs` L246–272）只要求 `name`，不驗 `spec`、不拒未知鍵，所以帶 extension 的卡在現有聊天桌匯入路徑不會壞；世界書條目已認 `extensions.table_tavern.visibility`（`src-tauri/src/data/worldbook.rs` L119–140：`"public"`、`{"characters":[...]}`，其他＝GM）——VN 卡沿用同一命名空間 `table_tavern`。
- **偵測**：有 `data.extensions.table_tavern.vn` 且 `format == "tt-vn-card"` → 匯入走 VN 卡路徑（建 N 個角色、寫 `vn.json`、不建「作品名」那個角色）；沒有 → 照普通卡。
- **劇透**：卡是明文，ST 使用者打開就看得到私設與聖經；與總綱 L74「君子鎖」同一立場，不加密。

### 2.2 ST 標準欄位怎麼填

| 欄位 | 填什麼 | 給誰用 |
|---|---|---|
| `spec`／`spec_version` | `chara_card_v2`／`2.0` | 兩邊 |
| `name` | 作品名 | 兩邊（我們：VN 桌名） |
| `description` | 世界概要＋登場角色公開設定（純文字） | ST |
| `scenario` | 主角處境一段 | ST |
| `first_mes` | 開場旁白的純散文版（去掉 `[bg]`、`[choices]` 等標記） | ST；我們聊天桌 |
| `creator_notes` | 分級、警示、「在 Table Tavern 以 VN 模式遊玩」說明 | 兩邊 |
| `tags` | 題材標籤 | 兩邊 |
| `character_book.entries` | 世界公開條目（地點、信仰、道具），`extensions.table_tavern.visibility: "public"`；角色私設條目 `{"characters":[id]}` | 兩邊（ST 照關鍵字觸發；我們照可見性） |
| `personality`／`mes_example`／`system_prompt` | 留空 | — |

聖經（章節拍、結局、伏筆）**不進** `character_book`：ST 的關鍵字觸發會把它當常駐或隨機注入，劇透又灌爆上下文。

### 2.3 `data.extensions.table_tavern.vn`

可見層：**P**＝公開（玩家、GM、所有角色線）、**C**＝該角色線＋GM、**G**＝只有 GM 線、**I**＝只給生圖管線，不進任何文字模型的提示詞。

| 路徑 | 型別 | 必填 | 層 | 說明 |
|---|---|---|---|---|
| `format` | `"tt-vn-card"` | 是 | — | 偵測用 |
| `version` | 整數 | 是 | — | 格式版本，不認得就拒收 VN 路徑、退普通卡 |
| `lang` | BCP-47 | 是 | — | 卡文字語系 |
| `rating` | `{level: "general"\|"teen"\|"adult", warnings: [字串]}` | 是 | — | 匯入時提示；生圖被拒時給玩家看原因 |
| `branching` | `"none"\|"light"\|"full"` | 是 | G | none＝主線固定、選項只改語氣與好感；light＝選項可改支線與結局細節；full＝結局分岔 |
| `pov` | `{narration: "first"\|"second"\|"third"}` | 是 | G | 旁白人稱 |
| `protagonist` | `{mode: "fixed"\|"player_card"\|"named", id?: 角色 id}` | 是 | G | fixed＝卡內角色當主角（玩家不能換）；player_card＝玩家卡；named＝只取名字 |
| `world.summary` | 字串 ≤1500 字 | 是 | P | 世界一段話，常駐 |
| `style.writing` | 字串 ≤1500 字 | 否 | P | 文風、句法禁忌、描寫準則；GM 與角色線都帶 |
| `style.glossary` | `[{term, rule}]` | 否 | P | 用語規則（如「方言」） |
| `cast[]` | 陣列，1–8 人 | 是 | — | 角色 |
| `cast[].id` | slug | 是 | — | 劇本格與 `Visibility::Characters` 用 |
| `cast[].name`／`aliases` | 字串／陣列 | 是／否 | P | |
| `cast[].role` | `"protagonist"\|"main"\|"support"\|"deity"\|"memory"` | 是 | G | memory＝只在回憶出場；deity＝只在指定場合顯現 |
| `cast[].appears` | 字串 ≤200 字 | 否 | G | 出場限制的白話說明（如「只在夢境與洞穴」） |
| `cast[].color`／`emoji` | 色碼／字元 | 否 | P | 名牌 |
| `cast[].height_cm` | 數字 | 否 | P | 舞台縮放（第 5 節 #6） |
| `cast[].public` | 字串 ≤1500 字 | 是 | P | 對應 `public_md` |
| `cast[].private` | 字串 ≤2000 字 | 否 | C | 對應 `private_md`；protagonist 的私設給 GM 線（主角沒有自己的角色線） |
| `cast[].expressions_extra` | `[{key, label, prompt}]` ≤6 | 否 | P／I | 五枚舉之外的表情鍵（第 5 節 #5，需作者裁決） |
| `cast[].variants` | `[{key, label, prompt}]` ≤4 | 否 | P／I | 對應總綱 L30 `@變體` |
| `cast[].appearance` | `{spec: 英文規格塊 ≤600 字元, notes: 字串}` | 是（有立繪者） | I | 生圖用外觀規格（總綱 tasks 拍板 8 的「外觀規格塊」由作者直接給） |
| `locations[]` | 陣列 ≤20 | 否 | P／I | 背景枚舉清單 |
| `locations[].slug`／`name` | slug／字串 | 是 | P | `[bg:]` 只認這些 |
| `locations[].times` | `["day","dusk","night",…]` | 否 | P | 允許的時段變體 |
| `locations[].prompt` | 英文 ≤500 字元 | 是 | I | 背景生圖提示 |
| `locations[].scale` | 字串 ≤200 | 否 | I | 尺度備註（巨型家具等） |
| `art.style` | 英文 ≤600 字元 | 是 | I | 全作畫風 |
| `art.background_rules` | 英文 ≤800 字元 | 否 | I | 背景細節度規則 |
| `art.negative` | 英文 ≤300 字元 | 否 | I | 避免項 |
| `art.cover_as_style_ref` | 布林 | 否 | I | 封面當生圖畫風參考圖（第 5 節 #11） |
| `opening` | 字串 ≤2000 字 | 是 | P | 第一回合旁白（寫成劇本格） |
| `bible.logline` | 字串 ≤300 字 | 是 | G | 主線一句 |
| `bible.chapter_max` | 整數 3–30 | 是 | G | 章數 |
| `bible.chapter_lock` | 布林 | 否 | G | true＝玩家不能改章數 |
| `bible.chapters[]` | 陣列，長度＝`chapter_max` | 是 | G | 每章一筆 |
| `…chapters[].title` | 字串 | 是 | P（章字卡顯示） | |
| `…chapters[].goal` | 字串 ≤300 字 | 是 | G | 本章要達成的劇情 |
| `…chapters[].beats` | `[字串]` ≤8、每條 ≤200 字 | 是 | G | 節拍（照序但不逐句） |
| `…chapters[].end_when` | 字串 ≤200 字 | 是 | G | 結束條件（GM 判斷後寫 `[chapter]`） |
| `…chapters[].plant`／`payoff` | `[秘密或伏筆 id]` | 否 | G | 本章要埋／要收的 |
| `…chapters[].anchors` | `[字串]` ≤6、每句 ≤80 字 | 否 | G | 錨句：作者要原樣出現的台詞或意象 |
| `…chapters[].locations`／`cast` | `[slug]`／`[id]` | 否 | G | 本章預期場景與登場者（可拿來預生背景） |
| `bible.secrets[]` | `[{id, holder, truth, reveal: {chapter?, flag?, never?}, hint}]` | 否 | G | `holder` 可為 null（世界真相）；是角色 id 時另寫進該角色私設；`never`＝只暗示 |
| `bible.flags[]` | `[{id, type, init, min?, max?, desc, trigger}]` | 否 | G | 照總綱 §7 |
| `bible.endings[]` | `[{id, title, when: [條件], summary}]` | 是（≥1） | G | 條件型別照總綱 L104；`default` 引擎自建；`branching: none` 時 `when` 必含章門檻 `chapter >= chapter_max`（缺就匯入時補） |

字數上限見第 4 節；「字」＝字元，所有上限匯入時夾斷並提示，不拒收整張卡〔模型判斷·未裁決〕。

## 3 SunSmell 示範卡全文

附錄 A。只到大綱層級，不貼原文、不寫露骨內容。

## 4 全文怎麼對待與文字量上限

### 4.1 四個選項

| | 做法 | 後果 |
|---|---|---|
| A | 濃縮成聖經：每章目標、節拍、結束條件、伏筆、錨句；對白由 AI 寫 | 選項有意義、AI 有發揮空間；作者的字句只保住錨句 |
| B | 全文帶進卡、整份當上下文 | 12 萬字元約 12–18 萬 token〔推測：中文每字約 1–1.5 token，依模型而異〕，每回合重送；常駐前綴爆量，且模型會照抄，選項失去意義 |
| C | 全文帶進卡、只注入當章原文 | 一節原文平均約 3,300 字元（120,506 字元 ÷ 37 節；最短 `CH15_1` 4,596 位元組、最長 `CH07_2` 16,895 位元組，中文約 3 位元組一字），一章 1–4 節，第一章四節約 1 萬字元；上下文負擔可接受，但模型傾向照抄當章原文，玩家選項與原文衝突時劇情會「拉回原文」，等於假選項 |
| D | 全文當劇本照播，AI 只生美術（不同產品） | 作者字句 100% 保留、零文字模型額度；但要另寫 `.nani`→劇本格轉換、沒有選項——這是「預寫劇本播放器」，不是本案 |

### 4.2 建議

**選 A，加分章注入**〔模型判斷·未裁決〕：

- **常駐（GM 線 system 穩定前綴）**：世界概要、文風、角色公開設定、主角檔（protagonist 的 public＋private）、聖經骨架（每章一行標題＋目標）、秘密表、旗標表、結局表。
- **當章包（章邊界換入）**：當章節拍、結束條件、要埋／收的伏筆、錨句、預期場景。章邊界本來就換 system（總綱 L62、L87 的快取紅線），所以不新增規則。
- **角色線**：該角色公開＋私設＋文風；不帶聖經（總綱 L72 硬邊界）。
- **生圖欄位（I 層）**：只進生圖管線，不進文字模型。
- 不帶全文。理由：純選項制下帶全文（B、C）等於不需要模型即興，又要付即興的成本；要保作者字句就走 D。

### 4.3 文字量上限

推導〔推測〕：中文每字約 1–1.5 token。現有 GM system 已帶世界書、角色公開設定與導演指示，VN 卡再加的常駐量不宜超過一般 ST 卡常駐量的數倍；一般角色卡的 `description` 多在 1,000–4,000 字（未抽樣統計）。SunSmell 的作者設定原稿（WorldBible 16,835＋WorldBible_Part3 4,866＋Settings 3,790＋大綱 4,867，共約 30,000 字）全塞常駐就太多，這就是要分層的理由。

| 部分 | 建議上限 |
|---|---|
| 常駐（GM 線 VN 部分合計） | 12,000 字 |
| 當章包 | 4,000 字／章 |
| 單一角色私設 | 2,000 字 |
| 生圖欄位（I 層，合計） | 不計入常駐；整卡上限內 |
| **整張卡 `vn` 擴充** | **建議 ≤50,000 字，硬上限 100,000 字**（約 300 KB UTF-8、base64 後約 400 KB〔推測〕） |

SunSmell 示範卡估計（附錄 A 寫滿 16 章時）：常駐約 9,000 字、當章包 1,000–1,500 字／章、整卡約 35,000–45,000 字〔推測〕，在建議上限內。

## 5 對總綱的改動建議（逐條，全部〔模型判斷·未裁決〕）

其中 #5（觸及拍板 7 表情枚舉）與 #10 的章數鎖（觸及拍板 6 玩家可改）要先問作者，其餘不牴觸既有拍板。

1. **§1 L12**：匯入檔帶 `table_tavern.vn` → 不問桌型、直接 VN 桌，精靈跳過聖經生成、從第 ③ 步立繪開始。理由：卡已經說了它是 VN。
2. **§4 L55–56**：第 ① 步多第三種起點「VN 卡」；第 ② 步顯示卡內聖經（照劇透鎖遮罩），「重骰聖經」對 VN 卡改成「恢復卡內原版」。理由：作者寫好的聖經不該被模型覆蓋。
3. **§2 L20、§4 L55**：新增「卡定主角」（`protagonist.mode: fixed`）：主角不開角色線，主角的台詞、獨白由 GM 線寫，GM 帶主角全檔；選項文字＝主角這一拍的行動或台詞。理由：SunSmell 主角台詞量是全書最多（764 行），不能空著。
4. **§3 L29**：加內心獨白格（如 `名字 (thought): 台詞`，播放器以不同字色顯示）。理由：SunSmell 有 294 行獨白，用顏色區分是 VN 慣例。
5. **§3 L29、拍板 7（需作者裁決，觸及〔作者裁決 2026-08-07〕）**：卡可宣告額外表情鍵（每角色 ≤6），清單外仍退 neutral；沒宣告的卡維持五枚舉。理由：細表情是這類作品的情緒載體；額外鍵也走臉層（總綱 L118），每多一鍵只多一張臉層。
6. **§9 L114**：舞台高度照 `height_cm` 比例縮放（以最高者為滿版、下緣對齊），沒填的照現行統一比例。理由：體型差是 SunSmell 的主題，統一比例會抹掉。
7. **§6 L83**：背景庫鍵改為 `地點slug＋時段`；同地點的新時段以既有基底圖做光線編輯生成（不是從零生）；卡內 `locations` 當 `[bg:]` 的枚舉清單。理由：MinimalBackgroundList 明定「光線變體優先於新背景」，從零生會換掉構圖。
8. **§5 L75**：揭露時機多一種 `never`（只暗示、永不明說）；章多 `plant`／`payoff` 欄、秘密多 `hint` 規則。理由：拉克的不老設定明訂不得明說；TrackingSheet 整份是埋設／回收表。
9. **§3 L35、§7、§8**：聖經加 `branching`；`none` 時 GM 導演指示改為「選項只改語氣、節奏、好感，不改本章目標」，結局表可只有一條作者結局（加引擎 `default`），且該結局 `when` 必含 `chapter >= chapter_max`（匯入時自動補）——否則 L104 會接受提早寫出的 `[ending]`，後面的章全被跳過。理由：讓線性作品也能用純選項制，而不是假裝有分岔。
10. **§8 L102**：VN 卡的 `chapter_max` 取代模型推薦（與拍板 6 相容）；`chapter_lock: true` 讓玩家不能改**需作者裁決**——牴觸拍板 6「玩家可改」〔作者裁決 2026-11-01，§12 L170〕。理由：作者的章數是設計，不是建議。
11. **§4 第 ③ 步、§6**：VN 卡的封面同時當畫風參考圖（立繪、背景生成都附上），並當標題畫面背景。理由：封面是卡唯一的美術資產，也是作者認可的畫風。
12. **§13**：新增分級欄位的處理：匯入時顯示分級與警示；生圖被供應商拒絕時退剪影＋名牌／字卡，並提示可換供應商。理由：成人卡在 ST 生態很常見，被拒是可預見的正常路徑。
13. **§9 L124**：角色 `role: memory`／`deity` 與 `appears` 說明帶進 GM 導演指示，上場判定照常。理由：避免已故角色在現實場景出現。
14. **§10、§11**：VN 卡匯入放 `src-tauri/src/import/vn_card.rs`，產出 `vn.json`、角色、世界書；併入包 4（聖經）或另開一小包，核心估約 200–300 行〔推測〕。理由：解析是信任邊界輸入，照流程上限要做方案送審。
15. **§13 L183**：「VN 桌匯出成 VN 卡」是創作者的自然路徑（在劇本分頁寫好聖經→匯出卡分享），建議列為 v1.5，而不是「v1 不做 VN 桌匯出」一筆帶過。理由：沒有匯出，VN 卡只能手寫 JSON。

## 6 未涵蓋與沒查的範圍

- **讀了**：`Assets/Story/README.md`、`Bible/Settings.md`、`Bible/SceneGuide.md`、`Bible/WorldBible.md`（前 17,000 位元組，約全檔 40%）、`Bible/WorldBible_Part3.md`（開頭）、`Planning/NewOutlineCombinedV2.md`、`ChapterBreakdown.md`、`ChapterBreakdown_Part3.md`、`TrackingSheet.md`（前 6,000 位元組）、`Style/StyleCore.md`、`References/BackgroundStyle/BackgroundStyleGuide.md`、`MinimalBackgroundList.md`（部分）、`References/CharacterDesign/sprite_expression_workflow.md`（開頭）、`docs/superpowers/specs/2026-05-06-title-menu-design.md`（開頭）；`.nani` 只讀 `CH00_Opening`、`CH01_1_Dock` 開頭，其餘用 grep 統計（指令、說話者、獨白行數）。
- **沒讀**：`Source/` 全部草稿、其餘 35 個 `.nani` 全文、`WriteStyle.md`、`AntiAI_StyleGuide.md`、`StyleExamples.md`、`Part3_WritingPrompt.md`、`WritingPlan.md`、表情 CSV 與提示表、`docs/superpowers/plans/`、所有圖片（只量了封面尺寸）、`Archive/`（照名字不讀）。
- **附錄 A 第 4 章以後**的節拍是從 ChapterBreakdown 與大綱推的，沒有對 `.nani` 原文核對；`.nani` 第 11–15 章與 Source Part3 章號不同，附錄以 `.nani` 為準。
- **沒實測**：ST 讀寫 400 KB 級卡片、ST 重存是否保留 `extensions.table_tavern`、生圖供應商對本作畫風（獸人、肌肉、成人題材）的拒絕率、選項 A 在真模型上的遵從度（節拍是否被跳過、錨句是否原樣出現）、token 換算。
- 附錄結局 `when` 的寫法（`path`／`gte`、`flag`／`is`）是示意，沒對照 `src-tauri/src/mechanism/triggers.rs` 的實際條件格式。
- 說話者 `Bobo`（CH11_1，19 行）與 `Pobo`（12 行）疑為同一角色的拼寫不一（`TrackingSheet.md` 只有「波伯」），未核對原文。

---

## 附錄 A：SunSmell 示範卡（文字內容）

以 JSON 結構分段示意（實際卡為單一 JSON）；`…` 表示同型省略。

### A.1 ST 標準欄位

```json
{
  "spec": "chara_card_v2",
  "spec_version": "2.0",
  "data": {
    "name": "太陽的氣味",
    "description": "虛構亞熱帶島嶼「日嶼」：半地下屋、紅瓦、珊瑚石牆、涼台，人口外流的老島。信仰分裂成兩神：村落大廟的赤山王（捧空劍鞘）與深山洞穴被除名的嶺鬼（插著赤裸神劍）。\n\n登場：林誠吾（紅虎獸人青年，190 cm，來島打工換宿的美術系畢業生）；拉克·桑加（巨大紅虎獸人木匠，超過 350 cm，開朗敦厚）；碼頭遊客中心的兔子獸人老婆婆；送食材的山羊獸人長老波伯；村裡的羊獸人醫生。",
    "scenario": "誠吾帶著過世父親留下的中空木雕來到日嶼，想找尋他認定存在的「紅虎獸人母親」的線索，住進巨人木匠拉克的家，一起修廟。",
    "first_mes": "（vn.opening 去掉 [bg]、[choices] 標記的純散文版）",
    "creator_notes": "分級：成人（BL、雄孕、血緣、性場景、受傷與疾病）。在 Table Tavern 以 VN 模式遊玩：AI 照作者聖經即時演出，立繪與背景即時生成。",
    "tags": ["BL", "furry", "island", "mystery", "slice of life"],
    "character_book": {
      "entries": [
        { "keys": ["赤山王", "大廟"], "content": "村落旁緩坡上的開放式無牆亭子廟，珊瑚礁岩地、紅瓦頂。赤山王神像為深色硬木粗獷雕刻、無漆無金，雙手捧空劍鞘，背面保留原木未雕。", "extensions": { "table_tavern": { "visibility": "public" } } },
        { "keys": ["嶺鬼", "洞穴"], "content": "深山禁地的洞穴小廟，頂端裂縫漏光照在長滿苔蘚的岩石祭壇；祭壇縫隙深插一把赤裸神劍，劍柄刻狂暴猛獸圖騰。島民視為禁忌。", "extensions": { "table_tavern": { "visibility": "public" } } },
        { "keys": ["方言"], "content": "島民之間的當地話一律稱「方言」。", "extensions": { "table_tavern": { "visibility": "public" } } },
        { "keys": ["短劍", "劍鞘", "木雕"], "content": "拉克年輕時雕的一對神木短劍與木鞘：鞘送給了和也，劍自己留著，藏在工具帆布袋最深處。合鞘時『喀』一聲，圖騰拼成完整的島嶼圖景。", "extensions": { "table_tavern": { "visibility": { "characters": ["lak"] } } } }
      ]
    },
    "extensions": { "table_tavern": { "vn": "（見 A.2–A.5）" } }
  }
}
```

### A.2 全域、主角、文風、美術

```json
{
  "format": "tt-vn-card",
  "version": 1,
  "lang": "zh-TW",
  "rating": { "level": "adult", "warnings": ["sexual", "mpreg", "incest-reveal", "injury", "illness"] },
  "branching": "none",
  "pov": { "narration": "first" },
  "protagonist": { "mode": "fixed", "id": "seigo" },
  "world": {
    "summary": "日嶼是亞熱帶虛構小島，融合蘭嶼與石垣島元素但不指向真實地點：半地下屋與赭紅瓦頂、珊瑚石牆、涼台、白砂路、朱槿與蟬鳴。人口外流，政府想靠觀光活化。信仰分成兩神：主流的赤山王（秩序與庇護，大廟神像捧空劍鞘、背面未雕）與被邊緣化的古神嶺鬼（野性與原始能量，深山洞穴祭壇插著神劍）。數十年前大地震後嶺鬼被除名、洞穴封閉。島上近來偶有地震。"
  },
  "style": {
    "writing": "第一人稱，敘事者誠吾。輕柔、明亮、溫馨：寫光線、空氣的氣味與溫度、日常物件。心情用小動作與生理反應暗示，不直說。短句為主但保留完整中文語法。對話遲鈍、溫柔、留白，不要太聰明太直接。禁止：單字動詞或形容詞、「X 的、Y 的、Z 的」三連、「是＋形容詞＋的」翻譯腔、結語套話與重複總結。獸人描寫以獸人生理為主（肉墊、短爪、耳朵、尾巴、毛皮），避免『流汗』等人類特徵，改寫成張口喘息散熱。",
    "glossary": [
      { "term": "方言", "rule": "島民的當地話一律叫方言，不寫成任何真實語言名" },
      { "term": "太陽的氣味", "rule": "木屑清香混著拉克曬暖的體味；全作核心意象，不要隨口濫用" }
    ]
  },
  "art": {
    "style": "Japanese visual novel, anime style, muscular anthropomorphic animal characters (kemono), emphasis on weight and touch rather than prettiness; warm subtropical sunlight.",
    "background_rules": "Rough watercolor with unpolished thick-brush accents, watercolor paper texture, translucent washes, dry brush. Only the story-relevant focal area is clear; non-focal areas loose and under-rendered; at most 3-5 readable props; broad color masses in distant areas. No photorealism, no 3D render lighting, no glossy polish. Lak's spaces exaggerate scale: door height, workbench, timber thickness, tool size.",
    "negative": "photorealistic, 3D render, glossy, cluttered prop wall, text, watermark",
    "cover_as_style_ref": true
  }
}
```

### A.3 角色

```json
[
  {
    "id": "seigo", "name": "林誠吾", "aliases": ["誠吾"], "role": "protagonist", "color": "#C8502D", "height_cm": 190,
    "public": "紅色老虎獸人青年，黑眼珠、粉紅鼻子，喉前到腹部與大腿內側是米黃色毛；壯但不巨大。美術系畢業，擅長小模型、圖樣與細部雕刻，帶著父親留下的舊雕刻刀組。來自都市，有點自卑、細膩，也有自尊與主動。由人類男性林和也撫養長大，叫他父親；相信自己的紅虎血統來自一位沒見過的紅虎母親。來日嶼打工換宿當職人助手，順便找母親的線索。",
    "private": "（主角私設給 GM 線）誠吾自己不知道：和也其實是他的生母（具雄孕能力的人類男性），拉克是他的生父；他繼承了雄孕體質，第 7 章起出現反胃與嗜酸。這些在揭露前不得由任何角色說破。",
    "appearance": { "spec": "red tiger anthropomorphic young man, 190cm, sturdy muscular build (not huge), black eyes, pink nose, cream fur on throat, chest, belly and inner thighs, black tank top, shorts, casual modern style", "notes": "立繪七分身到小腿（總綱 L113）" },
    "expressions_extra": [
      { "key": "flustered", "label": "慌張臉紅", "prompt": "flustered, blushing, ears flattened" },
      { "key": "worried", "label": "擔心", "prompt": "worried, brows knitted" },
      { "key": "pained", "label": "痛苦", "prompt": "in pain, teeth clenched" }
    ]
  },
  {
    "id": "lak", "name": "拉克·桑加", "aliases": ["拉克", "老爸"], "role": "main", "color": "#A8321E", "height_cm": 355,
    "public": "巨大紅虎獸人，身高超過 350 公分，hyper muscle，胸腹米黃色毛。島上的木匠，造祭海木舟、修廟。溫柔敦厚、有點呆萌的大暖男，工作時帶著木屑與曬暖的體味。穿舊式工作服：單肩織帶上衣、寬腰布，邊緣磨出流蘇。說話直、句子短。",
    "private": "古神嶺鬼的後裔，外表壯年、實際年紀遠超常人，見證過島上數代興衰——絕不說出年齡或『不老』，只在回憶『以前』時流露不尋常的時間跨度。長年獨自替嶺鬼承接、消化地底壓力；壓力累積時會像野獸般喘息、性慾難抑，他以此為恥、拚命壓抑。年輕時與人類男性林和也相戀，雕了短劍與木鞘，把鞘送給和也、自己留劍，劍至今藏在工具帆布袋最深處；不知道和也有雄孕體質，也不知道有孩子。和也離島後他在碼頭站到天黑。聽到『林』這個姓會眉頭一皺。",
    "variants": [
      { "key": "festival", "label": "祭典裝", "prompt": "very short deep red loincloth with tassels, bare upper body, necklace of dark agate beads and white shells" },
      { "key": "possessed", "label": "附身", "prompt": "possessed by an ancient god, glowing eyes, feral posture" }
    ],
    "expressions_extra": [
      { "key": "guarded", "label": "迴避警戒", "prompt": "averting gaze, ears twitching back, guarded" },
      { "key": "guilt", "label": "內疚", "prompt": "guilty, looking down" },
      { "key": "tender", "label": "溫柔", "prompt": "tender soft smile" }
    ],
    "appearance": { "spec": "gigantic red tiger anthropomorphic man, over 350cm, hyper muscular, cream fur on chest and belly, one-shoulder woven strap top exposing shoulder and chest, wide loincloth with worn fringe, warm good-natured face", "notes": "與誠吾同框時頭身比必須明顯更大" }
  },
  {
    "id": "grandma", "name": "老婆婆", "role": "support", "height_cm": 130,
    "public": "碼頭遊客中心櫃檯的兔子獸人老婆婆，身形嬌小、背微駝，長耳朵耳尖已白。和藹，說話慢，會帶芭樂與醃梅子來探望人。",
    "private": "記得二十多年前的和也：『很安靜的小伙子，手很巧』，和也在時拉克很開心；和也是『好像身體不舒服，回本島看醫生』離開的。第一眼就從味道認出誠吾是和也的孩子。知道嶺鬼『不是壞東西』，趕走之後地震反而變多。知道古神的後裔是拉克，紅虎從以前到現在只有拉克一個。",
    "appearance": { "spec": "small elderly rabbit anthropomorphic woman, slightly hunched, long drooping ears with white tips, simple island clothes" }
  },
  {
    "id": "pobo", "name": "波伯", "role": "support", "height_cm": 165,
    "public": "年邁的山羊獸人，定期送魚和菜給拉克。話少、口氣硬但心軟。",
    "private": "對拉克有超越鄰居的敬意（知道他不是普通人，但不說）。對紅虎女性『沒有印象』。",
    "appearance": { "spec": "elderly goat anthropomorphic man, island elder, weathered, carrying a basket of fish and vegetables" }
  },
  {
    "id": "kazuya", "name": "林和也", "role": "memory", "appears": "只在誠吾的回憶與夢中出現；現實時間已過世。",
    "public": "人類男性，安靜、柔和、纖細。誠吾的『父親』，常一個人坐在窗邊。",
    "appearance": { "spec": "slender gentle human man, quiet expression, soft afternoon light" }
  },
  {
    "id": "aka", "name": "赤山王", "role": "deity", "appears": "只在夢境（銀藍空間）中顯現。",
    "public": "日嶼主神，象徵秩序與庇護。"
  },
  {
    "id": "lingui", "name": "嶺鬼", "role": "deity", "appears": "只在洞穴祭壇顯現；說話一字一句、斷續。",
    "public": "被除名的古神，象徵野性與原始能量。"
  }
]
```

### A.4 地點

```json
[
  { "slug": "ferry", "name": "渡輪船艙", "times": ["afternoon"], "prompt": "high-speed ferry cabin interior, plastic seats, window view of bright open sea" },
  { "slug": "dock", "name": "日嶼碼頭", "times": ["noon", "dusk"], "prompt": "small old concrete pier on a subtropical island with patched cracks, rusty bollards, a shabby white visitor center at the edge, glaring noon sea", "scale": "" },
  { "slug": "visitor-center", "name": "遊客中心", "times": ["day"], "prompt": "dim small visitor center interior, faded tourism posters, wooden counter, mugwort sachet in the corner" },
  { "slug": "village", "name": "村落白砂路", "times": ["noon", "night"], "prompt": "white coral-sand lane between chest-high coral-stone walls, half-sunken houses with red tile roofs, hibiscus, banana trees" },
  { "slug": "workshop", "name": "拉克的工房", "times": ["day", "dusk"], "prompt": "half-open-air woodworking yard by the coast, piled timber, giant workbench and huge tools, oversized doorway", "scale": "everything sized for a 350cm giant" },
  { "slug": "lak-house", "name": "拉克的家", "times": ["morning", "day", "night"], "prompt": "enormous high-ceilinged wooden house interior, thick central pillar, giant table and chairs, normal-sized single bed by the window in the guest room", "scale": "giant furniture, one human-sized guest room" },
  { "slug": "veranda", "name": "涼台", "times": ["dusk", "night"], "prompt": "open wooden raised platform with thatched roof beside the house, sunset over the sea" },
  { "slug": "temple", "name": "赤山王大廟", "times": ["day", "dusk"], "prompt": "open wall-less shrine pavilion on four thick pillars, red tile roof, coral rock floor, dark rough-carved wooden god statue holding an empty scabbard" },
  { "slug": "beach-festival", "name": "海灘祭典", "times": ["night"], "prompt": "beach bonfire festival night, islanders dancing, small red-black-white ritual boats" },
  { "slug": "clinic", "name": "診所", "times": ["day"], "prompt": "tiny old single-storey clinic with vines on the walls, old equipment" },
  { "slug": "cave", "name": "嶺鬼洞穴祭壇", "times": ["moonlight", "day", "crisis", "calm"], "prompt": "hidden cave chamber, light beam from a crack in the ceiling onto a huge mossy rock altar with a bare sword thrust into it", "scale": "" },
  { "slug": "dream", "name": "夢境", "times": ["silver-blue"], "prompt": "abstract silver-blue dream space with temple and cave motifs" }
]
```

### A.5 聖經

```json
{
  "logline": "以為自己有個紅虎母親的青年來到日嶼，在巨人木匠家打工換宿，一步步拼出自己的身世，最後選擇留在島上與兩神、與拉克一起生活。",
  "chapter_max": 16,
  "chapter_lock": true,
  "secrets": [
    { "id": "lak-ageless", "holder": "lak", "truth": "拉克是嶺鬼後裔、不老", "reveal": { "never": true }, "hint": "只透過『以前』的時間跨度、島民的敬意、偶發的古老疲憊暗示" },
    { "id": "sword", "holder": "lak", "truth": "拉克藏著與誠吾木鞘成對的短劍", "reveal": { "chapter": 10, "flag": "clue.sword" } },
    { "id": "lingui-pressure", "holder": "lak", "truth": "拉克替嶺鬼承接地底壓力，壓力大時性慾失控", "reveal": { "chapter": 9 } },
    { "id": "kazuya-mother", "holder": null, "truth": "和也是誠吾的生母（雄孕），拉克是生父", "reveal": { "chapter": 10, "flag": "clue.truth" } },
    { "id": "seigo-pregnant", "holder": null, "truth": "誠吾懷孕了", "reveal": { "chapter": 10, "flag": "clue.truth" } },
    { "id": "sun-smell", "holder": null, "truth": "『太陽的氣味』是木頭混拉克的體味", "reveal": { "chapter": 3 } }
  ],
  "flags": [
    { "id": "closeness", "type": "count", "init": 0, "min": 0, "max": 10, "desc": "誠吾與拉克的親近度", "trigger": "互相照顧、坦白、身體接觸的溫情時刻 +1；冒犯或疏遠 -1" },
    { "id": "clue.notes", "type": "bool", "init": false, "desc": "讀過和也的嶺鬼紀錄", "trigger": "誠吾在洞穴找到並讀完文獻" },
    { "id": "clue.grandma", "type": "bool", "init": false, "desc": "聽老婆婆說和也因身體不適離島", "trigger": "老婆婆說出這句" },
    { "id": "clue.sword", "type": "bool", "init": false, "desc": "短劍合鞘", "trigger": "誠吾把短劍插進木鞘" },
    { "id": "clue.truth", "type": "bool", "init": false, "desc": "誠吾想通身世與懷孕", "trigger": "合鞘後誠吾串起所有線索" }
  ],
  "endings": [
    { "id": "warm", "title": "太陽的氣味", "when": [{ "path": "chapter", "gte": 16 }, { "flag": "clue.truth", "is": true }], "summary": "誠吾告白、拉克回應，兩人決定在日嶼一起生活，修復建築、傳承雙面神的傳說。" }
  ],
  "chapters": [
    {
      "n": 1, "title": "序章：海風捎來的木屑香",
      "goal": "交代誠吾的動機與木雕，抵達日嶼。",
      "beats": [
        "渡輪船艙：冷氣、引擎嗡嗡、窗外大海；誠吾大腿上放著舊棉布包的中空木雕。",
        "回憶：父親和也坐在窗邊，說木雕是從日嶼帶回來的，『那裡的木頭聞起來有太陽的味道』。",
        "誠吾的認知：自己是紅虎、父親是人類，所以血統一定來自紅虎母親；父親總說『等你長大再說』，卻沒等到那天。",
        "畢業後的空虛；查日嶼時看到修廟與木工的打工換宿招募，決定去。",
        "汽笛、跳板『哐噹』，推開艙門，濕熱海風撲面。"
      ],
      "end_when": "誠吾踏上碼頭。",
      "plant": ["kazuya-mother", "sun-smell"],
      "anchors": ["那裡的木頭⋯⋯聞起來有太陽的味道。", "等你長大了⋯⋯再說吧。"],
      "locations": ["ferry", "dock"], "cast": ["seigo", "kazuya"]
    },
    {
      "n": 2, "title": "第一章：日嶼的陽光與巨人的影子",
      "goal": "在島上落腳：遇到老婆婆、走過村落、見到拉克、入住巨人之家、第一次打聽紅虎母親。",
      "beats": [
        "正午碼頭只剩誠吾一人；遊客中心裡兔子老婆婆指路去找拉克（她多看了誠吾一眼）。",
        "白砂路、珊瑚石牆、半地下屋、涼台、蟬鳴；走到海邊的工房。",
        "拉克初登場：巨大的身影、木屑與曬暖體味——誠吾第一次聞到『太陽的氣味』卻說不出來源。拉克聽到『林』姓時眉頭一皺、欲言又止。",
        "巨人之家：門框兩倍高、宗柱、到胸口的桌子；客房卻有正常尺寸的單人床與書桌（誠吾不知道是誰留下的）。",
        "傍晚波伯送魚菜，對拉克恭敬得不像鄰居；誠吾打聽紅虎女性，波伯『沒有印象』，拉克說『從以前到現在就只有我一個』。誠吾決定來日方長。",
        "涼台入夜，海風與星空。"
      ],
      "end_when": "誠吾在涼台或客房結束第一天。",
      "plant": ["kazuya-mother", "lak-ageless"],
      "anchors": ["從以前到現在就只有我一個。"],
      "locations": ["dock", "visitor-center", "village", "workshop", "lak-house", "veranda"],
      "cast": ["seigo", "grandma", "lak", "pobo"]
    },
    {
      "n": 3, "title": "第二章：廟宇與日常",
      "goal": "開始修廟；看見捧著空劍鞘、背面未雕的赤山王；兩人工作默契成形，誠吾悟出太陽的氣味。",
      "beats": [
        "第一天上工：繞過珊瑚石牆到大廟，開放式亭子、線香與芬多精。",
        "赤山王神像捧著空劍鞘，紋路和誠吾的木雕一模一樣；神像背面是粗糙未雕的原木。拉克說『劍不知道什麼時候不見了』。",
        "分工：拉克做大型木工、誠吾做細部雕刻；體型差帶來的日常小插曲（工具、高度、遞東西）。",
        "誠吾偷偷觀察拉克的身體，又為自己的視線慌張。",
        "湊近看木紋時聞到木屑香混著拉克曬暖的體味，誠吾想通父親說的『太陽的味道』。",
        "收工時拉克手探進帆布袋最深處碰到什麼，動作頓住、耳朵往後壓，沒拿出來；誠吾只用餘光瞥到。"
      ],
      "end_when": "工作日常告一段落，誠吾意識到自己在意拉克。",
      "plant": ["sword"],
      "payoff": ["sun-smell"],
      "anchors": ["劍不知道什麼時候不見了。"],
      "locations": ["temple", "workshop"], "cast": ["seigo", "lak"]
    },
    { "n": 4, "title": "第三章：地震與祭典", "goal": "首次地震、拉克消失一下午；海灘祭典的溫馨夜晚。", "beats": ["早晨地震，拉克說要去確認一些事後消失。", "海灘營火祭典：祈浪舞、紅黑白小木舟裝花與五穀推入海；拉克換祭典裝。"], "end_when": "祭典夜結束。", "plant": ["lingui-pressure"], "locations": ["workshop", "beach-festival"], "cast": ["seigo", "lak"] },
    { "n": 5, "title": "第四章：鏽釘", "goal": "誠吾被鏽釘刺傷感染破傷風、惡化；拉克抱他進禁地洞穴。", "beats": ["鏽釘刺傷、診所羊醫生處理；缺免疫球蛋白、船五天後才來。", "症狀惡化、拉克焦急決斷。", "洞穴祭壇、神劍；誠吾昏迷前看見拉克以古老聲音祈禱、像野獸喘息。"], "end_when": "誠吾在洞穴失去意識。", "plant": ["lingui-pressure"], "locations": ["workshop", "clinic", "lak-house", "cave"], "cast": ["seigo", "lak"] },
    { "n": 6, "title": "第五章：奇蹟", "goal": "醒來痊癒，醫生稱奇，拉克避而不談。", "end_when": "誠吾接受拉克不想談。", "locations": ["lak-house", "clinic"] },
    { "n": 7, "title": "第六章：夜的喘息（成人）", "goal": "誠吾察覺拉克夜裡痛苦壓抑；主動越線；之後拉克內疚迴避，兩人仍有一段甜蜜日常。", "end_when": "日常恢復。" },
    { "n": 8, "title": "第七章：反胃與洞穴", "goal": "反胃初兆；拉克失蹤；誠吾獨自找到洞穴、發現和也筆跡的嶺鬼紀錄、摔傷，被拉克帶回。", "end_when": "誠吾帶著文獻被帶回家。", "plant": ["seigo-pregnant"] },
    { "n": 9, "title": "第八章：拼圖", "goal": "詢問拉克、得知和也是『很重要的朋友』、拉克得知和也過世崩潰；讀完文獻得知雙面神與嶺鬼職責。", "end_when": "clue.notes 為真。" },
    { "n": 10, "title": "第九章：壓抑與老婆婆", "goal": "拉克再度壓抑並拒絕誠吾、催他回本島；地震加劇；老婆婆說和也因身體不適離島；拉克失控差點傷害誠吾後逃走。", "end_when": "clue.grandma 為真且拉克離開。", "payoff": ["lingui-pressure"] },
    { "n": 11, "title": "第十章：夢與真相", "goal": "向赤山王祈禱、夢中受託；老婆婆說古神後裔是拉克；工具箱找到短劍、合鞘，誠吾想通一切。", "end_when": "clue.truth 為真。", "payoff": ["sword", "kazuya-mother", "seigo-pregnant"], "anchors": ["喀。"] },
    { "n": 12, "title": "第十一章：洞穴", "goal": "說服波伯帶路上山、洞穴祈禱、嶺鬼顯現欲附身、察覺孩子。", "end_when": "嶺鬼停在誠吾面前。" },
    { "n": 13, "title": "第十二章：相認與結合（成人）", "goal": "拉克趕到代為承接附身；誠吾說出真相、亮出合一的劍與鞘；兩人在洞穴結合，嶺鬼嘆息、地震平息。", "end_when": "洞穴回歸寧靜。" },
    { "n": 14, "title": "第十三章：雕刻嶺鬼", "goal": "事後對話；取回神劍；誠吾在神像背面雕出安靜的嶺鬼，神劍放進其掌心。", "end_when": "神像完工。" },
    { "n": 15, "title": "第十四章：雙神入夢", "goal": "赤山王與嶺鬼同時入夢，解釋壓力機制，請誠吾留下；誠吾答應。", "end_when": "誠吾醒來。" },
    { "n": 16, "title": "第十五章：太陽的氣味", "goal": "清晨拉克不再迴避；誠吾告白；拉克回應、接吻，三代人的『太陽的氣味』閉環。", "end_when": "輸出 [ending: warm]。", "anchors": ["原來這就是太陽的氣味。"] }
  ]
}
```

### A.6 開場（`opening`，劇本格）

```
[bg: ferry | afternoon]
客輪底下的引擎發出規律的嗡嗡聲，冷氣很強，空氣裡有塑膠和消毒水的味道。

我靠著窗，大腿上放著一個用舊棉布包起來的木雕。中空的，一端圓弧、一端有個開口。

這是父親留下來的東西。

[choices]
- 拆開棉布，再看一次木雕
- 望著窗外的海，想起那個午後
```

篇幅實測：本附錄（A.1–A.6，含 JSON 語法與英文生圖提示）約 15,000 字元，其中第 1–3 章為完整密度、第 4–16 章只有概要；第 4–16 章補到同密度後整卡約 35,000–45,000 字〔推測〕，在第 4 節建議上限內。
