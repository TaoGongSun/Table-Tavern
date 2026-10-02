# backend-zh-messages 規格

範圍〔作者裁決 2026-10-02〕：A＋B＋C 類改成「後端回代碼、前端 i18n」；D 類本案不做，之後另立案。

## 做法（Sol 審過；Grok 額度用完，本案只經 Sol 審〔作者裁決 2026-10-02〕）

**代碼格式**：後端新模組 `ui_msg.rs`，`enum UiMsg`（`#[serde(tag = "code", rename_all = "snake_case")]`，參數是具名欄位）。`Display` 輸出 `TTMSG:` 接一段 JSON。錯誤仍走既有 `Result<_, String>`／`invalid_data(...)`，不改指令簽名。代碼一旦落檔就是持久契約，不得改名。
- 包裹別人錯誤時用帶 `error` 欄位的變體（`UiMsg::X { error: inner.to_string() }`），不再留中文外殼；巢狀翻譯只認名為 `error` 的欄位，其餘參數（路徑、檔名、供應商 body）一律原文。
- `AI_*` 前綴必須留在字串最前面，不得被包進 UiMsg 參數；前綴後面的中文說明改成不帶語言的 `status=… body=…`。生圖暗號 `REFUSED`／`NO_IMAGE` 不動。

**前端**：
- `src/shared/ui/backend-text.ts`：`backendText(raw: unknown): string` 只做顯示。掃描器理解 JSON 字串（引號、跳脫、巢狀），同串可多個標記、壞標記後的好標記照解，遞迴深度上限 3；解析後驗證物件、已知 code、必要參數與型別，不合法就整段原文保留。字典在 `src/i18n/features/backend-msg.ts`（十語系，鍵 `be_<code>`）。`backendCode(raw)` 只認字串起首（容許 `Error: `）的那個標記，供程式判斷用。
- **state 一律存後端原文，render 時才 `backendText`**；`explainAiError` 照舊吃原文，`ErrorNote` 的主文字與小字、`CardEditor` 的小字都走 `backendText`。
- 前端拿中文比對的地方（`App.tsx` worldBusy、`version-center.ts` 兩句、`VersionTab.tsx` cannotReplace）改用 `backendCode`，跟對應的後端改動同包切換。
- `t()` 參數代入改成對模板單次替換（現在逐參數 split/join，先代入的值含 `{名}` 會被再換）。
- `check-i18n.mjs`：接上 `features/` 字典；從 `ui_msg.rs` 抽出實際 serde code（抽取失敗就擋），核對十語系 `be_*` 鍵一一對上、模板佔位符與 Rust 欄位一致；A 類 reason 鍵一併核對。

**A 類**：`NeedsRepair { message }` 改 `NeedsRepair { reason, error? }`（`outside`／`rename`／`convert`／`missing`／`io`，`io` 帶系統錯誤原文）。連帶改 `Recovered::Repair`、前端 open-world 型別、App state、FormatNotice 與測試。鍵 `needsRepair_<reason>`。

**C 類**：重構審閱 detail、帳本接管／跳過 detail、`（官方別名）`、`(CLI 預設)`、「自動檢查已關閉」（`CheckResult::Failed.message`）存代碼字串，顯示處套 `backendText`；舊檔中文原樣顯示。帳本 detail 送重構 AI（`refactor_ai/context.rs`）前，在後端從字串解析成英文短句（`UiMsg::ai_text_from_str`）；舊中文、未知或壞碼原樣送。

**已知限制**：降回舊版時，已落檔的代碼會以 `TTMSG:` 原文顯示。

**不在範圍**：只送 AI 的機制擋下紀錄（`mechanism/apply.rs` 等，經 `state.notes` 進提示詞）歸 E 類；只寫進 log 的 `—（這條路不回報快取）` 歸 G 類。

## 分包

1. 地基＋A 類：`ui_msg.rs`、`backendText`／`backendCode`、`t()` 單次替換、i18n 檔與 check-i18n 對照、顯示錯誤處改成 render 時翻譯、修復頁原因代碼。
2. B 類桌資料面：`data/`、`import/`、`refactor/`、`receipts`、`commands/scene|refactor|image`，含 App.tsx worldBusy。
3. B 類更新器：`updater/`、`commands/update|versions`，含 version-center／VersionTab 判碼。
4. B 類 AI 連線面：`ai_transport`、`transport/client`、`responses_transport`、`smart_free`、`cli/`、`lanes`、`session_file`、`evaluator`；逐條確認會不會到畫面，只進 log 或只進 AI 的不改。
5. C 類。

