# 內建規則書補齊四套＋選書版面

作者原話（2026-11-13）：「我想要在TRPG房間加入之前列出的剩下四種規則，你負責當主線兼任常識審查，幫我加入並看怎麼安排比較好看，可以嗎？請幫我確認是單人和多人都有。」

沿用 [ttrpg-rules-system](ttrpg-rules-system.md) 拍板 1 與「C 包技術方案」：每套 20–60 條、每條 2–5 行、關鍵字觸發、只餵 GM、內文是官方授權稿英文原句的刪減摘錄（不改寫、不憑記憶）、附署名與授權條目、介面標題不用出版商商標。以下除標明外皆〔模型判斷·未裁決〕。

## 單人與房間
單人桌與房間桌共用同一個入口與同一條注入路徑：桌設定 `WorldEditor` → `DiceSettings` → `RuleBookList`，`AppWorkspace` 傳 `onAddRulesBook` 不分 `roomMode`；GM 回合只有 `chat_turn/gm.rs` 一處組裝。書加進桌就是單人、房間都有；手機玩家端沒有新畫面（規則只餵 GM）。骰池固定含 d6×12、d20×8、d100×4，骰鈕吃 `XdY±Z`，五套的骰法都不必改骰子。

## 版面與行為（定案）
- **一桌一套內建書**：兩套規則的條目會被同樣的常用詞同時觸發、餵給 GM 互相矛盾的裁決。已加入一本時，其他書的鈕是「換成這本」：一次確認（講明舊書刪幾條；有改過的才補一句會保留），先刪舊書沒改過的條目，刪成功才加入新書。玩家自己匯入的世界書不受限。
- **列的長相**：第一行＝白話名（粗體，隨語系）＋系統代稱小字＋右側鈕；第二行＝一句副標；ⓘ 展開說明與署名。
- **桌設定預設收合**：沒加書時只有一行「▸ 規則書（選一套；不加也能玩）」，點開列五本；加了一本後那本常駐（移除鈕），其餘收在「▸ 換一套規則」。加入或換書成功後自動收起。理由：五本全列會把世界設定欄擠到第一屏之外（2026-11-13 截圖）。
- **指南對話框**：五本永遠全列，鈕與桌設定同步。
- **順序**（新手到老手）：5e SRD 5.1 → Dungeon World → Cairn 1e → BRP → PF2 Remaster。系統代稱＝書 JSON 的 `name`，也是世界書條目上的標籤；授權寫在 ⓘ 的署名小字，不放進代稱。
- 白話名（zh-TW）：經典奇幻冒險／說故事的奇幻冒險／極簡老派探險／百分骰・調查與現代／重戰術奇幻冒險；副標與說明見 `rules-copy.json`，其餘九語寫成當地白話。
- **書目錄**：`fingerprint` 每本唯一；「已加入」＝標題屬於該書且內文含 fingerprint（Dungeon World 的授權原文含 5e 署名網址，只看內文會誤判）；條目標題跨書不重複、每條 key 不重複（都有測試）；中日韓關鍵字至少兩字。同時間只跑一個加入／移除／換書。

## 各書來源（2026-11-13 主線實抓皆 200）
| 書 | 來源 | 授權 |
| --- | --- | --- |
| Dungeon World | `github.com/Sagelt/Dungeon-World` 的 `text/*.xml`（LICENSE 同 repo） | CC BY 3.0；LICENSE 另列其中 SRD 5.1 內容 CC BY 4.0，兩段署名都要保留 |
| Cairn 1e | `github.com/yochaigal/cairn` 的 `first-edition/cairn-srd.md`（SRD v1.0） | CC BY-SA 4.0；本書 JSON 以同授權釋出，署名條目與 README 寫明 |
| BRP | Chaosium `BasicRoleplaying-ORC-Content-Document.pdf`（SHA-256 `d38af1fc…cedbb28`） | ORC；照文件內 ORC Notice 原文 |
| PF2 Remaster | Archives of Nethys `2e.aonprd.com` 內標 Player Core 的頁 | ORC；只取 Player Core 的 Licensed Material，不取舊 OGL 頁、GM Core 與 Reserved Material |

