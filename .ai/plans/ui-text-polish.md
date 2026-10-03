# ui-text-polish — 介面文字四處小毛病

2026-10-02 實測梯 1 順手發現，主線 2026-10-03 於 df5c399 確認四項皆成立。

## (a) 計數字串單複數〔作者裁決 2026-10-03：全部帶計數的字串、所有需要複數區分的語系一起修〕

- 做法：`src/i18n/plural.ts`（不引依賴）在 `t()` 代入參數前展開 plural，選出分支、`#` 換回 `{x}`，再走原本的單次代入（代入值裡的 `{…}`／`#` 不再被解讀）。類別由平台 `Intl.PluralRules(lang)` 判定，缺該類別退 `other`；參數須為數字或純數字字串，缺參數或非法值走 `other`。features 字典同走 `t()`。
- 文法（ICU 子集）：`{x, plural, 類別 {分支} …}`。類別只收 zero／one／two／few／many／other，必須有 other、不得重複；分支內只准 `#`、一般佔位符與文字；巢狀 plural、`=n`、`offset:`、ICU 單引號跳脫（`'{` `'}` `''` `'#`）一律拒絕，單引號是普通字元；其他任何大括號都算錯。執行期帶 plural 語法卻解析失敗時，整串模板原樣回傳、連一般佔位符都不代入，不半解析。數字字串轉換後也須是有限數。
- `scripts/check-i18n.mjs`：語系字典改依檔名辨識（`xx.ts`／`xx-YY.ts`），plural.ts 不再被當字典；每個字串先過文法，錯的列出；佔位符比對用 plural.ts 的 `messagePlaceholders`（plural 參數算一次、分支內一般佔位符各分支須一致且算一次、其餘照重複次數算）；按鈕計寬用 `widestRendering`（每個區塊取最寬分支）。
- 呼叫端 `SettingsForm` 的冷卻秒數改傳 number。
- 掃描方式：列出 en 主字典與 features 字典的全部佔位符，挑出數字型（n、d、count、secs、rounds、observed、done、total、remaining、limit、pct、size），並與 `BACKEND_MSG_PARAMS` 的 number 欄位交叉核對（多出 `no_migration_path` 的 from／to＝格式版本號、`scene_not_found` 的 scene＝幕序號，皆為編號不是數量，不改；`renameConfirm` 的 from／to 是名字）；再逐鍵讀 de／es／fr／pt-BR／ru 原文，判斷字面是否隨數量變。
- zh-TW／zh-CN／ja／ko 無單複數，全部不動。ru 用「名詞（複數屬格）: {n}」句型的鍵可讀成省略「數量」的標籤，任何數字都合文法，不改（含 unsavedChanges／unsavedLeaveConfirm）；這只限本案列出的鍵，不推廣成「有冒號都免改」。ru 只改數字直接修飾名詞的兩鍵。

要改的鍵與語系：

| 鍵 | 數字參數 | 改的語系 |
|---|---|---|
| cliLoginCooldown | secs | en de es fr pt-BR |
| worldbookCharacterCount | n | en de es fr pt-BR |
| aiGalleryLoadMore | n | es fr pt-BR（restante(s)） |
| worldbookDuplicatesSkipped | d | en de es fr pt-BR |
| worldbookDedupeDone | n | en de es fr pt-BR |
| ledgerStatsRejected | n | en de es fr pt-BR |
| ledgerStatsClamped | n | es fr pt-BR |
| ledgerStatsErrors | n | en es fr pt-BR |
| ledgerStatsJumps | n | en de es fr pt-BR |
| refactorPartialFailed | n | es pt-BR |
| refactorSummaryCharacters／Mechanisms／Entries | n | en de es fr pt-BR |
| refactorApplyDoneCharacters／Entries／Deleted／Mechanisms | n | en de es fr pt-BR |
| refactorDroppedSection | n | en de es fr pt-BR |
| refactorUnabsorbedSection | n | en de es fr pt-BR |
| unsavedChanges、unsavedLeaveConfirm（後半代名詞同步） | n | en de es fr pt-BR |
| worldbookImportDone | n | en de es fr pt-BR ru |
| undoLastImportKept | n | en de es fr pt-BR |
| readOnlySkipped | count | en de es fr pt-BR ru |
| usageHitObserved | rounds | en de es fr pt-BR |

