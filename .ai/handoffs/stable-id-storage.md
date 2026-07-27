# Handoff: stable-id-storage

## Current state
實作完成並通過主線靜態驗證（cargo 104 綠／clippy 0 error／fmt 乾淨／npm build exit 0），**尚未實跑過 app**——GUI 驗收清單見 Next action。下方「實作計畫」保留為規格原文，實際落地與它的差異列在 Completed。

## 實作計畫（2026-07-28 主線）

### 目標資料格式

```
worlds/<world-ULID>/
  state.json                    { id, name, model_bindings(key=角色 id), current_scene, ... }
  world.md
  worldbook.json                visibility.characters = [角色 id]
  characters/<char-ULID>.md     frontmatter 首行加 id:
  characters/<char-ULID>.png            全身圖
  characters/<char-ULID>.avatar.png     頭像
  gen-gallery/<char-ULID>/<unix_ms>.png ← 移進世界目錄（順手修掉「放錯層」bug）
  transcript/<scene>.jsonl      { ts, speaker_id, speaker_name, kind, text }
```

config.json `preferences.last_world` 存 world id。

- ULID 用 `ulid` crate（拍板）。新增 `data::new_id() -> String`。
- `speaker_id`：角色事件存角色 id；GM 旁白／系統訊息與玩家發言存空字串（`kind` 已足以區分）。
  `speaker_name` 是**當下顯示名快照**，改名後舊事件不動——這是既有拍板行為，測試要守住。

### data.rs

- `WorldState` 加 `id: String`、`name: String`，**必填**（不給 `#[serde(default)]`、不用 `Option`）。舊桌解析失敗即略過。
- 新 `pub struct WorldMeta { id, name }`；`list_worlds -> Vec<WorldMeta>`，逐桌讀 `state.json`，**解析失敗跳過該桌**（`eprintln!` 一行即可，不做 UI 提示——拍板「不寫偵測」）。排序邏輯（last_active）不變。
- `CharacterMeta`／`CharacterCard` 加 `id: String`；`serialize_character` 寫出 `id:` 行；`parse_frontmatter` 解析 `id`，**缺 id 視為解析失敗**，`list_characters` 略過該檔（其餘欄位規則不變）。
- 定址全面換手：所有 `world: &str` → `world_id: &str`、角色 `name: &str` → `character_id: &str`。
  `character_path` = `worlds/<world_id>/characters/<character_id>.md`。
  `gallery_dir(root, world_id, character_id)` = `world_dir(...).join("gen-gallery").join(character_id)`。