每本施工時記下實際取得的網址、日期與 SHA-256。授權聲明用字、署名句一律從抓到的文件讀，不從本表或記憶抄；來源或授權讀不清楚就停下回報，不寫那本。

### 包 1 實抓（2026-11-13）

Dungeon World，`https://github.com/Sagelt/Dungeon-World` 的 `master`。摘錄只用下列檔，原始檔不進 repo。

| 檔 | 網址 | SHA-256 |
| --- | --- | --- |
| LICENSE | `https://raw.githubusercontent.com/Sagelt/Dungeon-World/master/LICENSE` | `a7ee56d4993a1b876a1bc3896d6b16dd8ac3cf4e00a857d5eec7ba1e4e7d2e4e` |
| text/Moves.xml | `https://raw.githubusercontent.com/Sagelt/Dungeon-World/master/text/Moves.xml` | `4a9517ffe668a7ba1cc9513b5482d622c71241f92b1a9e3b52cc182214d0ac97` |
| text/Playing_the_Game.xml | `https://raw.githubusercontent.com/Sagelt/Dungeon-World/master/text/Playing_the_Game.xml` | `77276be2cc87af2a281fb32bb4f9941e4e32f030032d0fa909e040bf29245a7f` |
| text/GM.xml | `https://raw.githubusercontent.com/Sagelt/Dungeon-World/master/text/GM.xml` | `1b67ee47f77b888c869e8912346852fad706dd7cd3099bbca8013344d2c78000` |
| text/Character_Creation.xml | `https://raw.githubusercontent.com/Sagelt/Dungeon-World/master/text/Character_Creation.xml` | `45add5f581fee92e40eb7b9ab122a22681136de576878700dada7f6076d8eb33` |

LICENSE 只有兩段授權句，沒有作者姓名。署名條目照該檔逐字（含 SRD 5.1 段），前面加一行來源網址；fingerprint 用 repo 網址 `github.com/Sagelt/Dungeon-World`。作者姓名查過 README、LICENSE、`text/Introduction.xml`、`text/appendices/Thanks.xml`、dungeon-world.com 的 about／downloads／buy 頁，都沒有列出作者；只有 GitHub 帳號 Sagelt 的個人檔名稱是 Sage LaTorra，README 卻寫 authors（複數），所以不寫姓名〔模型判斷·未裁決〕。系統代稱維持「Dungeon World」。屬性調整值在 XML 裡是緊挨的表格格子，書裡那一行只在格子之間加空白。已加入判定改成「標題屬於這本、且內文含 fingerprint」：LICENSE 原文含 5e 的署名網址，只對內文做子字串會把這本也看成 5e 已加入。

### 包 2 實抓（2026-11-13）

Cairn 1e，`https://github.com/yochaigal/cairn` 的 `main`。

| 檔 | 網址 | SHA-256 |
| --- | --- | --- |
| first-edition/cairn-srd.md | `https://raw.githubusercontent.com/yochaigal/cairn/main/first-edition/cairn-srd.md` | `62d09f6769971a6aa61a1d41fb498c2640a4809df2638ad664b8938795681c14` |
| README.md | `https://raw.githubusercontent.com/yochaigal/cairn/main/README.md` | `879ff51754cfa862e2d559694ed90c04522272082617225da2d390e1f24f0149` |

