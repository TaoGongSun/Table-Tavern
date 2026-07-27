# Handoff: character-card-avatar-issues

## Current state
五項＋建卡新流程＋刪除入口全部實作完成（cargo 94 綠＋npm build 綠），等使用者實測。2026-07-27 使用者追加拍板：建卡流程改成「點建卡→右側開空白角色卡編輯器填名字與資料」，取代原本側欄的名稱輸入框；改名不做自然語言內文的機械取代。

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

## Verification
- `cargo test` 94 綠（基線 91，+3：`rename_character_moves_files_and_backfills_references`、`rename_character_rejects_collision_and_invalid_names`、`delete_world_removes_directory_and_gallery`）
- `cargo clippy --all-targets` 無新增警告（既有 5 個都在 data.rs:47／import.rs／lib.rs:461,632）
- `npm run build` exit 0
- 第 1 項間距：另一 session 以編譯產物做 before/after Playwright 截圖對照

## Remaining
使用者實機實測全部五項＋建卡新流程；下列缺陷未修（見 Next action）。

## Next action
1. **生成圖庫目錄放錯層**：`gallery_dir` 落在 `{data_root}/{world}/gen-gallery/{角色}`，但世界資料夾是 `{data_root}/worlds/{world}`——圖庫是 `worlds/` 的兄弟目錄，不在世界裡。刪桌與刪角色都已各自補上清圖庫，但 `rename_world` 仍只搬 `worlds/{名}`＝改桌名後整個生成圖庫失聯。修法是移到世界資料夾內並加一次性搬移（舊路徑存在且新路徑沒有就搬），等使用者拍板。
2. 生圖用的是**已存檔**的 public_md（後端從磁碟讀卡），編輯器裡沒存的描述不會進提示詞——要不要在生圖前擋未儲存變更，待拍板。

## Constraints
- 沿用 character-image-avatar 既有拍板：頭像存正方形 PNG、圓框走 CSS；移除全身圖不連動刪頭像；刪角色清兩檔。
- 編輯畫面按鈕列一律置頂（全 app 統一），例外只有生圖對話框的主要動作放右下。
- **側欄卡片高度差是刻意的**（2026-07-27 使用者拍板）：有全身圖的角色卡 69px、只有 emoji 或圓頭像的 44px。有圖的卡比較高＝比較顯眼，用來讓玩家想要角色圖、進而想用 AI 生圖。程式碼上它看起來像 `height: 100%` 解析不出來的意外（App.css `.tcard-image` 有註記），**不要當 bug 修掉**；GM 卡是用 `aspect-ratio: 2 / 3` 明確對齊到 69px 那一檔。