- `validate_name` 刪掉，換 `validate_id(&str)`：只接受 26 字 Crockford base32（`0-9A-HJKMNP-TV-Z`），擋掉一切路徑逃逸。所有用 id 組路徑的地方都要先過它。
- 顯示名的唯一限制：`validate_single_line`（frontmatter 逐行解析，換行會壞檔）。**同名不再擋、`/` `\` `.` 開頭、GM／玩家保留字全部放行。**
- `create_world(root, name) -> DataResult<String>`：mint id、建目錄、寫入含 id/name 的 state.json，回 id。
- `rename_world(root, world_id, new_name)`：讀 state.json 改 `name` 寫回，**不碰目錄**。
- `rename_character(root, world_id, character_id, new_name)`：讀卡改 `name` 寫回，**不碰任何檔案路徑**。
  → 刪掉 `rename_in_transcripts`、`rename_in_worldbook`、`model_bindings` 搬 key 那三段，整個函式收斂成三行。
- `write_character(root, world_id, card)`：`card.id` 空即回 Err（id 由前端先跟 `new_id` 要，見下）。
- `reorder_characters(root, world_id, ids: &[String])`。
- `create_sample_world` 冪等判斷改成：`list_worlds` 裡有同名（該語系範例桌名）就直接回那桌 id。範例桌的 transcript 種子事件 `speaker_id` 空、`speaker_name = "GM"`。
- `delete_character` 三個附屬路徑照新格式刪（卡片／png／avatar.png／gen-gallery）。

### import.rs

- `import_character`：mint 新 id，`name` 照卡片原值（不再 `validate_name`，改 `validate_single_line`）。
- 圖檔存取函式參數換成 `character_id`。

### transport.rs

- `assemble_messages`：自己講的話判斷改 `event.speaker_id == card.id`（順帶解掉同名角色互相認錯的問題）；行文仍用 `event.speaker_name`（LLM 看名字）。
- 世界書可見性判斷改 `ids.iter().any(|id| id == &card.id)`。
- `pick_speaker`／`suggest_instruction` 維持吃名字（LLM 只認名字），不動。

### lib.rs（Tauri commands）

- 參數一律改名：`world` → `world_id`，角色 `name`／`character` → `character_id`（前端 invoke 傳 `worldId`／`characterId`）。**刻意改名**：前端若還誤傳名字會直接壞給你看，不會靜默寫錯資料。
- 新增 `new_id() -> String`：前端開「新角色」編輯器時先要一個 id，草稿期生圖就能落到正確的 gen-gallery，存檔用同一個 id。
- `generate_character_image(world_id, character_id, name, description, extra_prompt, source)`：`name` 只進提示詞，`character_id` 決定圖庫路徑。
- `gm_suggest_speaker` 回**角色 id**：後端用 `pick_speaker` 拿到名字後對回 roster（同名取第一個），玩家哨兵原樣回傳。
- `list_worlds` 回 `Vec<WorldMeta>`。
- `create_world` 回新 id。

### App.tsx

- `worlds: WorldMeta[]`；側欄顯示 `name`、值用 `id`。
- `characterImages`／`characterAvatars` 的 key 改角色 id。
- `mainView` 的 `name` 欄改 `id`，標題文案另從 meta 查顯示名。
- `speaker` state 存 id；送出事件帶 `{ speaker_id, speaker_name }`（玩家事件 `speaker_id: ""`、`speaker_name: "玩家"`；GM 同理）。
- 訊息列：顏色／頭像查 `metaOf(event.speaker_id)`，**顯示名一律用 `event.speaker_name`**（改名後舊對話仍顯示舊名）。
- `last_world` 存 id；開機找不到該 id 就退回清單第一桌。
- 移除「名稱不可重複／不可含特殊字元」的擋下與提示，連同 i18n 對應鍵一起刪。

### README

「資料存放」那條補半句：資料夾以代碼命名，桌名與角色名存在檔案內（`state.json` 的 `name`、角色卡 frontmatter 的 `name`）。

## 測試清單（cargo，取代既有以名字定址的測試）

1. `create_world` 回 id；`state.json` 含 id/name；`list_worlds` 回 `WorldMeta` 且維持 last_active 排序
2. 兩桌同名可並存，各自獨立 id 與內容
3. `rename_world` 後目錄路徑不變（斷言舊路徑仍在），只有 `state.json` 的 name 變
4. 兩個同名角色可並存，各自讀寫互不干擾
5. `rename_character` 後：卡片／png／avatar／gen-gallery 路徑全部不變；transcript 舊事件 `speaker_name` 維持舊名；`model_bindings` 不需改動仍指向同一角色
6. `delete_character` 連帶刪 png／avatar／gen-gallery（且 gallery 確實在世界目錄內）
7. 舊格式資料被略過不炸：無 id 的 `state.json` → `list_worlds` 不含該桌；無 id 的卡 → `list_characters` 略過
8. 顯示名放行：含 `/`、`..`、開頭 `.`、名為 `GM` 都能存能讀；含換行仍擋
9. 路徑逃逸：`world_id = "../x"`、`character_id = "a/b"` 一律被 `validate_id` 擋
10. `reorder_characters` 以 id 排序，封存角色仍接在後面
11. 世界書 `visibility.characters` 存 id；改名後條目不需回填仍可見
12. 匯入 ST 角色卡：產生新 id，name 照原值（含原本會被擋的字元）
13. `create_sample_world` 冪等：連呼兩次只有一桌
14. `assemble_messages`：同名兩角色，各自只把自己的台詞當 assistant

前端：`npm run build` exit 0。

## Completed
- **Rust 端**（`Cargo.toml` 加 `ulid = "3"`，唯一新依賴）：`data.rs` 定址全改 id（`new_id`／`validate_id` 取代 `validate_name`／`WorldMeta`／`WorldState{id,name}` 必填／卡片 frontmatter 加 `id`／`gallery_dir` 移進世界目錄／`list_worlds`、`list_characters` 解析失敗靜默略過並 `eprintln!`）；`import.rs` 匯入時 mint 新 id；`transport.rs` 自己講話判斷改 `speaker_id == card.id`、世界書可見性比對改 id、行文仍用 `speaker_name`；`lib.rs` 全部 command 參數改 `world_id`／`character_id`，新增 `new_id`，`gm_suggest_speaker` 回角色 id。
- **前端**：`worlds` 改物件陣列（顯示 name、值用 id）、圖與頭像 Record 改以 id 為 key、`mainView`／`speaker`／`last_world` 全改 id、事件帶 `speaker_id`＋`speaker_name`（訊息列顯示一律讀快照名）、名稱限制與三個錯誤訊息 i18n 鍵刪除、改名提示文案重寫。
- **主線收尾精簡**：刪掉沒有呼叫者的 `rename_character`（後端函式＋command＋handler 註冊）——改名就是存一次卡片，相關測試改走實際路徑；順手刪 `write_character` 裡被 `validate_id` 涵蓋的重複空值檢查。
- README「資料存放」補一句：資料夾與檔名是代碼，名字存在檔案裡。
- **空桌回收加上「改過名就不回收」**（GUI 實測揪出，既有行為非本次改壞）：原本條件只有零訊息＋零角色＋world.md 空白，改過桌名的空桌照樣被收掉。改在前端 `reclaimIfUntouched`——名字還是自動名（`newTableName` 或 `newTableName N`）才呼叫回收；判斷放前端是因為自動名是語系字串，後端不知道使用者語言。比對不到時的失敗方向是不刪，不是誤刪。

### 與計畫的差異（執行者自行拍板，可回頭改）
- `export_transcript_markdown`／`export_scene_markdown` 的標題改成內部 `read_state` 取顯示名（原本吃 `world` 字面值當標題，改代碼後不成立）。
- `read_state` 遇 `state.json` 不存在直接回 Err，不再 `Default` 回退（新流程一定會寫這檔，缺檔＝壞資料）。
- 玩家事件的 `speaker_name` 存當下語系的「玩家／Player」字樣；`gm_suggest_speaker` 的玩家哨兵仍是語言無關的 `"玩家"`。
- 改名確認框保留，文案配合新行為改寫。2026-07-28 拍板不刪：改名本身雖已無風險，但「已送出的對話仍顯示舊名」這句仍為真，且 character-card-avatar-issues 實測過灰字提示會被漏看，才改成確認框。
- `list_worlds` 同時間戳的 tie-break 從目錄名改成「顯示名 → id」。

## Verification
- `cd src-tauri && cargo test`：`104 passed; 0 failed`（基線 99）。測試清單 14 條全部有對應測試，主線逐條核對過名稱：#1 `create_world_returns_id_with_state_and_meta`／#2 `two_worlds_with_same_name_coexist_with_independent_ids`／#3 `rename_world_keeps_directory_and_changes_only_name`／#4 `two_characters_with_same_name_coexist_independently`／#5 `rename_keeps_paths_and_preserves_transcript_snapshot`／#6 `delete_character_removes_card_images_and_gallery`／#7 `legacy_cards_and_worlds_without_id_are_skipped`／#8 `display_names_allow_special_characters_but_reject_newlines`／#9 `validate_id_rejects_path_escaping_ids`／#10 `reordering_characters_by_id_keeps_unlisted_after_listed`／#11 併在 #5 的世界書斷言／#12 `importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact`（import.rs）／#13 `sample_world_is_ready_to_play` 的冪等斷言／#14 `assemble_messages_uses_speaker_id_not_name_for_same_named_characters`（transport.rs）
- `cargo clippy --all-targets`：0 error；5 個 warning 主線用 `git stash` 前後對照確認全是改動前既有（`data.rs:47`／`import.rs:219,227`／`lib.rs:510,679`）
- `cargo fmt --check`：本次改的四個檔零 diff（殘留 diff 只在 `cli.rs`／`install.rs`，改動前即違規，未動）
- `npm run build`：exit 0，`✓ built in 450ms`
- **前後端介面對接**：兩包分開實作沒一起跑過，主線改用機械比對——從 `lib.rs` 抽出每個 `#[tauri::command]` 的參數名，對照 `App.tsx` 每個 `invoke` 的 key，結果「所有 invoke 參數完全對得上」；唯一沒被前端呼叫的 command 是 `write_state`（改動前就沒人用，不在本案範圍）。