每包：`npm run verify` 全綠＋該包的反例測試（解析器反例、翻譯前後 AI 分流不變、更新器判碼、新舊 detail 混存、AI 脈絡）；包 1、3、4、5 加做 zh-TW 與 ru 畫面抽查。

## 盤點：後端非測試程式碼裡含中文的字串（共 617 行）

| 類 | 筆數 | 內容 | 本案處理 |
|---|---|---|---|
| A | 4 | 需修復整頁說明 | 改（核心） |
| B | 273 | 指令失敗時原樣回前端的錯誤字串（`Err("…")`、`invalid_data("…")`）：更新器、CLI／API 連線、換幕、匯入、圖片、設定檔、桌檔讀寫、session 檔、算式求值等 | 改 |
| C | 55 | 畫面上顯示的非錯誤說明：重構結果審閱的 detail、機制帳本接管／跳過的 detail、`(CLI 預設)`、`（官方別名）`、`自動檢查已關閉`（細分後部分改歸 E／G，見「不在範圍」） | 改 |
| D | 13 | 寫進桌資料或匯出檔的文字：跑團紀錄匯出標題、`【前情提要】`、`（角色回歸）`／`（人物登場）` 前綴、角色卡檔 `## 公開`／`## 私有` 段標、匯入卡時的段標（簡介、人格與語氣…） | 本案不做，另立案〔作者裁決 2026-10-02〕 |
| E | 81 | 送給 AI 的提示詞 | 不改〔模型判斷·未裁決〕：不是介面文字，已另有語言指示 |
| F | 24 | 解析 AI 輸出／卡片內容用的多語關鍵字（`標題：`、`下一位`、`狀態欄`…） | 不改〔模型判斷·未裁決〕：是比對用，不是顯示 |
| G | 167 | 日誌、`expect`／`unreachable`、內嵌測試 | 不改〔模型判斷·未裁決〕：玩家看不到 |

前端已有三處拿後端中文字串做比對（改 B 類就得一起改）：`src/App.tsx` 的「這張桌正在處理中，請稍候再試」、`src/features/updater/version-center.ts` 的「要更新的版本已經換了」「無法自動替換」。

### 完整位置（行號以 main cbbcd9f 為準）

