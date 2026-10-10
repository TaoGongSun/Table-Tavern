# worldbook-path-foreign-ids 施工方案

交接見 [handoffs/worldbook-path-foreign-ids.md](../handoffs/worldbook-path-foreign-ids.md)。行號以分支起點 `fbc1095` 為準，路徑相對 `src-tauri/src/`。

## 一、現況

### 寫入
- 兩條路共用 `data/worldbook/book_import.rs` 的 `import_worldbook_as`（`:73`）→ `normalize_imported_entry`（`:241`）。
- 本桌角色 id 集合只在角色卡路讀（`:96-102`，`list_characters`，不含玩家卡 `data/character.rs:251`）；世界書路給空集合。
- 世界書路（`BookOwner::Gm`，`:261-265`）：`visibility` 有寫就原樣保留，沒寫才補 `gm`；`source_cards` 完全不碰，原樣留下。
- 角色卡路（`:268-275`）：`explicit_card_visibility`（`:304-324`）只留本桌 id，濾空或讀不懂就給這張卡自己的角色；`source_cards` 一律蓋成 `[本卡 id]`。這條路已經沒有殘留問題。
- 去重命中時 `merge_entry_into`（`:342-375`）把來源條目的名單與 `source_cards` 併進桌上那條；收據記合併前後值（`:134-147`、`:183-193`）。

### 讀取
- 解析：`data/worldbook.rs:105-127` `visibility_from_value`，物件形 `characters` 全是字串就讀成 `Characters(ids)`，**不檢查 id 在不在本桌**；其餘讀不懂的退回 `Gm`。
- 掃描可見度：`world_scan/book.rs:116-122`，角色視角只看名單內的 id；GM 全看。
- 機密旗標：`world_scan/book.rs:55` `limited`、`world_scan/placement.rs:55-57` `confidential()`，只要是 `Characters(_)` 就成立，不管名單裡是誰。影響遞迴公開緩衝（`world_info/scan.rs:638-645`）、outlet 私密標記（`world_scan/mod.rs:276-280`）、角色線機密段分流（`transport/turns.rs:245-249`）。
- 跟卡匯出的歸屬：`data/worldbook/raw_entries.rs:178-195`，名單或 `source_cards` 含這張卡就跟著匯出。
- 合併落定規則：`book_import.rs:356`，帶 `source_cards` 的 `Gm` 不會被後來的角色卡收成名單。
- 重構：`refactor/sources.rs:150-176` 新條目沿用名單、聯集 `source_cards`；`refactor/apply.rs:590-603` 照搬條目整包抄原始值；`refactor/apply.rs:604-612` 名單全不在本桌時改 `Gm`（只管全別桌，混雜的照留）。
- 顯示：`refactor_ai/context.rs:147-157` 名單 id 找不到角色就印 id 原文；前端 `src/features/worldbook/worldbook-model.ts:16-20`、`WorldbookEntryForm.tsx:117-135` 選「指定角色」但沒有任何勾選框是勾的，看不到也取消不了的 id 照留。

### 帶進別桌 id 的入口
| 入口 | 位置 | 現況 |
|---|---|---|
| 世界書路（桌面版） | `import/files.rs:72` | 名單、`source_cards` 原樣留下 ← 本案 |
| 網頁存檔·世界書路 | `import/web_save/mod.rs:196` | 同上，走同一個函式 ← 本案 |
| 角色卡路（桌面版、網頁存檔） | `import/card.rs:378-379` | 已濾名單、已蓋 `source_cards` |
| 整本世界書匯出 | `data/worldbook.rs:778-789` | 直接複製 `worldbook.json`，帶出本桌 id——別桌 id 的主要來源 |
| 角色卡匯出 | `import/export.rs:317-326` | 只留 `gm`／`public`，名單與 `source_cards` 都不帶出 |
| AI 重構 | `refactor/sources.rs`、`refactor/apply.rs` | 重構卡的 meta 會帶來源桌的名單；回退時只把全別桌名單改 GM，混雜名單照留（`refactor/apply.rs:604-612`）。範圍外〔模型判斷·未裁決〕 |

### 修正上一份立案說明
立案說的「讀起來等於只給 GM，行為無誤」不完全成立：全是別桌 id 的名單讀成 `Characters`，角色確實看不到，但 GM 掃描把它當機密條目（`limited`／`confidential()`），遞迴文字不進公開緩衝、所在 outlet 被標私密。名單型條目合併時還會污染桌上既有條目：別桌 id 併進本桌條目的名單、把沒有來源卡的 `Gm` 條目改成 `Characters(別桌)`、名單型條目的別桌 `source_cards` 併進來讓 `Gm` 條目變成「落定」，擋住之後的角色卡收編。另外空名單 `characters: []` 讀成 `Characters([])`（`data/worldbook.rs:113-117`），不是退回 GM，而是沒有角色看得到的機密條目。