SRD 本文寫「Cairn is licensed under CC-BY-SA 4.0」，README 寫「The game was written by Yochai Gal」並同樣標 CC-BY-SA 4.0；repo 根目錄 LICENSE 卻是 GPLv3（應是網站程式碼）。內文授權以 SRD 本文為準〔模型判斷·未裁決〕。署名條目寫標題、作者、來源網址、授權連結，並寫明本書 JSON 是改作、以同授權 CC BY-SA 4.0 釋出；修改說明含「刪句刪段、去掉 Markdown 標記、表格列分到三條」。Scars 表 12 列拆成三條（1–4、5–8、9–12），每列一行。Cairn 1e 沒有經驗或等級，「成長」只收 Growth 原則與 Narrative Focus 那一句。

### 包 3 實抓（2026-11-13）

| 檔 | 網址 | SHA-256 |
| --- | --- | --- |
| BRP ORC Content Document（303 頁） | `https://www.chaosium.com/content/orclicense/BasicRoleplaying-ORC-Content-Document.pdf` | `d38af1fc2c27326b0ee8e42af0579684220ccf30809b195275d9daf29cedbb28` |
| Chaosium ORC 頁（notice 與 Sample Attribution；頁面自註 subject to change） | `https://www.chaosium.com/orclicense`（轉到 `/orc-license/`） | 2026-11-13 存下的 HTML：`34a379170f285085b76bef8d4896ad90ae3423f26e7f7337d19aa023950cbecc` |
| ORC License FINAL（Azora） | `https://azoralaw.com/wp-content/uploads/2023/09/ORC-License.FINAL_.pdf` | `3fffe0fe69781b7115bccbfa0f51489e0a073c211772f5ebcd84d34f88f19b29` |
| ORC AxE | `https://www.chaosium.com/content/orclicense/ORC_AxE_FINAL.pdf` | `a38fb4e74d1de2647b5e6bd572c2e6f46196461c8f1413347ab5cfd74afdc27b` |

文字用 macOS PDFKit 抽出；比對時去掉頁尾「Basic Roleplaying ORC Content Document N / Chaosium, Inc.」與換頁標記，避開頁尾黏句、表格亂序與行尾斷字的段落。ORC Notice 用 Chaosium 網頁指定的「BRP: Universal Game Engine Notice」原文（TX 9-307-067）；PDF 第 1 頁同段還寫「TX000 [number tbd]」、© 2023，網頁 Sample Attribution 寫 © 2024，署名照網頁〔模型判斷·未裁決〕。ORC III 另要求 Attribution、改作者希望怎麼被署名、Reserved Material 與 Expressly Designated Licensed Material 聲明，署名條目各寫一句（後兩項寫 none），再加一句非官方、未受背書。Chaosium 請求「Powered by BRP Logo」署名是圖，JSON 放不了，留到收尾決定要不要在說明頁放〔模型判斷·未裁決〕。名稱：ORC 不授權商標（Reserved Material），AxE 明說可依 nominative fair use 標示相容的系統，所以系統代稱維持「BRP」，文案不暗示官方或背書；摘錄不收商標詞與專有名詞〔模型判斷·未裁決〕。理智（Sanity）在 ORC 文件第 10 章，屬 Licensed Material，收四條（含暫時瘋狂的兩張表）。Chaosium notice 裡「This product is the original work of Chaosium」、Product Identity 段、「With a very few exceptions」段講的是 Chaosium 自己的產品，沒抄進署名條目〔模型判斷·未裁決〕。

### 包 4 實抓（2026-11-13）

PF2 Remaster，只取 Player Core。原始頁不進 repo。