## Remaining
- **GUI 實測（沒做過，只有靜態驗證）**：清單見 Next action。app 從頭到尾沒被跑起來過，第一次啟動若炸在意料外的地方，最可能的位置是開桌流程（`enterTable` → `read_state`）。

## Next action
1. 使用者三分鐘 GUI 實測（**開 app 前先刪掉 `~/Documents/TableTavern`**，拍板不做遷移；`config.json` 不用刪，開機找不到舊桌會自動退回第一桌）。`npm run tauri dev` 要重啟才吃得到 Rust 改動：
   - 建兩個同名角色 → 側欄兩張卡、發言者選單兩列、各點一次確認講話的是對的那位
   - 改角色名 → 全身圖／頭像／生成圖庫都還在，舊對話仍顯示舊名
   - 改桌名、桌名輸入含 `/` 的字串 → 存得下去、重開仍在
2. 併驗：sponsor-features 的生成圖庫實測、character-card-avatar-issues 的改名提示複驗

## Constraints
- 改名後舊對話仍顯示舊名（2026-07-27 拍板），新設計以 `speaker_name` 快照守住。
- 側欄卡片高度差（有圖 69px／無圖 44px）是刻意的產品決策，重構不要動到。
- 不寫舊資料遷移、不寫舊資料偵測（拍板）；舊桌／舊卡靜默略過。