## 二、做法
1. `import_worldbook_as` 兩條路都在鎖內讀本桌角色 id 集合（`book_import.rs:96-102` 拿掉分支）。世界書路因此多一種失敗：讀 `characters` 目錄出錯時整次匯入失敗；目錄不存在時回空清單（`data/character.rs:231`），不會失敗。這跟角色卡路一致，可以接受。
2. 世界書路的可見度（新寫一個 `explicit_book_visibility`，角色卡路的 `explicit_card_visibility` 不改）：
   - `"gm"`／`"public"` 照留。
   - 物件形 `characters` 先照讀取端（`data/worldbook.rs:105-127`）的規則驗整個陣列：不是陣列，或混有非字串（例如 `["本桌id", 42]`），整份當讀不懂。全是字串才濾成本桌 id。
   - 濾空、空名單 `characters: []`、讀不懂、沒寫，一律寫成 `"gm"`（P1）。
3. 世界書路的 `source_cards`：
   - 可見度明寫為 `"gm"` 且 `source_cards` 非空：「明寫」只認來源條目原始值是字串 `"gm"`，在改寫 visibility 之前判斷；「非空」指 `source_cards()`（`book_import.rs:327`）解析後至少有一個字串。這種條目原樣保留，不濾。〔模型判斷·未裁決〕GM 條目上的 `source_cards` 用來「落定」（`book_import.rs:342-358`），避免之後被同內容的角色卡收成名單。濾空會讓別桌已改成 GM 的內容，在本桌再匯入同一張角色卡時開放給該角色看，等於擴大可見度。保留這些字串不影響誰看得到，跟卡匯出的歸屬也對不到本桌角色。
   - 其餘條目（名單型、公開、由名單濾空或讀不懂改寫成 gm 的）：只留本桌 id，空了就移除欄位（沿用 `raw_entries.rs:203` `set_source_cards` 的語意）。
4. 註解同步改寫：`book_import.rs:260` 世界書路規則、`:266-267` 角色卡路說明（改成對照世界書路的寫法）、`:302-303` 函式說明。
5. 角色卡路、讀取端、匯出、重構都不改。去重指紋（`book_import.rs:446-470`）不含可見度與 `source_cards`，認重複的結果不變；合併與收據自動拿到處理過的值，不需改。
6. `src/shared/contracts/web-save/web-save.md:82` 補一句：世界書路的角色名單只留本桌 id，濾空就給 GM。

濾掉之後的影響：
- 去重：命中與否不變。同桌匯出再匯回時 id 都在本桌，名單與 `source_cards` 原樣保留，合併後值不變，不產生收據還原項。
- 合併：名單型條目的別桌 id 不再進本桌條目；全是別桌 id 的名單改成 gm 進來，`(Gm, Gm)` 與 `(Characters, Gm)` 都保留桌上原值。
- 來源卡：名單型條目的別桌 `source_cards` 本來就對不到本桌角色，匯出歸屬不變；明寫 gm 的照留，落定效果不變。
- 可見度：全別桌名單由「機密的 `Characters`」變成一般 `Gm`，GM 視角的遞迴與 outlet 不再被標私密；角色視角不變（本來就看不到）。混雜名單只剩本桌角色看得到，跟現在一樣。

## 三、範圍
- 程式只改 `data/worldbook/book_import.rs`；文件改 `src/shared/contracts/web-save/web-save.md:82`。
- 測試：新檔 `import/foreign_ids_tests.rs`（要用到角色卡路、收據撤銷、GM 掃描，`import/card_book_tests.rs` 已過千行，另開一檔）；`data/worldbook/book_import_tests.rs` 只改寫既有那條；以及 `import/web_save/tests.rs`（讀進 `worldbook-route.json` 後在記憶體裡改，照 `tests.rs:1070` 的寫法）。不動 `src/shared/contracts/web-save/` 的 fixture。
- 範圍外〔模型判斷·未裁決〕：已在桌上的殘留（P2）、刪角色後的名單（另案 character-delete-visibility-cleanup）、重構（見入口表）、前端。