#### B 錯誤字串
- `ai_transport.rs`：12,26,119,130,133,318,324,327,462
- `cli/runner.rs`：96,160,217,239,250,258,274,340,344,354,358,376
- `cli/stream.rs`：269,317,318,348
- `commands/chat.rs`：458
- `commands/image.rs`：223,229,231,232,236,241,244,266,332,433,435,438,439
- `commands/refactor.rs`：28,351,435,507
- `commands/scene.rs`：244,304,307,313,319
- `commands/update.rs`：171,174,228,239,272,274,277,281
- `commands/versions.rs`：11,31,45
- `data/config.rs`：52,78,114,134,143,213,216,219,227
- `data/format/backups.rs`：79,81,83,86,90
- `data/format/commit.rs`：134,139,142,145,243,302,303,309,314,535,551,619,675,678
- `data/scene/export.rs`：58,78,112
- `data/scene/lifecycle.rs`：48,52,153,157,187,192,196
- `data/world.rs`：242
- `data/world_file.rs`：119,132,139,189,201,233,243,259,262,266
- `data/world_lock.rs`：16,290
- `data/worldbook.rs`：515,517,524,571,575
- `evaluator.rs`：88,125,213,221,305,343,387,415,417,439,455,466,474
- `import/card.rs`：111,117,268,287
- `import/card_io.rs`：14,20,22,35,40,50,53,60
- `import/export.rs`：172,178,184,186,207
- `import/images.rs`：86,90
- `lanes.rs`：165,420,512
- `openrouter_oauth.rs`：242,246
- `receipts.rs`：550,556
- `refactor/apply.rs`：36
- `refactor/interface.rs`：59,91,109,130,177,191
- `responses_transport.rs`：153,191,223,244
- `session_file.rs`：40,72,79,83,109,111,116,128,132,137,141,158,166,170,182,185,193,200,202,206,213,224,228,236,239,243,248,256,258,262,265,268,293
- `smart_free/mod.rs`：93,100,113,133
- `transport/client.rs`：53,422,447,523,531,605,645
- `updater/catalog.rs`：96,103,105,108,111,183,186,190,200
- `updater/check.rs`：8
- `updater/launch.rs`：30,36
- `updater/macos.rs`：16,150,153,176,180,186,233,242,277,281,284,291
- `updater/residue.rs`：167,172,203
- `updater/rollback_point.rs`：18,32,35,73,172
- `updater/slot.rs`：5,82,83,84,89,118,124,125,126
- `updater/store.rs`：51,76,86,89,91,99,194,207,209,210,212,283,298,302,345
- `updater/store_lock.rs`：41,66
- `updater/verify.rs`：34,38,39,42,48,49
#### C 畫面說明
- `ai_transport.rs`：384,416,448
- `cli/catalog.rs`：133
- `commands/settings.rs`：128
- `commands/update.rs`：41
- `data/worldbook.rs`：773
- `import/mechanism.rs`：223
- `lanes.rs`：63
- `mechanism/apply.rs`：45,90,98,106,116,124,136,154,166,190,202,220,254,261,268,276,284,293,302,314,322,352,357,373,379,393,399,405
- `mechanism/derive.rs`：83
- `mechanism/ledger.rs`：120
- `refactor/apply.rs`：427
- `refactor_assemble.rs`：222,330,443,447,526,585,662,673,680
- `responses_transport.rs`：218,297
- `transport/client.rs`：374,417,494,582
#### D 資料／匯出文字
- `data/character.rs`：169
- `data/scene/export.rs`：86,93,121
- `data/scene/lifecycle.rs`：29
- `data/scene/presence.rs`：15
- `data/worldbook.rs`：579
- `import/card.rs`：13,14,15,16,17,254
#### E 提示詞
- `cli/request.rs`：34,35
- `commands/chat.rs`：375,377,384
- `commands/scene.rs`：256,331
- `lanes.rs`：379,767
- `refactor_ai/context.rs`：16,19,24,26,34,36,49,53,57,159,163,229,235,243,246,249
- `refactor_ai/expand.rs`：40,43,48,50,76
- `refactor_ai/prompt_common.rs`：27,36,39,47,99,117,118,119,120
- `refactor_ai/rewrite.rs`：63
- `refactor_ai/survey.rs`：103,112,158,187
- `refactor_ai/types.rs`：60,79
- `snapshot_patch.rs`：26,40,49
- `transport/arrivals.rs`：9,112,118
- `transport/assemble.rs`：67,70,126
- `transport/context.rs`：55,61,75,80,86,94,216
- `transport/state_view.rs`：25,53,55,63,64,65,84,98,350
- `transport/turns.rs`：45,46,74,87,106,160,174,184,298,302
#### F 比對關鍵字
- `data/character.rs`：133,134
- `data/world.rs`：307,308,309,310,311,312
- `import/mechanism.rs`：411
- `refactor_ai/context.rs`：172
- `transport/messages.rs`：13,31,39
- `transport/response.rs`：13,14,91,107,240,241,302,336,337,338,421
#### G 日誌／斷言／測試
- `commands/update.rs`：382,389
- `data/character.rs`：233
- `data/world_lock.rs`：385
- `ejs.rs`：669
- `evaluator.rs`：391,425,442
- `import/mechanism.rs`：305
- `inflight.rs`：233,238,272,324,331,335,350,355,361,374,379,388
- `lanes.rs`：1074,1075,1077,1091,1092,1105,1106,1107,1108,1138,1139,1158,1183,1186,1189,1203,1231,1256,1258,1270,1274,1283,1286,1299,1302,1308,1309,1314,1321,1327,1349,1351,1361,1365,1382,1384,1387,1388,1393,1396,1402,1403,1405,1406,1407,1408,1409,1410,1424,1433,1439,1440,1449,1463,1471,1472,1473,1474,1528,1529,1531,1537,1541,1542,1543,1548,1549,1552,1553,1565,1570,1571,1572,1575,1576,1588,1593,1596,1614,1619,1622,1693,1705,1751,1752,1764,1769,1770,1855,1856,1857,1865,1866,1877,1878,1887,1888,1899,1975,1980,1981,1982,1985,1988,1996,2001,2098,2099,2101,2126,2135,2136,2140,2146,2147,2149,2154
- `lib.rs`：187,190
- `refactor_ai/context.rs`：173,183,190,196
- `refactor_ai/result_parse.rs`：187
- `refactor_session.rs`：164,172,173,175,181,185
- `transport/messages.rs`：75
- `updater/check.rs`：22
- `updater/macos.rs`：138,141
- `updater/mod.rs`：75,78
- `updater/post_launch.rs`：30,34,43
- `updater/preview.rs`：48
- `updater/previous.rs`：75,97
- `updater/rollback_point.rs`：48,211,219
- `updater/store.rs`：341