不需要改的數字鍵：worldbookCount、lobbyTableCount、castArchiveAria（標籤／逗號句型）、openingChoiceItem 與 scene* 系列（序數）、openingTranslateAllProgress、refactorParallelStep、smartFreeDailyLeft（分數）、usageSavedHeadline（百分比）、versionStoreTotal、backupStoreTotal（size 是容量字串）；de 的 aiGalleryLoadMore（verbleibend）、ledgerStatsClamped、ledgerStatsErrors、refactorPartialFailed 與 fr 的 refactorPartialFailed 字面不隨數變。

## (b) 新手連線說明對齊 zh-TW 精簡版〔作者裁決 2026-10-03〕

- 檔案 `src/i18n/features/openrouter-onboarding.ts`。zh-TW 不動。
- 九語系改 intro、freeNote、browserHint、cliHint 四鍵，逐鍵對 zh-TW：intro＝「登入一次就能開始聊天」、freeNote＝「第一次自動用可用的免費模型」、browserHint＝「登入完成後自動接續」、cliHint＝「想用 CLI？到 設定 → AI 連線 切換」。title、manualIntro 原本就與 zh-TW 對等，不改；按鈕、狀態字、err* 長短語意都對等，不改。
- 各語系沿用該檔原本的人稱（fr vous、ru вы），與主字典的 tu／ты 不一致屬既有狀況，不在本案處理〔模型判斷·未裁決〕。

## (c) 原生確認窗引號被擠下行

- 原因：macOS 原生對話窗（plugin-dialog）在 CJK 名字與西式收尾引號之間會斷行。
- 做法：西式引號包變數處，引號內側兩邊插 U+2060 WORD JOINER；fr「« {x} »」內側空白換 U+00A0，並同樣加 WJ（NBSP 前若是空白或連字號仍可能斷行，WJ 才擋得住），「» ?」的空白也換 NBSP。原始碼一律寫 `\u2060`／`\u00a0` 跳脫。CJK 括號「」由禁則處理，不動。fr.ts 其他鍵既有 19 個直接寫入的 NBSP 不改成跳脫〔模型判斷·未裁決〕。
- 範圍（走原生 confirm 且有引號包變數的鍵）：`deleteCharacterConfirm`、`deleteTableConfirm`、`renameConfirm`、`undoLastImportConfirm`、`worldbookDeleteConfirm`；語系 en、de、es、fr、pt-BR、ru、ko、zh-CN 中用西式引號者。其他原生對話窗字串（標題 `{version}` 等）沒有引號包變數。
- 原生驗證：獨立 Swift 腳本直接建 NSAlert（不開 app、不碰資料目錄），代入修前／修後的 de、fr 實際字串，名稱往前加長到收尾引號落在行界，量行首並截圖。de 修前在 CJK、混合、emoji、結尾連字號四種名稱都重現引號孤行，修後全部黏住；長英文名不受影響；fr 修前修後都沒重現。

## (d) 刪舊 onboarding 鍵

- 十份主字典的 `onboardTitle`、`onboardIntro`、`onboardStep1`／`1Btn`／`2`／`3`／`3Btn`、`onboardCost`、`onboardSaveBtn`、`onboardCliHint` 全刪，連同 zh-TW 的段落註解。
- 已確認零引用：src／scripts／src-tauri 無靜態或樣板組鍵（唯一動態組鍵是 `be_${code}`）；`check-i18n.mjs` 的 `fr:onboardSaveBtn` 白名單一併刪。

## 驗收

- 單元測試（`src/shared/ui/i18n-plural*.test.ts`、`native-dialog-quotes.test.ts`；i18n/ 底下依結構規範不放測試）：en 1/2、ru 0/1/2/5/11/21/22/25/小數、pt-BR 0/1/2、fr 0/1/2、數字字串、缺參數／非法值、多個 plural、分支含一般佔位符、代入值含 `{}`／`#`、文法拒絕各類、體檢佔位符清單與最寬分支、features 字典經 `t()` 展開；五個 confirm 所有引號位置（含 rename 重複 `{from}`）、fr NBSP 碼位。
- `npm run verify` 全綠。

## 結論（2026-10-03 結案）

- Sol 驗收同意（b794d63 複驗簽收，無剩餘必改）。`npm run verify` 全綠：vitest 654、cargo 773、harness 28、check:i18n 全 OK。
- NSAlert 原生驗證：de 修前在 CJK、混合、emoji、結尾連字號名稱都重現收尾引號孤行，修後全部黏住；長英文名不受影響；fr 修前修後都沒重現。
