# hand-built-card-interface-prompt — 做法

## 問題
匯入卡後跳「這張卡自帶介面畫面。開桌後點聊天上方的『卡片介面』就能整面打開」，但該桌沒有「卡片介面」鈕。驗收：只有真的帶卡片介面的卡才跳；手建卡與只帶 regex 的卡不跳。

## 重現（2026-10-27，測試通道，全新 root、不打 AI）
| 匯入 | 提示 | 「卡片介面」鈕 |
|---|---|---|
| `WestFantsy.png`（replace_string 內含整頁 HTML） | 跳 | 有（對照組） |
| `WestFantasy_NOUI.png`（1 支顯示 regex、replace 不含頁面） | 跳 | 無 |
| `clean-hero-roster.json`（2 支顯示 regex、replace 不含頁面） | 跳 | 無 |
| 同桌匯入 NOUI 後手建角色「手建測試」並存檔 | 存檔時沒有任何對話窗 | — |

## 根因
- 提示出口只有 `tellAboutInterface`（`src/features/import/useImportController.ts:306-321`），條件 `mine.scripts.length > 0`。`scripts` 是後端 `is_display_script`（`src-tauri/src/import/interface.rs:270`）濾出的所有「啟用、非 promptOnly、placement 含 2」的 regex，任何美化用 regex 都算。
- 鈕的條件是 `shellReady`（`useCardInterfaceController.ts:335`）：要 `findShell` 真的從開場白／樓層文字套完腳本後 `extractShell` 抽出整頁 HTML（`interface-card.ts:219`）。兩邊條件不同，只帶美化 regex 的卡就「提示有、鈕沒有」。
- 手建角色：呼叫 `tellAboutInterface` 的只有 `importAsCharacter`、`importAsWorldbook`、`importWebSave`（useImportController.ts:377／415／507），建卡存檔（CardEditor）不經過；`read_card_interfaces` 也只掃有原卡檔的角色。所以「手建」是描述上的混淆，提示一定來自某次匯入。
- 「排隊延後才跳」不成立：匯入每步都 `await`（`showMessage` 等玩家按掉才往下），沒有延後佇列。

## 做法〔模型判斷·主線裁決 2026-10-27，三方共識〕
提示條件改成「這張卡有一支腳本的替換範本（replace_string）能抽出整頁 HTML」。
1. `src/features/card-interface/interface-card.ts` 加 `cardCarriesPage(card)`：`card.unsupported === null && card.scripts.some((s) => extractShell(s.replace_string) !== null)`。與鈕共用同一個抽殼函式 `extractShell`，但看的是替換範本，不套 find_regex 到開場白。
2. `useImportController.ts` 的 `tellAboutInterface`：第一支分支 `mine.scripts.length > 0` 換成 `cardCarriesPage(mine)`。scrypt／remote_loader 兩支提示照舊（不受影響）。
3. 測試：`interface-card.test.ts` 加純美化 regex／整頁 HTML／圍欄包頁面／unsupported 四案，外加「範本有頁面、開場未命中」一案（預期 true，對應已知限制）；新檔 `src/features/import/card-interface-notice.test.tsx`：美化卡不跳、整頁卡跳、加密（scrypt）與雲端載入器卡提示保留。手建不測：它本來不經 `tellAboutInterface`，匯入測試鎖不住。
4. 不動後端、不動 `openIfDrawable`。

已知限制（接受，罕見邊際輕防〔模型判斷·主線裁決〕）：
- 範本有頁面、但 find_regex 對不上開場白時，提示先跳，鈕要等模型輸出才出現。
- 後續腳本把頁面移除、或 regex 無效時，同樣可能提示與鈕不一致。
- 整頁 HTML 只在模型輸出時才由 `$1`／`{{match}}` 帶出、範本本身不含頁面起點的卡，匯入當下不預告。
- TestCards 帶 regex 的卡（Grok 重掃 19 張）靜態抽殼與套上 first_mes 的結果沒有不一致。

## 預計改動
| 檔案 | 現行 | 預估 |
|---|---|---|
| `src/features/card-interface/interface-card.ts` | 540 | +6 |
| `src/features/import/useImportController.ts` | 712 | ±2 |
| `src/features/card-interface/interface-card.test.ts` | — | +30 |
| `src/features/import/card-interface-notice.test.tsx`（新） | — | 約 120 |

不碰 present-name-interface-lang（StateBar／AppWorkspace）與 card-shim-no-zod（card-interface controller、mvu shim）在改的檔。

## 業界做法與差距
- Marinara Engine（原始碼唯讀）：匯入 ST 卡時把內嵌 regex 靜默轉成腳本，只讓使用者選範圍（角色／全域，`ImportCharacterModal.tsx`、`st-character.importer.ts`），沒有「這張卡帶介面」的提示，也沒有整頁殼的概念。
- SillyTavern（憑記憶，未查原始碼）：匯入帶內嵌 regex 的卡時問「是否允許這些 regex」，是安全同意，不宣稱有介面。
- 差距：兩者都不對使用者預告「有介面」，我們是特有功能，提示要準；做法是讓提示與鈕共用抽殼函式，而不是拿掉提示。