| 檔 | 網址 | SHA-256 |
| --- | --- | --- |
| ORC License（Paizo 頁，含 AxE 全文） | `https://paizo.com/orclicense` | `c37386bed9a31181efb62627a933ec3bfd43ddc81e1362c0b2b4fe1bc3444782` |
| ORC AxE PDF | `https://downloads.paizo.com/ORC_AxEFINAL.pdf` | `eea1902ed7e5237d2adb9d58b797f370084f5346494c011f56fd45cc2f38126e` |
| Paizo 授權總覽 | `https://paizo.com/licenses` | `b4513700a4da43b7c4e38fd3a437d70ebf26fd06f3b7caceaac0238797b08254` |
| Paizo blog「Pathfinder Infinite and the ORC License」（2023-11-14） | `https://paizo.com/community/blog/v5748dyo6sig4` | `cf7177b2b71e130195ca1513a7b0ad31f63847de9d6308f0c64ebfacecea85f7` |
| AoN 授權頁（署名字串） | `https://2e.aonprd.com/Licenses.aspx` | `0185c88d01051f8ed1babce094f6d245fd783a5cfad0d9c091ef90f991b5198a` |
| AoN 來源頁 Player Core（發售 11/15/2023、已套 2026-02-06 勘誤） | `https://2e.aonprd.com/Sources.aspx?ID=216` | `5e59c07334121825407abdb8080a21f687f9f57024d32a8201bd6a16428a8b39` |
| Roll20 Player Core 版權頁轉載（旁證） | `https://roll20.net/compendium/pf2/Rules:Credits` | `a1beb79709d8848a206edb89f7a91c8f0caf89c55332bd9cf1b885340b6286eb` |
| 摘錄用的 56 頁 AoN 頁（ID 見下） | `Rules.aspx?ID=…&NoRedirect=1`、`Conditions.aspx?ID=…`、`Actions.aspx?ID=…&NoRedirect=1` | 各頁雜湊清單的雜湊 `4e4f45091b78780edc7d1e5a7841991c56c107c226f0376cacb2301fa671a589`；同頁重抓雜湊不變 |

- Rules ID：2137 2268 2269 2278 2279 2281 2282 2284 2286 2287 2288 2289 2295 2296 2297 2306 2307 2311 2317 2318 2319 2320 2324 2328 2330 2333 2338 2343 2372 2375 2416 2417 2419 2423 2428 2430 2443 2455。
- Conditions ID：58 59 61 68 71 73 76 77 88 90 91 92 93 94 99。Actions ID：2399 2404 2405。

- 只取 Remaster：AoN 每頁有 `<div class="sources">` 指向 `Sources.aspx?ID=216`「Player Core pg. N」；舊版頁指向 `ID=1`「Core Rulebook」且標「Legacy Content」（負對照 `Rules.aspx?ID=174`、`ID=312`、`Conditions.aspx?ID=3&NoRedirect=1`）。第 8 章 195 頁全是 ID=216；摘錄用 56 頁的內文區段（含 Rules 2343 內嵌的動作區塊）所有來源連結也全是 216、無 Legacy 標記，條目標題去掉「(PF2)」後都在該頁出現。不帶 `NoRedirect` 的舊版狀態頁會轉到 Remaster 頁，抓頁一律帶它或核對來源欄。
- Notice：ORC 句照 paizo.com/orclicense III.a 原文。署名字串 AoN 授權頁與 Roll20 版權頁轉載逐字相同。Player Core 屬 ORC：Paizo blog 寫「Tomorrow, Paizo will release our first books using the Open RPG Creative license (ORC)」，日期與 Player Core 發售日相符；書內 ORC Notice（Reserved Material 範圍、無 Expressly Designated Licensed Material）只在 Roll20 轉載讀到；paizo.com/pathfinder/playercore、/pathfinder/remaster/faq 原始 HTML 與 AoN 授權頁都沒有 ORC 字樣。Player Core 屬 ORC 靠 blog 日期對上加 Roll20 轉載〔模型判斷·未裁決〕。
- 授權鏈：AoN 自稱依 Paizo Community Use Policy 經營、未受 Paizo 背書，權利不從 AoN 來，是 Paizo 依 ORC II.a 直接授予（「in all media and formats」，AxE：「licensed to use Licensed Material verbatim」）。AoN 是轉錄本，已套勘誤、會把連結改寫成「here」「page N」、插入「Paizo Clarification」；逐句比對只證明忠於 AoN，不證明逐字等於印刷版〔模型判斷·未裁決〕。
- 只收 Player Core（GM Core 沒抓到它自己的 notice）；簡單 DC 表用 Player Core 第 4 章那張。
- 摘錄 59 條＋署名 1 條；腳本逐句比對 313 句全中，比對前只把 AoN 連結造成的「字 空白 標點／破折號」併回。避開帶 here／頁碼指涉、Paizo Clarification、專有名詞與商標（Pathfinder、Golarion、Halfling Luck、Grim Reaper、the Void、Combat Pad 等）及帶具名法術的例句。表格的格與列用「 | 」接成一行；有幾條在句中截掉尾端的括號交叉參照。
- 撞名加後綴：Saving Throws、Cover、Stunned、Prone 加「(PF2)」。系統代稱「PF2 Remaster」照 AxE nominative fair use，署名條目加商標歸屬與非官方句〔模型判斷·未裁決〕。