## 四、測試
1. 世界書路可見度：名單全別桌 → `"gm"`；混雜 → 只剩本桌 id；`gm`／`public` 照留；`characters: []` → `"gm"`；混了非字串的名單（`["本桌id", 42]`）→ `"gm"`。
2. 既有測試 `book_import_tests.rs:27-40` 改寫：`42` 存成 `"gm"`、讀取仍是 `Gm`（原本 `:36` 斷言原樣保留）；`:14` 說明一起改。
3. `source_cards`：名單型全別桌 → 欄位移除；混雜 → 只剩本桌 id；明寫 gm 帶別桌 `source_cards` → 原樣保留。
4. 別桌落定的 GM 條目：世界書路匯入後，再匯入同內容、條目不寫 visibility 的角色卡，仍是 `Gm`。對照：同樣情境但 gm 條目沒有 `source_cards`，會被收成該卡的名單。
5. 同桌往返（去重）：整本匯出再匯回同一桌，全數略過，名單與 `source_cards` 不變，收據沒有還原項。
6. 同桌往返（新插入）：匯出 → 清空世界書 → 匯回，條目全部新插入，名單與 `source_cards` 原樣保留（驗證確實讀了本桌 id 集合）。
7. 合併：桌上 `Characters([本桌])` 遇到全別桌的同內容條目 → 名單不變；桌上沒來源卡的 `Gm` 遇到全別桌 → 仍是 `Gm`，之後匯入同內容的角色卡仍收得成該卡的名單。
7b. 合併落定：桌上沒有來源卡的 `Gm`，遇到「明寫 gm＋別桌 `source_cards`」的同內容條目會被落定，之後的角色卡收不成名單。
8. 去重收據：混雜名單只併本桌 id，撤銷時還原 before；玩家改過的可見度撤銷時保留（照 `import/card_book_tests.rs:555`、`:693` 的寫法）。
9. GM 掃描（放 `import/foreign_ids_tests.rs`，用 `chat_assembly::test_gm_scan` 拿 GM 視角的 `placed`）：全別桌名單改成 gm 之後，該條目的 `confidential()` 不成立（`limited` 與 outlet 私密標記都由同一判斷而來）。
10. 網頁存檔世界書路（`import/web_save/tests.rs`）：記憶體裡塞別桌名單與 `source_cards`，匯入後被濾掉。
11. 既有角色卡路測試（`import/card_book_tests.rs:287` 等）照舊全綠。
- 驗證：`npm run verify`、`cargo test`。只動資料層，不需 GUI 實測。

## 五、已知差異
- 與 ST：ST 的條目沒有「給哪個角色看」的欄位，最接近的 `characterFilter` 匯入時也不清理對不到的角色。本案多做一步濾掉，邏輯簡單、資料更乾淨，不照 ST。
- 玩家卡 id 不算本桌角色（沿用角色卡路的 `list_characters`）：名單裡若有本桌玩家卡的 id，同桌往返會被濾掉；只含玩家卡時變成 `Gm`。玩家卡沒有自己的掃描視角，實際看得到的人不變。〔模型判斷·未裁決〕
- 讀不懂的 `visibility`（數字、混了非字串的名單）原本原樣保留、讀取時退回 GM；改後直接寫成 `"gm"`，看得到的人不變。空名單原本讀成 `Characters([])`（機密、沒人看得到），改後寫成 gm，等於拿掉機密旗標。
- 明寫 gm 的條目會把別桌的 `source_cards` 帶進來，去重合併時併進桌上那條（`book_import.rs:364-373`）；桌上原本沒有來源卡、還沒落定的 `Gm` 因此被落定。這是刻意的：別桌作者「只給 GM」的決定傳到同內容的條目上。〔模型判斷·未裁決〕
- 明寫 gm 的條目保留別桌 `source_cards`，桌上會留著對不到角色的字串；這是為了保住落定效果。〔模型判斷·未裁決〕
- 同桌往返「完全不變」只對有效的本桌 id 成立。桌上已有殘留時，去重命中只會把新值併進留下的那條，不會順手清掉它原有的別桌 id。

## 六、待作者拍板
**P1 名單濾空後給誰**
- 選 A（GM）：跟世界書路「沒寫就給 GM」、讀取端讀不懂退回 GM、重構全別桌改 GM 一致；角色都看不到這條，跟現在看到的人一樣。
- 選 B（全員公開）：所有角色都看得到這條。原作者明明限定過可見範圍，公開等於擴大洩漏範圍。
- 建議 A。

**P2 已經匯入、桌上殘留的別桌 id 要不要清**
- 選 A（不清，只管新匯入）：改動只在匯入函式；舊桌留著的殘留照現在行為（角色看不到、GM 當機密）。依「正式發佈前舊桌不相容」不需要遷移。
- 選 B（清）：加一次性清理或讀取時濾掉，舊桌也乾淨；要多改讀取端或加遷移，還要處理收據與撤銷的前後值對不上。
- 建議 A。

另：本案「名單空了退回 GM」只管匯入。刪角色後名單變空要給誰由 character-delete-visibility-cleanup 自己決定，不受本案結論約束。

**P3 明寫 gm 時保留別桌 `source_cards`，合併時會落定桌上的 `Gm`**
- 選 A（現做法）：別桌作者「只給 GM」的決定會傳過來，同內容的角色卡收不走。
- 選 B（一律濾掉）：比較簡單，但上面那個決定不會傳過來，之後匯入同一張角色卡會把它收成該角色名單。
- 建議 A。〔模型判斷·未裁決〕
