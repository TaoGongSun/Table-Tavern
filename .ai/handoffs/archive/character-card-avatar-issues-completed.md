# Completed: character-card-avatar-issues

（自交接檔搬出的已完成項目與驗證證據；現場狀態見 [../character-card-avatar-issues.md](../character-card-avatar-issues.md)）

## 使用者原文（2026-07-27）
1. 人物有頭像後，這個頭像和文字太近了，讓他間隔多一點
2. 角色卡不能改名，要有地方改名
3. 移除頭像，編輯人物圖像等沒有被算進未儲存變更
4. 移除頭像沒有出現警告提示，例如「會變回 emoji」
5. 點擊建卡下方會跳出 invalid name: ""，沒有辦法建卡

追加：「不應該是有個新角色名稱的輸入框，是點建卡之後右邊換成角色卡介面，在那邊輸入名字和資料。」

## Completed
- **第 1 項間距**（App.css）：`.avatar-round` 加 `margin: 2px`（黑框是 box-shadow 不佔版位）；`.opt-target` gap 0.4→0.65em；`.card-editor-avatar` margin-bottom 0.25→0.6rem；角色卡名牌 `margin-left` −6px→**0**（使用者實測要求：不壓圖窗也不留空隙）。圓頭像卡看起來仍有間距＝圖窗本身的留白，非名牌間距。
- **第 5 項＋名稱檢查**：`characterNameError(name, taken)`（App.tsx:133）規則對齊後端 `validate_name`（data.rs:197），建卡／改名共用；擋空名、`/`、`\`、開頭 `.`、`..`、控制字元、GM／玩家保留字、**同名**（原本同名會直接覆寫既有卡片＝資料遺失）。
- **第 3 項圖像暫存**：`draftImage`／`draftAvatar`（undefined 沒動過／null 標記移除／`{bytes,url}` 待存），加入／更換／裁頭像／移除全部只改記憶體，按儲存才落地；未儲存計數與離開確認比照世界設定頁。CropDialog 改回傳 `{bytes,url}`（bytes 存檔、url 預覽）。生圖後同步 `gen_prompt` 到記憶體與存檔快照，避免按儲存蓋回舊值。
- **第 4 項移除頭像警告**：confirm「會變回原本的 emoji 圖示」。
- **建卡新流程**：側欄只剩「建卡」「匯入卡」兩鈕；`mainView` 新增 `new-character`；CardEditor `name: string | null`（null＝草稿，讀空白卡不打後端）。草稿模式關閉 AI 生圖（後端生圖要讀已存檔設定，圖庫目錄也以角色名建立）與隱藏「隱藏角色」鈕。存檔後畫面停在該卡、speaker 跟著換（`finishCardSaved`）。
- **第 2 項改名**：後端 `rename_character(world, from, to)`（data.rs）搬檔（卡片 md 含 frontmatter name／全身圖／頭像／`gen-gallery/{名}` 目錄）＋回填（劇情紀錄全部幕的 speaker、世界書 `visibility.characters`、`state.json` 的 `model_bindings` key）；擋同名碰撞與 `validate_name`。自然語言內文（world.md／世界書內文／public_md／private_md／對話正文）**不動**——機械取代會誤傷同名詞句。
- **刪除入口補齊**：角色卡編輯畫面加「刪除角色」鈕（與側欄隱藏區共用 `deleteCharacter`＝確認框＋善後，`finishArchiving` 更名 `finishRemoval`）；`delete_character` 一併清 `gen-gallery/{名}`（原本留孤兒檔，建同名角色會撿到舊圖）。桌列表每列加 ✕ 刪桌，後端 `delete_world` 整包清世界資料夾＋放在 `worlds/` 外的圖庫；刪掉最後一桌自動補範例桌。
- **GM 卡書皮方案**（fable，commit 7597061，僅 App.css）：世界書要一眼看出不是角色卡——整張卡改深皮革底＋內縮 4px 燙金內框＋銅金名牌，書本小圖沿用不換（Codex 生圖任務取消）。皮革色抽成各主題 `--gm-leather` token 混 `--surface-2` 再疊打光漸層：三個深色主題深皮革、四個淺色主題淺一半（使用者拍板：有顏色壓過白卡即可）、贊助五主題各自跟主配色換皮（琥珀／苔綠／蜜蠟／酒紅／板岩藍）；⚙ 用 `var(--ink-1)` 跟明度走。高度改 `min-height: 4.3125rem` 對齊有圖角色卡的 69px（使用者試過 92px 嫌太高後拍板）。注意 `.tcard-gm` 必須排在 `.tcard` 基底之後（同權重靠順序蓋底色）。

## Verification
- `cargo test` 94 綠（基線 91，+3：`rename_character_moves_files_and_backfills_references`、`rename_character_rejects_collision_and_invalid_names`、`delete_world_removes_directory_and_gallery`）
- `cargo clippy --all-targets` 無新增警告（既有 5 個都在 data.rs:47／import.rs／lib.rs:461,632）
- `npm run build` exit 0
- 第 1 項間距：另一 session 以編譯產物做 before/after Playwright 截圖對照
- GM 書皮：`npx tsc --noEmit` exit 0；七主題以 dev server 注入卡片 DOM 合成對照圖逐一驗色，高度以 getBoundingClientRect 量測 GM=狐狸=69px；使用者在 dev app 實際切七主題驗收通過（sponsor 主題閘門曾以 `import.meta.env.DEV` 暫時旁路，驗收後已還原，App.tsx 未進 commit）
- 2026-07-27 使用者實測：桌列表分隔線、角色圖示 emoji 欄位、桌刪除鈕、角色刪除鈕、建卡新流程、GM 書皮卡（七主題）全部通過