## 結案（2026-11-14）
- 施工：包 1（書目錄、版面、Dungeon World）Grok；包 2–3（Cairn、BRP 與截圖後小修）、包 4（PF2 Remaster）Opus；收合版面、操作鎖與 Sol 必改由主線直接改。作者 2026-11-13 指示本對話改由 Opus 寫碼、Sol 審查，Grok 額度留給生圖測試。
- 驗收：主線每包重跑 `npm run verify`（17 步全過）、四本逐句比對抓到的原文（Dungeon World 0 句、Cairn 1 句、BRP 1 句、PF2 0 句對不上，兩句都是原文夾連結標記／跨行）。Sol 驗收 2 輪：第 1 輪必改 1（刪舊書失敗仍加新書）採納前半，「加入失敗還原舊書」主線否決；第 2 輪 PASS。Fable 常識審由主線兼任，截圖 3 輪（單人桌與房間桌）親看。
- 行數：核心約 +131／−55（立案估 120–180），測試約 +110；書 JSON 四本與十語系文案另計。

## 待作者裁決
- **PF2 Remaster 的授權鏈要不要留**〔作者 2026-11-13：「如果很不好做，也可以放棄」〕：書做出來了，內文 319 句都對得上 Archives of Nethys 標為 Player Core 的頁，但有兩個弱點——(1) Paizo 沒有免費官方原稿，摘錄來源是 AoN 的轉錄本；(2) 書內 ORC Notice 的 Reserved Material 範圍只在第三方轉載讀到，不是 Paizo 官網原文。主線判斷可留〔模型判斷·未裁決〕；要拿掉就刪 `pf2-remaster.json`、`rule-books.ts` 的一筆與 `rules-copy.json` 的 `pf2`。**下次發佈前請裁決。**

## 已知限制
- 換書時新書加入失敗（罕見），舊書已刪：再按一次加入即可，玩家改過的條目不受影響。〔主線裁定，否決 Sol 的還原方案〕
- 回合進行中換書，刪除立刻生效、加入等回合結束，中間短暫沒有規則書。〔主線裁定〕
- 刪掉或改掉署名條目會被看成「未加入」；只改關鍵字、沒改內文的條目在移除時仍會被刪。
- Dungeon World 署名沒有作者姓名：repo 與官網各頁查不到可靠出處，只寫來源網址與授權。
- Chaosium 希望以「Powered by BRP」標誌署名，書檔放不了圖，尚未放在任何說明頁。
- PF2 的「檢定」「d20」等核心詞幾乎每回合觸發（59 條、約 3.1 萬字元，是五本裡最厚的）。
- 5e 與 Dungeon World 仍有單字的中日韓關鍵字（毒、弓等），子字串比對會誤觸發。

## 發佈時要補（這次沒動，免得網頁說明先於 app 上線）
README 授權段（四本的來源與授權、Cairn 同授權聲明）、三語房間說明書與下載頁提到內建規則書的句子、CHANGELOG；照規約送 Fable 白話審。

## 觀察
