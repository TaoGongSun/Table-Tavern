# vn-mode 包 8 方案：VN 作品包（`.ttvn` 匯出、匯入、重播模式、信任邊界）

總綱 [vn-mode.md](vn-mode.md) §14（定案：條件、包什麼、格式骨架、兩種匯入、重播模式、信任邊界、匯出前提醒）、§11 包 8 列、§12 拍板 11–13、§13（玩家名寫死、無防拷貝）；前置 [包 6](vn-mode-pkg6.md)（`SceneLabel.source/keep/cut`、`origin`、`choice_state`、`tip_of`、`scene_high`、內容雜湊已讀；§5 與包 8 的關係）、[包 5](vn-mode-pkg5.md)（`ended`、結局一覽、新周目白名單與失敗刪整桌）、[包 9](vn-mode-pkg9.md)（VN 卡匯入的信任邊界與 `create_world_exclusive`／`discard_new_world`）、[包 9b](vn-mode-pkg9b.md)（卡內地點與額外表情鍵）、[包 7](vn-mode-pkg7.md)（立繪檔案布局）、[包 3](vn-mode-pkg3.md)（背景庫）。

**方案狀態**：主線定案（異議與預設全採納）；Sol 方案審第 1 輪（串 01a1994f）必改 3 條＋建議 1 條已併入（§2.2 第 3 步、§2.5 #11a、§2.6.1、§2.4 第 7 步、§3）。立案估 550（大案）；逐項重估核心約 **2,060**（§6），前幾包實作是方案估的 1.3–1.6 倍，預期實作 2,700–3,300。碰信任邊界輸入（匯入別人的 zip），不論案別方案必送審。建議拆 8a／8b／8c（§6）。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 0 業界做法與工具

| 業界 | 我們 | 差距 |
|---|---|---|
| **容器**：EPUB／ODF／OOXML／Character Card V3 `.charx`＝zip＋固定位置的 manifest＋相對路徑資源；EPUB 另要求第一個條目是不壓縮的 `mimetype` 供嗅探 | `.ttvn`＝zip＋`manifest.json`（`format: "tt-vn-work"`、`version`）＋`script.json`＋`characters.json`＋`assets/`＋選配 `settings/` | 不做 `mimetype` 首條目（只靠副檔名＋manifest 判別，沒有外部工具要嗅探） |
| **zip-slip**（Snyk 2018）：解壓時把條目名接到目的目錄會被 `../`、絕對路徑、`\` 逃逸；做法是 `enclosed_name()` 正規化或白名單 | **條目名從不拼進落檔路徑**：落檔路徑一律由我們驗過的鍵重組（立繪走 `SpriteDir`＋`validate_id`、背景走 `valid_slug`＋時段鍵、封面固定名）；條目名只當查表鍵，另外拒收含 `..`、`\`、絕對路徑、NUL 的名字 | 比 `enclosed_name` 更嚴（結構上不可能逃逸） |
| **zip bomb**：單檔高壓縮比（42.zip 遞迴）、重疊條目（Fifield 2019 non-recursive bomb，多個中央目錄項指向同一段壓縮資料）、標頭謊報大小。做法：限條目數、限每檔與總量的**實際**解壓位元組（`Read::take`）、重複名拒收、不遞迴解壓 | 上限見 §2.5，全部以實讀位元組累計，不信 `size()`；重複條目名拒收；不收內層 zip | 同業界 |
| **連結與特殊項目**：tar／zip 都可能帶 symlink（Unix mode `0o120000`）、裝置檔；做法是拒收 | `unix_mode` 是 symlink 或非一般檔拒收整包；目錄條目略過；加密條目與 Stored／Deflated 以外的壓縮法拒收 | 同；既有先例 `updater/macos.rs:158-205`（tar 解壓擋逃逸與連結，`UiMsg::ArchivePathUnsafe`／`ArchiveHasLink`） |
| **圖片炸彈**（decompression bomb）：先讀檔頭查尺寸、限像素總量與解碼配置再解碼；高安全場景重編去除 metadata | 每張 PNG 走既有嚴驗（`png_image.rs:406` `validate_png_image`：結構、CRC、IHDR、尺寸 8192／2400 萬像素、整條 zlib 串流與掃描線，64 KB 緩衝串流驗、不解碼成點陣）；超限**拒收不縮**（立繪臉框是像素座標，縮了對不上）；ancillary chunk 剝掉只留繪圖必要的 | 不重編（異議 6） |
| **schema 驗證**：版本化格式、型別先行、未知欄位忽略、再做語意與參照完整性驗證（JSON Schema；Rust `serde`＋手寫語意驗證或 `jsonschema`） | `serde` 型別（未知欄位忽略）＋逐欄夾斷＋參照完整性（幕樹、角色、資產）；`version` 不符拒收 | 同包 9，不對外發 JSON Schema 檔 |
| **原子匯入**：先解到暫存目錄驗完再一次搬進正式位置；失敗不留半成品 | `create_world_exclusive` 拿新桌獨占→建完→任一步失敗 `discard_new_world` 刪整桌（包 9、包 5 新周目同模式） | 等價 |
| **大檔不走 IPC**：桌面 app 傳路徑給原生端自己讀 | 前端 `open()` 拿路徑、Rust 讀檔並先查檔案大小；不把幾百 MB 轉成 `number[]` 過 IPC | 既有卡片匯入走 `Vec<u8>`（`useImportController.ts:202-215`），作品包不同 |
| **VN 重播／流程圖**：Ren'Py `Replay`（從已解鎖場景重播、不改存檔）、Steins;Gate／Zero Escape 流程圖（已走路線可跳、未走灰掉） | 重播桌＝同一個 VnPlayer 關掉生成；選項點已走過的可跳、沒走過的灰掉「作者未走過這條路」 | 不畫整張流程圖（v1 只在選項點列） |
| **Rust 工具** | `zip` 4.x（已在依賴樹，經 `tauri-plugin-updater`，`Cargo.lock:6009`，沒開壓縮 feature）升成直接依賴：`default-features = false, features = ["deflate-flate2"]`（只用既有 `flate2`／miniz_oxide，不帶 aes、bzip2、lzma、zstd、zopfli）；讀 `ZipArchive::by_index`、`ZipFile::{name, is_dir, is_symlink, unix_mode, encrypted, compression}`；寫 `ZipWriter`＋`SimpleFileOptions`（PNG `Stored`、JSON `Deflated`）。圖驗證用既有 `import/png_image.rs`、`png_clean.rs`；原子寫用既有 `commit_world_write_atomic` | — |

## 1 現況查證（2026-11-02，分支 vn-mode，commit 797f2b52）

### 1.1 可直接打包的落檔資料

- **事件流**：`worlds/<id>/transcript/<幕>.jsonl`，`TranscriptEvent`（`data/scene/transcript.rs:22-70`）：`text` 是**剝殼後的劇本格式原文**（`[bg]`、對白、`[choices]`、`[chapter]`、`[ending]`），格不另存、讀時解析（`chat_turn/vn_script.rs:454` `read_scene`）。另帶 `raw`（模型原文，含狀態區塊）、`state`（逐則完整 `TableState` 快照）、`gm_only`、`marker`、`opening`、`id`、`message_vars`／`vars_*`、`turn_key`、`action_id`、`ts`。
- **`read_scene` 從快照讀的東西**：第一則的 `state.table` `place`／`time`（背景沿用起點，:464-473）、每則 `state.tree.chapter`（:505-509）、聖經在時的旗標提示（:501-504）；`endings_overview` 逐則找 `tree.ended`（`vn_bible.rs:266-306`）。所以演出包每則要帶最小快照。
- **解析名單**：`VnRoster::load`（`vn_script.rs:116`）用目前角色清單對名字、`card.cast` 的額外表情鍵、`Places::of(card)` 卡內地點。卡內地點會把 `[bg]` 對成 `card:<鍵>` 的 slug（`vn_backgrounds.rs:146-164`）——匯入桌沒有同一份地點表，背景檔名就對不上。
- **幕樹**：`WorldState.scene_labels`（`data/state.rs:501-521` `SceneLabel{base, version, parent, forked, source, keep, cut}`）；沒標籤的幕預設 `parent = scene − 1`（`lifecycle.rs:18-32`）；`next_child` 只認有標籤的幕（`branches.rs:35`）。`scene_high`、`story_previous`、`origin`、`choice_state`、`tip_of`、`switch_scene` 都在 `data/scene/branches.rs`；`choice_branches` 在 `vn_branches.rs:69-120`（回 `{option, tip, ended}`）。`catchup_summaries`、`scene_titles` 在 `WorldState`。
- **立繪**：`vn/sprites/<角色id>/`：`sprite.json`、`body/base.png`（定稿）、`body/<變體>.png`、`face/<表情>.png`、`face-<變體>/<表情>.png`、`candidates/`（未選候選，不打包）（`vn_sprites/store.rs:1-3, 189-215, 252`）；`SpriteMeta` 載入時自驗（框在圖內、變體鍵合格，`store.rs:98-110`）。
- **背景**：`vn/backgrounds/<slug>-<時段>.png`＋`backgrounds.json`（slug→地點原文，只給提示詞與顯示，播放不需要）（`vn_backgrounds.rs:1-4, 283-300`）。
- **角色**：`characters/<id>.md`（公開／私設）、`<id>.png`（卡圖，立繪沒定稿時舞台的退路）、`<id>.avatar.png`（`import/images.rs:45-95`）；id 是 ULID，`validate_id`（`data/paths.rs:6-12`）擋路徑逃逸。
- **封面**：`vn/cover.png`（`data/vn.rs:701`），只有 VN 卡匯入的桌有。
- **`vn.json`**：`VnFile{spoiler_lock, bible, frozen, card, playthrough}`（`data/vn.rs:26-48`）；`Bible.chapters[].title`、`endings[{id,title,when}]`；`card: CardOrigin`（rating、pov、protagonist_fixed、cast 美術、locations，`vn.rs:190-245`、`vn_art.rs`）。
- **不打包的個人／工具資料**：`vn/vn-progress.json`（已讀、位置、存檔，包 6 §5）、`lanes.json`、`world-info/` 計時、`mechanism-log.jsonl`、變數模式控制檔、匯入收據。

### 1.2 依賴與既有驗證

- `zip` 4.6.1 只是 `tauri-plugin-updater` 的傳遞依賴，**沒開任何壓縮 feature**（`Cargo.lock:6009-6017`）；`flate2`、`sha2`、`image` 是直接依賴（`Cargo.toml`）。專案內沒有 zip 讀寫程式；tar 解壓的安全先例在 `updater/macos.rs:158-205`。
- PNG：`validate_png_image`／`classify_png_image`（`png_image.rs:406-416`，`STORED_IMAGE_LIMITS` 8192 邊、2400 萬像素 :27）、`strict_stored_png`（`png_clean.rs:181`，過大會縮、APNG 會剝）、`copy_chunks_before_iend`（:263，可丟指定 chunk）。
- 文字渲染：`DialogueBox` 以 React 文字節點輸出（`DialogueBox.tsx:34`），`features/vn/` 沒有 `innerHTML`——「只當文字」已成立，本包只要不新開 HTML 渲染。

### 1.3 匯入入口與建桌模式

- 卡片匯入：前端讀 `File.arrayBuffer()` 成 `number[]` 過 IPC（`useImportController.ts:202-215`）。VN 卡 `import_vn_card(data: Vec<u8>)`（`commands/vn_card.rs:5-17`）→ `materialize.rs:23-58`：`create_world_exclusive(…, Vn)`（`world.rs:170`）→ `build` → 失敗 `discard_new_world`（`world.rs:338`）；測試掛點 `hook(步驟名)`。
- 新周目 `data/vn_playthrough.rs`：白名單 `CARRIED`（:19-34）逐檔原子複製、失敗刪整桌；角色 id 沿用。
- 逐幕整段寫入：`write_imported_scene`（`data/scene/import.rs:25-88`，網頁存檔用）：重發事件 id、回傳 id 向量；快照＝整幕同一份 `base`（非變數模式），不能寫逐則不同的 `chapter`／`ended`。
- 匯出走路徑：`export_vn_card(world_id, path, cover)`（`commands/vn_card.rs:21-37`），封面缺時前端讓玩家挑（`features/vn/card/vn-card.ts:91-96`）。
- **大廳沒有任何拖放**：前端零 `onDragDropEvent`；Tauri 2 預設攔下 OS 拖放（`tauri.conf.json` 未設 `dragDropEnabled`），HTML5 drop 收不到檔案。
- 大廳 VN 入口卡 `Lobby.tsx:100` → VN 精靈 ①（一句話開桌／我有角色卡）。

### 1.4 會生成或寫分支的入口（重播桌要處理）

- 續寫守門 `vn::refuse_ended`／`refuse_if_ended`（`vn.rs:633-645`）五處：`chat_turn/gm.rs:121`、`chat_turn/character.rs:59`、`commands/scene.rs:57`（玩家句）、`commands/vn.rs:195`（換一組）、`scene_budget/summarize.rs:335`（換幕摘要）。
- 進桌自動觸發生成：`VnPlayer.tsx:138` `vn_backgrounds_fill`、`useSprites.ts:40` `vn_sprites_fill`（→ `vn_sprites/queue.rs:200, 222, 235`、`vn_backgrounds.rs:364`）。
- 會建新幕的：`vn_fork_choice`、`vn_replay_chapter`（`replay.rs:35`，截幕首分岔）、`vn_save_load` 非尾幕整幕分岔（`vn_branches.rs:184-215`）、`advance_scene`（章結束）、`vn_new_playthrough`、聊天視圖整幕分岔。
- `switch_scene_tx` 只收尾幕（`branches.rs:163-170`）。
- `WorldState` 沒有 `replay` 欄（`state.rs:431-482`）；`WorldMeta{id, name, read_only, needs_repair, table_kind}`（`world.rs:15-21`）。

## 2 定案設計

### 2.1 格式：`.ttvn`（zip，`version: 1`）

總綱 §14「來源就是包 1–7 落檔的事件流與 `vn/` 資料夾，匯出只做整理與複製，不另造中間格式」：`script.json` ＝**幕檔原樣**（幕＋標籤＋事件劇本原文），匯入端逐幕寫回，`origin`／`choice_branches`／`tip_of`／`endings_overview`／`read_scene` 全部原樣可用、零反向轉換（不做包 6 §5 說的「以源頭事件為節點」的樹，異議 1）。

```
manifest.json
  { format: "tt-vn-work", version: 1, app_version, title, lang,
    chapters: N（包內到過的最大章號）, endings: N（包內達成的結局數）,
    settings: bool, vars_mode: bool,
    rating?: { level, warnings },            // VN 卡桌才有，匯入時顯示
    assets: ["assets/…png", …] }            // 包內所有圖的完整清單＝參照完整性的依據
script.json
  { current: 幕號,                          // 匯出者目前所在幕（可續寫桌用；重播桌一律從幕 0）
    chapters: [章名…],                      // 第 1 章起；沒有聖經＝空
    endings: [{ id, title }],               // 只列包內達成的（總綱「結局一覽只列包裡的」）
    locations: [{ key, name, times[] }],    // 卡內地點（沒有＝空）：讓 [bg] 對回同一個 slug
    scenes: [{
      scene: 幕號（0..n−1，重新緊排、保持原順序）,
      title?: 幕名,
      label?: { base, version, parent?, forked, source?, keep? },   // 第 0 幕沒有；base／version＝顯示用編號，照原值；cut 不帶（匯入時重算）
      events: [{                            // 分岔幕且來源前段未改寫：只放 keep 之後自己的事件
        kind: "narration"|"dialogue"|"player"|"system",
        speaker?: 角色id,                   // dialogue 必填
        speaker_name: 字串,
        text: 字串,                          // 劇本格式原文；system 一律空字串
        recap?: true,                        // 換幕摘要（marker = scene_summary）
        opening?: true,
        stage: { place?, time?, chapter?, ended?, finale? }   // 最小快照
      }]
    }] }
characters.json
  { player?: 角色id,                         // 玩家卡／卡定主角（借 player_card_id）
    characters: [{ id(ULID), name, color, emoji, archived,
                   height_cm?, expressions_extra: [鍵], variants: [鍵],
                   image?: true, avatar?: true }] }   // 卡圖、頭像有沒有進包
assets/sprites/<角色id>/sprite.json、body/*.png、face/*.png、face-<變體>/*.png
assets/backgrounds/<slug>-<時段>.png
assets/characters/<角色id>.png、<角色id>.avatar.png
cover.png（選配）
settings/（只在 settings: true）
  vn.json         { bible, card }                         // 聖經與 VN 卡原件（frozen、playthrough、spoiler_lock 不帶）
  world.md
  characters.json { <id>: { public_md, private_md } }
  worldbook.json  ST character_book 形狀（重用包 9 匯出的條目轉換，帶可見性）
  history/<幕>.json  [TableState | null]，與該幕**完整**事件逐則對齊（含複製來的前段）
  summaries.json  { <幕>: 換幕摘要 }                       // catchup_summaries
```

- **事件位置要對齊**：`keep`／`choice_state` 都用逐字稿索引，所以 system 與 `gm_only` 事件**保留位置、清空內容**（`kind: "system"`、`text: ""`，`read_scene` 本來就略過 system，:499），不能整則刪掉。
- **演出包剝掉的**（隱私清單）：`raw`（含狀態區塊）、完整 `state`（只留 `stage` 五欄）、`ts`、`id`、`turn_key`、`action_id`、`message_vars`／`vars_rev`／`vars_epoch`、`truncated`、system 與 `gm_only` 事件的文字、`marker`（只留 `recap`）、`candidates/`、`backgrounds.json`、角色私設與公開設定、世界書、聖經（只留章名與已達成結局名）、`vn-progress.json`、模型指定。保留：`speaker_name`、玩家句（選項文字；聊天視圖自由輸入過的也會在，總綱 §13、§14 匯出前提醒）、換幕摘要旁白（log 用，章字卡後不跑摘要）。
- **角色 id 沿用 ULID**（包 5 新周目同做法）：立繪資料夾、`speaker`、`player`、聖經 `holder`、世界書可見性全以 id 為鍵，匯入時 `validate_id` 驗格式即可，不另寫一套重對應（包 9 是因為卡內 id 是任意字串才重對應）。代價：ULID 帶建立時間（毫秒時戳），可接受。
- **分岔幕去重**：分岔幕 F（`forked`、有 `source` S 與 `keep` K）若 S 的索引 K−1 那則 id 仍是 `cut`（＝`origin` 的同一條判斷，`branches.rs:84-110`），只放 F 的 `events[K..]`，匯入時複製 S 的前 K 則補回；判斷不成立（來源被聊天視圖收回改寫過）就整幕全放、`keep` 不帶（匯入後這條在選項點不列為別條分支，同包 6 §7 限制）。存檔整幕分岔、從某一章重玩（`keep = 1`）都自然變小。
- **幕號重排**：只收已發布、逐字稿非空的幕（`0..=scene_high`，水位＋1 的殘留不算），依原幕號遞增緊排成 `0..n−1`，`parent`／`source`／`current` 一起換；原幕號遞增，所以重排後 `parent < scene`、`source < scene` 恆成立（匯入驗它）。

### 2.2 匯出（8a；附設定 8c）

- **條件**：VN 桌、不是重播桌、`refuse_during_turn` 不在回合中、至少一則事件快照 `tree.ended` 有值（任一分支到結局）。入口：完結畫面「匯出成 VN 作品」、工具列 ⋯「匯出成 VN 作品」（沒到結局時灰掉並 title 說明）。
- **匯出前對話框**（新元件 `features/vn/work/ExportWorkDialog.tsx`，用既有 `Dialog`）：
  - 提醒（總綱 §14）：「玩家名與選項文字會原樣進包，請確認沒有個資。拿到檔案的人不需要 AI 就能看完全部已玩過的分支。」
  - 勾選「附設定（聖經、私設、世界書與續寫所需的歷史；拿到的人看得到所有秘密，也能接著寫）」，預設不勾〔拍板 12〕。變數模式桌（`message_vars` 控制檔有作用中的幕）此項停用並註明「這桌用卡片變數模式，作品包只能帶演出」（異議 7）。
  - 封面：有 `vn/cover.png` 用它；沒有就照 `vn-card.ts:91` 讓玩家挑一張 PNG（可取消，取消＝第一張背景，再沒有就不帶）。
  - 另存新檔對話框，副檔名 `.ttvn`，預設檔名＝桌名。
- **後端** `export_vn_work(world_id, path, settings, cover)`（`spawn_blocking`）：
  1. 寫入許可（共享）＋一次 `with_commit` 內讀 `state`、所有幕逐字稿、`vn.json`（快照一致；回合在途拒）。之後在鎖外複製圖（圖只會被生成佇列原子替換，讀到舊或新都是完整檔）。
  2. 組 `script.json`（§2.1 規則）、`characters.json`：清單＝`list_characters` 全部（含封存）；事件引用了不存在的角色 id（刪過的角色）就補一筆 stub（`name`＝事件的 `speaker_name`、`archived: true`、無圖）。
  3. 圖：每位角色的 `sprite.json`＋`body/`、`face/`、`face-*/` 下的 `*.png`（`candidates/` 不帶）；`vn/backgrounds/*.png`；角色卡圖與頭像；封面。全部列進 `manifest.assets`。**每張出包前一律過 §2.5 #8 同一支 `sanitize_png`**（`classify_png_image`＋chunk 白名單重組，匯出匯入共用）：庫內的圖不保證乾淨——`strict_stored_png` 合格時原樣保留（`png_clean.rs:185`），角色卡圖、VN 卡封面可能還帶 `chara`／`ccv3` 文字、`zTXt`／`iTXt`、`eXIf`。驗不過的庫內圖不進包、不列 `manifest.assets`，匯出完成時以 notice 告知（Sol 方案審第 1 輪必改 1）。
  4. 附設定：§2.1 `settings/`；世界書呼叫包 9 匯出的條目轉換（`import/vn_card/export.rs:193` `book_entries` 升 `pub(crate)`）；`history/<幕>.json` 取每幕完整逐字稿的 `state`。
  5. 寫到 `<path>.part` 再 rename（原子，失敗不留半個檔）；PNG 用 `Stored`、JSON 用 `Deflated`。
- 不帶：§1.1 最後一條的個人與工具資料。

### 2.3 匯入入口（8a；二選一 8c）

- **入口**：VN 精靈 ① 加第三個選項「匯入 VN 作品」（與「一句話開桌」「我有角色卡」並列）→ `open({ filters: [{ name, extensions: ["ttvn"] }] })` 拿路徑。拖進大廳見 §2.9／待裁決 3。
- `probe_vn_work(path)` → `{ title, lang, chapters, endings, settings, vars_mode, rating? }`：只開 zip、驗 manifest（§2.5 前四條＋manifest 欄位），不讀圖；失敗回錯誤碼。
- 前端：有 `rating` 照包 9 分級確認框顯示；`settings: true` 且非 `vars_mode` 時跳二選一「當作品玩（不需要 AI）」「當可續寫的桌（需要 AI，可從任一章或選項點接著寫）」（8c）；否則直接當作品玩。
- `import_vn_work(path, mode: "replay"|"continue", spoiler_lock)` → `{ world_id, notices }`，建好進桌（標題畫面）。
- 失敗訊息照錯誤碼在地化：格式版本不符、不是作品包、包太大、包內路徑不安全、包含連結、缺圖、圖壞、劇本對不上、需要更新版 app（`version` > 1）。

### 2.4 匯入建桌（8a；可續寫 8c）

照包 9：`create_world_exclusive(root, title, Vn)` 拿新桌獨占→ `build` 一次建完→任一步失敗 `discard_new_world` 刪整桌；`hook(步驟名)` 測試掛點同 `materialize.rs`。新模組 `src-tauri/src/vn_work/`（§2.10 落點）。

1. **兩段式驗證**：(a) 結構層——manifest、`script.json`、`characters.json`、`settings/` 的 JSON、幕樹與角色參照、`manifest.assets` 每項都在包內——在記憶體驗完才 `create_world_exclusive`，結構壞＝不建桌；(b) 圖層——建桌後**逐張**「讀（`take` 上限）→ `classify_png_image` → 剝 chunk → 寫 → 丟」，記憶體峰值≈單張上限；任一張壞就 `discard_new_world`（短暫建桌再刪，同失敗不留半桌）。不把整包讀進記憶體。
2. **角色**：`write_character`（`public_md`／`private_md` 演出包為空、可續寫取 `settings/characters.json`）；卡圖與頭像照 `save_character_png` 同一套嚴驗後寫；`player` 寫 `player_card_id`。
3. **立繪與背景**：路徑由 `SpriteDir::of(root, world, 角色id)`＋驗過的鍵組出（`body/base.png`、`body/<valid_variant_key>.png`、`face/<valid_art_key>.png`、`face-<變體鍵>/<表情鍵>.png`）；背景 `<valid_slug>-<valid_art_key>.png`。`sprite.json` 經 `SpriteMeta` serde 解析後重新序列化寫入（不原樣抄位元組）。
4. **幕**：依幕號遞增逐幕寫（新函式 `write_work_scene`，擴 `import.rs` 的同一套：整幕一次原子寫、重發 id）：
   - 分岔幕帶 `keep`：先複製已寫好的來源幕前 `keep` 則（重發 id），再接自己的事件；`cut`＝來源幕剛寫回的第 `keep−1` 則的**新** id（不信包內任何 id）。
   - 每則快照：重播＝`stage` 轉成 `TableState{table:{place,time}, tree:{chapter, ended, finale}}`；可續寫＝`history/<幕>.json` 的對應項（長度須等於該幕完整事件數）。
   - 標籤：每個 ≥1 的幕都寫明確標籤（沒標籤的幕預設 `parent = scene − 1`，`next_child` 也只認有標籤的幕）：`base`／`version` 照包原值（只是顯示用編號，不參與導覽；`base` 夾到幕數內、`version` 夾 1–99）、`forked`／`parent`／`source`／`keep`、`cut` 照上一條回填。
5. **狀態**：`current_scene`（重播＝0；可續寫＝`script.current`）、`scene_labels`、`scene_titles`、`catchup_summaries`（可續寫）、`table_kind: vn`、`replay`（重播＝true）、`state`＝目前幕最後一則快照。
6. **`vn.json`**：
   - 重播：`{ spoiler_lock: false, bible: 演出用最小聖經, card: 演出用最小卡原件 }`。最小聖經＝`chapters` 只填 `title`、`endings` 只填包內達成的 `{id, title, when: []}`、其餘空、`chapter_max`＝`manifest.chapters`；讓章字卡、結局一覽、完結畫面原樣可用。最小卡原件＝`cast[id].art.{expressions, variants}` 只有鍵、`height_cm`、`locations` 只有 `key/name/times`；生圖用的 prompt、外觀、畫風一律空——它只服務解析名單與縮放。
   - 可續寫：`settings/vn.json` 的 `bible` 過 `vn::parse_bible`＋`normalize`（包 4 同一套夾斷）、`card` 過包 9b 的美術欄位驗證（`import/vn_card/art.rs`）；`spoiler_lock` 照匯入時的選擇（預設 true）。
7. **機制規則**（可續寫）：不收包內規則，照聖經推導（`Bible::rules()`＋`ensure_engine_rules`，`vn.rs:544, 599`）寫 `state.mechanism`；有聖經時另設 `mechanism.incremental = true`（`vn_bible::install` 有設，`vn_bible.rs:219`，上面兩支不代做；Sol 建議）。
8. **世界書**（可續寫）：`settings/worldbook.json` 走既有 `import_worldbook_as(…, BookOwner::Gm)`（包 9 同一條路，可見性角色 id 對不到的剔除）。
9. **world.md**（可續寫）：夾斷後寫。
10. **封面**：嚴驗後寫 `vn/cover.png`。

### 2.5 信任邊界（每條的實作與上限）

上限數字皆〔模型判斷·未裁決〕，依「一般作品約 20 張背景、3–5 位角色各 5–15 張立繪、約 70–150 MB」留 5 倍餘裕。

| # | 威脅 | 實作 | 上限 |
|---|---|---|---|
| 1 | 包太大 | `metadata(path).len()` 先擋，再開 zip | 壓縮檔 ≤ 768 MiB |
| 2 | 條目過多 | 交給 zip crate 解析前整檔串流掃過每個結尾目錄候選（crate 會往回找遍全檔、容許尾端垃圾、候選失敗會退回前一個），驗原始條目數與中央目錄大小（≤ 條目上限 × 1 KiB），緊貼 zip64 定位標記就拒；本碟號≠目錄碟號的候選 crate 必拒，跳過免得誤擋；`len()` 是同名去重後的數，不能拿來擋 | ≤ 4,096 條 |
| 3 | 路徑逃逸（zip-slip） | **落檔路徑從不取自條目名**（§2.4 第 3 步）；條目名另拒收：含 `..` 段、以 `/` 開頭、含 `\`、含 `:`、含 NUL 或控制字元、長度 > 255 | — |
| 4 | 重複條目名（兩個中央目錄項同名，讀到哪個看實作） | 掃描時名字進 `HashSet`，重複即拒 | — |
| 5 | 連結與特殊項目 | `is_symlink()` 或 `unix_mode()` 的檔案類型不是一般檔（`0o100000`）與目錄 → 拒收整包；目錄條目略過 | — |
| 6 | 加密、少見壓縮法 | `encrypted()` 拒；`compression()` 只收 `Stored`、`Deflated`（其他 feature 本來就沒編進去，再明擋一次） | — |
| 7 | zip bomb（謊報大小、重疊條目、高壓縮比） | 每個要讀的條目 `by_index(i)`→`take(單檔上限 + 1)` 讀進記憶體，**以實讀位元組**判單檔上限並累加總量；不信 `size()`；只讀 manifest 列到的與固定名稱的條目，未知條目不解壓 | 單檔：PNG ≤ 24 MiB、`script.json` ≤ 48 MiB、`settings/history/*.json` 每檔 ≤ 16 MiB、其他 JSON ≤ 4 MiB、`world.md` ≤ 256 KiB；總解壓 ≤ 1.5 GiB |
| 8 | 圖片炸彈與壞圖 | 每張 `classify_png_image(bytes, STORED_IMAGE_LIMITS)`（串流驗結構、CRC、IHDR、zlib 與掃描線，不解碼成點陣）；`Animated`、`TooLarge`、`Invalid` 一律拒收整包（不縮、不剝）；通過後 `copy_chunks_before_iend` 只留 IHDR／PLTE／tRNS／IDAT／IEND 與 sRGB／gAMA／cHRM 再寫（`iCCP` 也剝：ICC 解析是圖片 CVE 常客，立繪背景不靠它）。這一支 `sanitize_png` 匯出端也用（§2.2 第 3 步） | 每張 ≤ 8192 邊、2400 萬像素（既有）；總像素 ≤ 30 億（壓 CPU 時間）；圖 ≤ 2,000 張 |
| 9 | 格式版本 | `format != "tt-vn-work"` 拒；`version != 1` 拒（> 1 提示「需要更新版 app」） | — |
| 10 | schema | serde 型別，未知欄位忽略；字串夾斷：`title` ≤ 120 字單行、`speaker_name` ≤ 64 字單行、事件 `text` ≤ 12,000 字、`place`／`time` ≤ 200 字、章名與結局名 ≤ 120 字、幕名 ≤ 120 字；`color` 須 `#rrggbb`（不合格換預設色）；夾斷記 notice | 幕 ≤ 1,000；角色 ≤ 32；章 ≤ 30；結局 ≤ 30；地點 ≤ 200；事件數見 #11a（展開後計） |
| 11 | 幕樹參照 | 幕號須為 `0..n−1` 連續；第 0 幕無標籤、第 0 則 `opening`；每幕 `parent`／`source` 須 `< scene`；`forked` 才可有 `source`／`keep`；`1 ≤ keep ≤ 來源幕完整長度`；每幕最多一個非分岔子幕（包 6 尾幕不變式）；`current` 須存在 | — |
| 11a | 去重分岔展開放大（小包複製長前段變成大量事件；Sol 方案審第 1 輪必改 3） | 建桌前依幕號順序算每幕**展開後**長度 `full(F) = keep + 自己的事件數`（`keep ≤ full(source)`）與展開後文字位元組 `bytes(F) = 來源前 keep 則的 text 位元組 + 自己的`；單幕、全包上限一律用展開後的值，超限拒收不建桌 | 每幕展開後事件 ≤ 4,000；全包展開後事件 ≤ 60,000；全包展開後逐字稿文字 ≤ 256 MiB（含快照估算：演出包每則最小快照計 256 B；可續寫的 `history` 本來就逐則存在包內，已由 #7 實讀量限住） |
| 12 | 角色參照 | 角色 id 過 `validate_id` 且唯一；`dialogue` 的 `speaker`、`player`、立繪資料夾、`assets/characters/*` 的 id 都要在 `characters.json` | — |
| 13 | 資產參照（「缺圖＝拒收」收窄，異議 4） | `manifest.assets` 每一項必須在包內且過 #8；每個角色有 `sprite.json` 就必須有 `body/base.png`；路徑形狀只收 §2.1 那幾種（鍵過 `valid_variant_key`／`valid_art_key`／`valid_slug`）。劇本引用但沒圖的表情、變體、背景**不是錯**（執行時本來就有退路鏈與字卡） | — |
| 14 | 快照與聖經 | `stage.chapter` 1–30、`stage.ended` 須在 `endings` 內；可續寫：`TableState` serde 解析、每則序列化後 ≤ 256 KiB；聖經走 `parse_bible`＋`normalize`、卡原件走包 9b 驗證 | — |
| 15 | 文字只當文字 | 劇本文字照現行 React 文字節點渲染（§1.2）；本包不新開 HTML 渲染；`text` 在匯入後照常經 `read_scene` 寬容解析，認不得的退旁白 | — |
| 16 | 失敗不留半桌 | §2.4 第 1 步先驗完；建桌後任一步失敗 `discard_new_world` | — |

- **IPC**：`probe_vn_work`／`import_vn_work` 收路徑（前端 `open()` 或拖放事件給的），Rust 自己讀；路徑來自玩家操作的系統對話框，不另驗目錄。

### 2.6 重播模式（後端 8a、播放器 8b）

- **狀態**：`WorldState.replay: bool`（`#[serde(default, skip_serializing_if = "is_false")]`），只在作品包匯入時寫 true、永不清；`WorldMeta` 帶出（大廳桌卡徽章「作品」）。`TableKind` 不加第三種（重播桌仍是 VN 桌，所有讀取路徑照舊）。
- **後端守門（8a，建重播桌的同一包就要有，否則匯進來的桌能被生成）**：
  - 兩層，見 §2.6.1 入口清單（Sol 方案審第 1 輪必改 2）：
    - **命令層**：清單上每個會呼叫模型或生圖的命令，在呼叫模型**之前**讀狀態判 `replay` → `UiMsg::VnReplayReadOnly`（自動觸發的兩個佇列命令靜默 no-op）。`vn::refuse_ended` 加 `replay` 判斷，順帶涵蓋既有五處。
    - **資料層兜底**：逐字稿與幕的寫入咽喉——`transcript.rs` 的 `write_line_tx`（所有追加）與 `edit_scene`（收回、改寫、刪則、還原），`lifecycle.rs` 的 `fork_scene_tx`、`begin_next_scene_tx`、`revert_scene_tx`——在交易內讀狀態，`replay` 就拒、零寫入；另有兩支不經這五支的寫入各自加判：`discard_unanswered_player`（直接截檔；狀態讀不到照舊收）與 `replace_scene_summary`。作品包建桌是先寫幕檔（`write_work_scene` 直接原子寫，不經這些函式）再寫 `replay: true`，不受影響。`data/scene` 其餘公開寫入函式都經這五支之一，測試逐一呼叫。
#### 2.6.1 重播桌入口清單（2026-11-02 掃 `commands/` 全部 `#[tauri::command]`）

| 類別 | 命令（檔:行） | 重播桌處理 |
|---|---|---|
| 文字模型 | `gm_narrate`、`chat_with_character`（經 `chat_turn/gm.rs:121`、`character.rs:59`） | `refuse_ended` 已在呼叫模型前 → 加 `replay` |
| 文字模型 | `vn_reroll_choices`（`commands/vn.rs:195`） | 同上 |
| 文字模型＋建幕 | `advance_scene`（`summarize.rs:335` `advance_with`） | 同上；資料層 `begin_next_scene_tx` 兜底 |
| 文字模型 | `regenerate_scene_summary`（`commands/scene.rs:556` → `summarize.rs:352` `regenerate_locked`，**未走** `refuse_ended`） | 拿許可後、呼叫模型前拒 |
| 文字模型 | `vn_bible_generate`、`vn_secret_reroll`（`commands/vn_bible.rs:100`） | 呼叫前拒 |
| 文字模型 | `translate_opening`（`commands/scene.rs:159`）、`post_opening`（:96，寫開場） | 拒（重播桌沒有開場白面板，兜底） |
| 文字模型 | `refactor_*`（survey／expand／recommend／apply 等） | 拒：模型入口一律先 `assemble_card_context`，在那裡判重播；`refactor_apply` 走已遊玩擋下 |
| 生圖 | `vn_sprite_generate`（`commands/vn_sprites.rs:184`）、`vn_sprite_reroll`（:255）、`vn_sprite_variant_generate`（:302）、`generate_character_image`（`commands/image.rs:192`） | 呼叫生圖前拒 |
| 生圖（自動） | `vn_sprites_fill`、`vn_backgrounds_fill`（進桌就打，`VnPlayer.tsx:138`、`useSprites.ts:40`）；佇列 `queue.rs:200, 222, 235`、`vn_backgrounds.rs:364` | 靜默 no-op（命令與佇列入口兩處都判） |
| 生圖 | `vn_background_regenerate` | 拒 |
| 寫逐字稿 | `append_player_event`（`commands/scene.rs:57`）、`append_transcript`（:12）、`pop_transcript_floor`（:278）、`restore_transcript_events`（:290）、`discard_unanswered_player`（:76） | 前一條已有的走 `refuse_ended`；其餘資料層兜底 |
| 建幕／改幕 | `fork_scene`、`revert_scene`、`vn_fork_choice`、`vn_new_playthrough`（建新桌） | 命令層拒；資料層兜底 |
| 建幕（改走 goto） | `vn_replay_chapter`、`vn_save_load` | 重播桌改呼叫 `goto`，不分岔（下一段） |
| 換幕（允許） | `vn_switch_branch`、`vn_replay_goto` | 允許（只換目前幕、不寫逐字稿） |
| 匯出 | `export_vn_card`、`export_vn_work` | 拒 |
| 允許 | `vn_progress_write`、`vn_save_create`／`delete`、所有 `read_*`／`vn_*_read`／`vn_endings`／`vn_choice_branches`／`vn_cover`、`rename_world`、`delete_world` | 照常 |
| 不處理（非模型、非逐字稿，前端在重播桌不給入口） | 角色／世界書／狀態編輯類（`write_character`、`upsert_worldbook_entry`、`set_table_state`、`card_vars_write` 等）、`import_character`／`import_worldbook`（匯進目前桌） | 列已知限制：從別處進得去就改得到設定，但不會呼叫模型也不會動逐字稿 |

- 測試：清單上「拒」的每個命令背後函式各一例，用假 CLI／假呼叫者斷言**模型與生圖零呼叫**、`scene_high` 與每幕逐字稿位元組不變；資料層五支各一例。

- **導覽改定義（8a 後端、8b 接線）**——重播桌不生成新內容，所有「去別處」都只換目前幕：
  - 新命令 `vn_replay_goto(world_id, scene) -> u64`：只收重播桌；`switch_scene_tx` 加 `require_tip: bool`（重播 false），其餘同包 6 §2.4（不碰逐字稿，狀態樹＝目標幕最後一則快照）。完結畫面由游標決定（`VnPlayer.tsx:350`：`play.end === "ended"` 且停在結局格），不看狀態樹的 `ended`，所以跳到會完結的分支幕首不會直接出「完」。重播桌沒有世界書計時與各幕隱藏快照檔，`settle_pending`／`restore_scene_visibility` 要是 no-op（施工時補測）。
  - 從某一章重玩：`replay_chapter` 在重播桌改走 `chapter_head`（`replay.rs:13`）＋`goto`，不分岔；進桌提示「幕首」。
  - 讀檔：`load_save` 在重播桌一律 `goto(save.scene)`，不分岔、不要求尾幕。
  - 標題「開始」：`goto(0)`＋進桌提示「幕首」（不分岔、不問確認）。
  - 章末：`[chapter]` 格播完、章字卡照播，**不**呼叫 `advance_scene`，改 `goto(換幕子幕)`＋「幕首」；沒有子幕＝「此路作者未完成」。
- **選項點（8b）**：重播桌的選項面板一律走「過去的選項面板」那一套（`PastChoicePanel`＋`vn_choice_branches`），`ChoiceBranch` 多回 `scene`（玩家在哪一幕選了這句）與 `answer_event_id`：
  - 本幕玩家句那個選項＝往前讀；其他有分支的＝`goto(那條的 scene)`＋進桌提示「指定格」停在那則玩家句；沒玩過的灰掉、標「作者未走過這條路」、不可按。
  - 不出「換一組」；不顯示「目前路線」字樣（讀者沒有自己的路線，只有作者走過的）。
  - 未選的選項點（作者停在選項就沒再玩）：照同一份分支清單，有別條的可跳，沒有就全灰並顯示「此路作者未完成」。
- **「此路作者未完成」（8b）**：游標到幕末，最後一格不是結局、沒有可按的選項、也沒有換幕子幕 → 對話框位置顯示「此路作者未完成」＋「回上一個選項」（本幕內最近的選項格；本幕沒有就退 `story_previous` 那幕最後一個選項格——跨幕走 `goto`，回溯本身仍不跨幕）＋「回標題」。
- **介面收掉（8b）**：工具列不出「停止」「下一章」「聊天視圖」；⋯ 只留「結局一覽」「章節選擇」（＝從某一章重玩改名，重播桌專用字串）；完結畫面不出「開新周目」「匯出成 VN 作品」；標題「設定」照舊；`enterTable` 對重播桌不給聊天視圖切換。auto／skip（只跳已讀）／log／存檔照常（總綱 §14）。
- **進桌提示**：`useVnPlayback` 的 `entry` 加 `{ kind: "start" }`（幕首第一個可停格）。

### 2.7 可續寫的桌（8c）

- 一般 VN 桌（`replay: false`），從 §2.4 還原：所有幕與標籤、逐則完整快照、換幕摘要、幕名、聖經、卡原件、角色公開與私設、世界書、world.md、立繪、背景、封面；機制規則照聖經推導；`current_scene`＝作者匯出時所在幕。
- 進桌出標題；「繼續」落在 `current` 幕（沒有閱讀位置＝包 2 預設位置）；「開始」＝從第 1 章重玩（分岔），選項點分岔、讀檔、章節選擇都走包 5／6 既有流程，從任一章或選項點都接得上（總綱 §14、Sol 總綱審第 1 輪必改 3）。
- 不還原（列已知限制）：`world-info/` 計時（定時條目從頭計）、`scene_card_visibility`（各幕自動隱藏快照）、`lanes.json`（第一拍重開續聊線）、模型指定（用匯入者自己的設定）、聖經以外的機制規則（卡片自帶的 MVU／重構規則）。
- 變數模式桌不能附設定（§2.2、異議 7）。

### 2.8 前端接線

- 8a：`features/vn/work/`：`ExportWorkDialog.tsx`（勾選、提醒、變數模式停用）、`useVnWork.ts`（另存、選封面、`export_vn_work`；`open`、`probe_vn_work`、`import_vn_work`、錯誤碼對照）；VN 精靈 ① 加選項（`VnWizard`）；工具列 ⋯ 與完結畫面加「匯出成 VN 作品」；桌卡「作品」徽章；`WorldMeta.replay` 型別；i18n `src/i18n/features/vn-mode.ts` 補字串。
- 8b：`VnPlayer` 收 `replay`；`VnToolbar`、`EndingScreen`、`TitleScreen` 依 `replay` 收鈕；`useChapterEnd` 重播分支；`useBranchOps` 加 `goto`；`PastChoicePanel` 加重播變體（灰掉、不標目前路線）；新元件 `UnfinishedPath.tsx`；`useVnPlayback` `entry: start`。
- 8c：二選一對話框、可續寫的錯誤碼。

### 2.9 拖進大廳（待裁決 3，預設做，放 8a）

總綱 §14 寫了「把 `.ttvn` 拖進大廳」，但大廳現在零拖放（§1.3）。做法：大廳掛 `getCurrentWebview().onDragDropEvent`，`drop` 且唯一一個路徑以 `.ttvn` 結尾 → 走 §2.3 同一流程；其他檔案不理（不做卡片拖放，那是另一件事）。約 35 行。

### 2.10 落點（照 [STRUCTURE](../../docs/STRUCTURE.md)）

- 後端新資料夾 `src-tauri/src/vn_work/`：`format.rs`（型別、常數上限）、`archive.rs`（zip 開包與 §2.5 #1–#8）、`export.rs`、`validate.rs`（#9–#14）、`materialize.rs`（建桌）、`mod.rs`；命令 `commands/vn_work.rs`。`data/scene/import.rs` 加 `write_work_scene`；`data/scene/branches.rs` `switch_scene_tx` 加 `require_tip`；`vn_branches.rs` `ChoiceBranch` 加欄、`load_save` 重播分支；`data/state.rs` `replay`；`data/world.rs` `WorldMeta.replay`。
- 前端 `features/vn/work/`；`VnPlayer.tsx`（539 行，已超 500）只加 prop 與接線，重播的面板與分支邏輯放新檔；`App.tsx`（1186）只加 prop。

## 3 測試

- **單元（後端）**：
  - 開包（每條一例，全部拒收且不建桌）：條目名 `../x.png`、`/abs.png`、`a\\b.png`、`C:x.png`；重複條目名；symlink 條目（`unix_mode 0o120777`）；加密條目；bzip2 壓縮法；條目數 4,097；**標頭謊報大小**（中央目錄 `size` 寫小、實際解壓超過單檔上限）；總解壓超限；壓縮檔超過大小上限（測試換小上限）。未知條目（`extra/readme.txt`）忽略、不落檔。
  - 圖：CRC 壞、IHDR 尺寸超限、APNG、尾巴多資料、manifest 列了但包內沒有 → 拒收；ancillary `tEXt` 被剝掉。
  - schema：`version: 2` 拒並回「需要更新版」；`format` 錯拒；未知欄位忽略；`source >= scene`、`keep` 超過來源長度、兩個換幕子幕、`speaker` 不在角色表、`stage.ended` 不在結局表、角色 id 不是 ULID → 拒；字串超長夾斷並出 notice。
  - **故障注入（失敗不留半桌）**：`hook` 在角色、立繪、背景、第 N 幕、狀態、vn.json 各步回錯 → `worlds/` 下沒有新桌目錄。
  - **往返**：建一張有三條分支（選項點分岔、存檔整幕分岔、從某一章重玩）、兩個結局、一條沒完成的桌 → 匯出 → 匯入重播：幕數、標籤（`cut` 為新 id 且 `origin` 成立）、每幕 `read_scene` 的格與原桌逐格相同（`event_id` 除外）、`choice_branches` 各選項點結果相同、`endings_overview` 只列達成的、背景 slug 對得上（含卡內地點）。
  - **隱私**：演出包解開後全文搜不到原桌的私設、`raw` 內的狀態區塊、system 事件文字、聖經秘密、`ts`。
  - 重播守門：照 §2.6.1 清單逐一（模型與生圖零呼叫、`scene_high` 與逐字稿位元組不變）；資料層五支兜底各一例；`vn_sprites_fill`／`vn_backgrounds_fill` 不排任何工作；`vn_replay_goto` 收非尾幕、拒非重播桌；讀檔與從某一章重玩在重播桌不建新幕（`scene_high` 不變）。
  - 匯出隱私（PNG）：庫內放一張帶 `chara` tEXt、`zTXt`、`iTXt`、`eXIf` 的角色卡圖與封面 → 匯出包解開逐 chunk 檢查只剩白名單；壞圖不進包、出 notice。
  - 展開放大：每幕實存事件都未超限、但大量分岔幕共用長前段使展開後單幕或全包事件數、文字量超限 → 拒收、不建桌。
  - 匯出：沒有結局拒；回合在途拒；重播桌拒；寫入失敗不留 `.ttvn`（只剩或連 `.part` 都清掉）；system 事件保留位置、清空文字；分岔幕只放後段、來源被改寫過的整幕放。
  - 8c：可續寫往返：快照逐則相同、摘要、聖經、世界書可見性、機制規則照聖經且 `incremental = true`；匯入後跑一拍假 GM 回覆：旗標更新被接受、秘密揭露寫進 `revealed`；`history` 長度不符拒；變數模式桌附設定被拒。
- **前端**：重播選項面板（有分支可按、沒玩過灰掉＋字樣、不出換一組）；未完成面板與「回上一個選項」；工具列與完結畫面在重播桌收鈕；章末走 `goto` 不打 `advance_scene`；匯出對話框（變數模式停用勾選、提醒文字）；精靈 ① 第三個選項。
- **測試通道**（release 包，主線親看截圖）：
  1. 用包 5／6 實測留下的桌（有結局、有分支）匯出演出包；`unzip -l` 看內容。
  2. **全新 `--root`、零金鑰零 CLI 設定**匯入（總綱 §14）：標題→開始→讀到選項點→選已玩過的、看灰掉的→讀到結局→完結畫面；走到未完成分支看「此路作者未完成」→回上一個選項；章末換章；存檔讀檔；結局一覽只列包內的。全程網路請求為零。
  3. 拖 `.ttvn` 進大廳（若做）。
  4. 8c：附設定匯出 → 匯入選「可續寫」→ 從第 2 章選項點選沒玩過的 → GM（luna）接著寫；回合數記錄。
  5. 竄改包（改一個條目名成 `../x`、刪一張列表內的圖）→ 匯入顯示錯誤、大廳沒有新桌。

**8a 實測結果**（2026-11-02，測試通道 release 包，全新測試根 `real8a/root`；截圖在子代理 scratchpad `real8a/`）：

- 測試桌：SunSmell VN 卡匯入，GM（claude-sonnet-5-5）真跑兩拍與一次換章、一次從第 1 章重玩選另一個選項；結局 `[ending: warm]` 為**注入**。立繪一組、角色圖、封面與一張背景**注入**帶 tEXt／zTXt／iTXt／eXIf 的 PNG。
- 匯出過：完結畫面「匯出成 VN 作品」→提醒→另存，16.8 MB；條目只有 manifest／script／characters／cover／背景 3 張／立繪 sprite.json＋定稿＋臉層／角色圖，候選不在；8 張 PNG 只剩 IHDR／IDAT／IEND；script 無私設、無 raw、system 事件清空、時間戳不在。
- 匯入過：精靈 ①「匯入 VN 作品」→確認框（標題、章數、結局數、分級）→進桌出標題；分岔幕 keep／cut 重建後過去的選項面板列出「已玩過 · 已完結」，切過去播到結局「已達成結局 1／1」，背景照常顯示。
- 守門過：重播桌的下一章、章末換章、聊天視圖 GM 推進與收回都顯示「這是匯入的 VN 作品…」；直接呼叫換一組、聖經生成、重寫提要、立繪定稿與變體生成、重生成背景、角色生圖、翻譯開場、新周目、分岔一律 `vn_replay_read_only`；`vn_sprites_fill` 不排工作；AI log 對重播桌零派送；三幕逐字稿檔不變。
- 壞包過：路徑跳出（壓縮檔路徑不安全）、條目 4,100（太大）、定稿 CRC 壞（內容損壞，附路徑）三個都拒收，`worlds/` 桌數不變。
- 拖進大廳：測試通道無法模擬系統拖放，未實測（單元測試只驗到無視窗環境不報錯）。
- 修正：(1) 匯出漏帶玩家卡／卡定主角（名單不列它）：補進 `characters.json` 並寫 `player`，往返測試加斷言；(2) 重播桌桌卡「VN」「作品」兩個徽章擠在一起截字：重播桌只標「作品」；(3) 匯入被拒時精靈還開著、蓋住大廳的錯誤訊息：拒收時收起精靈。
- 8a 範圍內的預期現象（交給 8b）：重播桌走到章末仍跑章結束流程而被擋；完結畫面與工具列的生成類鈕仍在（按了被擋）；聊天視圖仍可切換，送出被擋時出「這一輪沒能完成」。

## 4 異議（主線全採納，2026-11-02）

1. **`script.json` 用幕檔原樣，不做「以源頭事件為節點」的樹**（包 6 §5 的設想）：樹形格式對第三方閱讀友善，但匯入要再還原成幕與標籤，多一整套轉換與測試；幕檔原樣＋分岔幕只放後段，去重效果相同（共用前段只存一次），`origin`／`choice_branches` 等包 6 的函式匯入後原樣成立。總綱 §14「不另造中間格式」也支持這個。
2. **事件存劇本原文、不存解析好的格**：格是讀時由唯一解析器產生（包 1 定案），存原文才不會出現兩套格式；代價是之後解析器若改行為，舊包的顯示跟著變（寬容解析、只會更好），`version` 只在格式不相容時升。
3. **角色 id 沿用 ULID 不重對應**：比包 9 少一套 remap；`validate_id` 已擋路徑逃逸。
4. **「缺圖＝拒收」收窄成「manifest 列了但沒有＝拒收」**：照字面「劇本引用的圖都要在」會把正常桌拒掉——表情臉層、變體、背景生成失敗本來就靠退路鏈與字卡（包 3、7），作者桌上就缺。
5. **重播桌導覽一律「只換目前幕」，不分岔**：總綱 §14 寫「auto／skip／log／存檔分岔照常」；重播桌沒有新內容可寫，從某一章重玩、讀非尾幕存檔若照一般桌分岔，只會多複製幕。改成 `goto`（`switch_scene_tx` 放寬尾幕限制），存檔照常、分支切換照常。這等於包 6 異議 7 說的「檢視模式」只在重播桌啟用。
6. **圖不重編**：業界最嚴是全部解碼重編（順便洗掉 metadata），但每張要解碼＋編碼（2 MB 的圖約 0.2–0.5 秒，150 張就一分鐘），而既有嚴驗已串流驗完整條 zlib 與掃描線；改成驗過後只留繪圖必要 chunk，效果接近、成本近零。過大的圖拒收不縮（臉框是像素座標）。
7. **變數模式（MVU）桌不能附設定**：可續寫要還原控制檔、逐則表、種子與巨集（`write_imported_scene` 只支援單幕種子），且 MVU 表是任意 JSON 又是一個信任邊界。VN＋MVU 少見，照「罕見邊際狀況輕防」只給演出包。
8. **總綱沒列、本方案補進包的**：角色卡圖與頭像（立繪沒定稿時舞台退路、名牌頭像要用）、卡內地點與額外表情鍵（解析名單要用，否則背景 slug 對不上）、`manifest.assets` 清單、`rating`。都是演出所需、不是設定或劇透。
9. **拖放是新機制**（§2.9）：總綱寫了，但大廳完全沒有拖放基礎；約 35 行，列待裁決，預設做。

## 5 與其他包的關係

- 包 6：已讀鍵是內容雜湊，匯入桌已讀從空開始、之後照常；`vn-progress.json` 不打包。`origin`、`choice_state`、`tip_of`、`scene_high` 原樣用；`switch_scene_tx` 加 `require_tip` 一個參數。
- 包 5：完結畫面與結局一覽在重播桌靠「演出用最小聖經」原樣運作；新周目對重播桌停用。
- 包 9：VN 卡＝玩之前的文字源，作品包＝玩之後的成品，格式不共用（總綱 §15）；只共用世界書條目轉換與分級確認框。

## 6 預算（逐項估核心）

| 項目 | 8a | 8b | 8c |
|---|---|---|---|
| `Cargo.toml` zip 直接依賴、`vn_work/format.rs`（型別、上限常數） | 120 | | 30 |
| `archive.rs`：開包、條目掃描、名稱規則、重複、連結、壓縮法、實讀上限累計 | 120 | | |
| `export.rs`：收幕（已發布、緊排、去重判斷）、事件清理、stage、角色與 stub、資產清單、寫 zip 原子 | 260 | | |
| 匯出附設定：settings/ 各檔、世界書轉換接線、history | | | 90 |
| `validate.rs`：schema 夾斷、幕樹、角色、資產、快照驗證 | 170 | | 50 |
| `materialize.rs`：建桌、角色、立繪與背景落檔（含 PNG 驗與剝 chunk）、封面、`write_work_scene`（前段複製、`cut` 回填、最小快照）、狀態、最小 vn.json、失敗刪桌 | 260 | | |
| 可續寫建桌：完整快照、摘要、聖經與卡原件驗證、機制推導、世界書、world.md | | | 120 |
| 命令 `commands/vn_work.rs`（export、probe、import、replay_goto） | 50 | | 10 |
| `replay` 欄與 `WorldMeta`、§2.6.1 命令層守門、資料層五支兜底、`require_tip`、`replay_chapter`／`load_save` 重播分支、`ChoiceBranch` 加欄 | 110 | | |
| `sanitize_png` 共用（匯出端接上）、展開後上限計算（#11a） | 35 | | |
| 錯誤碼（`UiMsg` 變體與對照，非譯文） | 25 | 5 | 10 |
| 前端匯出對話框、另存、封面、⋯ 與完結畫面入口 | 110 | | 15 |
| 前端精靈 ① 匯入選項、probe、分級框接線、進桌、錯誤 | 80 | | |
| 前端二選一對話框 | | | 40 |
| 拖進大廳（§2.9） | 35 | | |
| 桌卡徽章、型別 | 15 | | |
| 播放器重播：prop 接線、工具列／完結／標題收鈕、`goto` 包裝、`entry: start`、章末走 `goto` | | 100 | |
| 重播選項面板變體（灰掉、跳分支、未選選項點） | | 70 | |
| 「此路作者未完成」＋回上一個選項（含跨幕退一幕） | | 60 | |
| CSS（對話框、徽章、灰掉選項、未完成面板） | 30 | 30 | 10 |
| **合計** | **約 1,420** | **約 265** | **約 375** |

- 合計約 **2,060**（Sol 方案審第 1 輪後；含 CSS、不含測試與 i18n 譯文）；立案 550 的 3.6 倍。照包 5／6 方案→實作 1.3–1.6 倍，預期 2,700–3,300。
- **建議拆三包**：
  - **8a＝匯出＋演出包匯入＋信任邊界＋重播後端守門**（約 1,420）：方案必送審的部分全在這包；守門跟建重播桌同包，避免中途出現「匯進來能被生成」的桌。
  - **8b＝重播播放體驗**（約 265）：純前端為主，依 8a 的 `goto` 與 `ChoiceBranch` 欄。
  - **8c＝附設定與可續寫**（約 375）：又一層信任邊界（聖經、快照、世界書），但都走既有驗證。
  - 本檔一次方案審涵蓋三包，施工後各驗收（同包 6）。
- 熱點：`VnPlayer.tsx` 539（已超 500：重播邏輯放新檔，播放器只接線）、`App.tsx` 1186（只加 prop）、`useImportController.ts` 814（不碰：作品包走自己的 `useVnWork`）、`vn_branches.rs` 226、`branches.rs` 200。

## 7 已知限制（預計）

- 作品包裡玩家名、主角稱呼、選項文字寫死在劇本裡，匯入者不能改名；沒有防拷貝、不加密（總綱 §13）。
- 角色 id 是 ULID，帶建立時間戳。
- 劇本存原文，日後解析器行為改變時舊包顯示跟著變。
- 分岔幕的來源前段被聊天視圖收回改寫過的，整幕放進包、在選項點不列為別條分支（包 6 §7 同一條）。
- 重播桌回溯不跨幕（包 6 異議 6）；「回上一個選項」只退一幕。
- 可續寫桌不還原世界書計時、各幕角色自動隱藏快照、續聊線、模型指定、聖經以外的機制規則。
- 變數模式桌只能匯出演出包。
- 重播桌不能再匯出成作品包或 VN 卡（作者原包就是發行檔）。
- 包內的圖不重編，只剝非必要 chunk。
- 重播桌完結畫面與結局一覽的「已達成 x／y」分母是包內達成的結局數，不是作者原桌的結局總數（最小聖經只帶達成的）。
- 圖壞的包會短暫建桌再刪（結構層壞才完全不建桌）。
- 重播桌的角色、世界書、狀態編輯命令不擋（§2.6.1 最後一列）：前端不給入口，從別處進得去也只改設定，不呼叫模型、不動逐字稿。
- 庫內驗不過的圖匯出時略過（notice 告知），不擋整包匯出。

- 8a 施工：重複條目名 zip crate 解析中央目錄時就只留一筆（後者蓋前者），我們只經這份解析結果驗與讀，不會「驗 A 讀 B」；改為不另拒收（§2.5 #4 原寫拒收），條目數上限照原始條目數算（#2）。
- 8a 施工：匯出前提醒用系統確認框（「附設定」勾選屬 8c，到時才換自訂對話框）；⋯ 與完結畫面的「匯出成 VN 作品」不預先灰掉，沒有結局時按下回「還沒有任何一條路線走到結局」。
- 8a 施工：`refactor_*` 的守門放在 `assemble_card_context`（六支模型命令都先組脈絡才派模型；它不需要原卡，原先「讀不到原卡會失敗」的假設不成立）。
- 8a 施工：精靈 ① 的「匯入 VN 作品」鈕與「我有角色卡」並列在一句話開桌的工具列；匯入建好照從大廳進桌（先到標題畫面），重播播放體驗屬 8b。
- 8a 施工：匯出時單張圖清理後超過 24 MiB 也當驗不過略過（提示告知）；事件引用已刪角色時補一筆封存名牌（無圖）。
- 8b 施工：換幕子幕與劇情前一幕在前端照 App 已有的 `sceneLabels` 算（`replay/replay-nav.ts`，規則同 `branches.rs` 的 `next_child`／`story_previous`），不另加後端命令；跳幕一律走既有 `vn_replay_goto`。
- 8b 施工：進桌提示擴成 `EntryAt`（last／cell／start／after／lastChoice）：玩家句不是可停格，選項跳分支停在那則玩家句**之後**第一個可停格；「回上一個選項」跨幕用 lastChoice。同一幕的 goto（標題開始已在第 0 幕、章頭就是目前幕）直接跳格。
- 8b 施工：重播桌章末字卡照播「第 N 章 完」（停 1 秒再換幕）→「第 N+1 章」，換幕後停 1.2 秒自動收（沒有首拍可等）；沒有章末格的換幕子幕（容量換幕）出「繼續」直接到子幕幕首。
- 8b 施工：重播桌 ⋯ 只留結局一覽與章節選擇（重生成背景、匯出成 VN 卡也收）；完結畫面的重玩鈕與章節面板標題改用重播桌字串（章節選擇／要從哪一章看起）。
- 8b 實測修正：未選選項點全灰（作者停在選項）時讀者會卡住，改為在選項面板裡放「此路作者未完成」＋回上一個選項＋回標題（§2.6 原寫只顯示字樣）；「回上一個選項」只認選過的選項格。已選選項的玩家句是幕尾時，「此路作者未完成」＋回標題放進同一個過去的選項面板（原本兩層面板疊在一起）。匯入作品先刷新桌單再進桌，播放器第一次畫就是重播桌。
- 8b 已知限制：章節選擇只列到目前章（章頭沿目前劇情鏈找）；章字卡蓋著時下一幕第一格已開始打字。
- 8c 施工：不做 `settings/summaries.json`：`catchup_summaries` 是沒有寫入端的舊欄位，換幕摘要本來就是逐字稿事件（`recap`），已隨 `script.json` 帶。
- 8c 施工：卡原件在包內寫成 VN 卡 JSON 的形狀（`slug`、`expressions_extra`，角色鍵是桌內 id），匯回走包 9b `art.rs` 同一套驗證（`origin_from`），原版聖經一併 `normalize`；丟掉的美術欄位以新提示 `dropped` 告知。
- 8c 施工：角色公開／私設各夾 20,000 字、world.md 夾 60,000 字（VN 卡的 1,500／2,000 太窄，作者手改過的設定往返會被截）；實讀上限另加快照檔每檔 16 MiB、world.md 256 KiB。
- 8c 施工：逐幕快照檔不在建桌前整包讀進記憶體：建桌前只驗每幕的檔都在，寫幕時逐檔讀、驗長度＝展開後事件數、每則 ≤ 256 KiB，不合就刪整桌（同圖層「短暫建桌再刪」）。
- 8c 施工：機制規則照聖經推導並設 `incremental = true`、受保護根（不走 `vn_bible::install`：它要求未遊玩，匯入桌已有幕）；凍結聖經不寫，第一拍 `turn_inputs` 自己凍。
- 8c 施工：匯出改自訂對話框（`ExportWorkDialog`：提醒＋附設定勾選，變數模式停用並註明；`BibleView` 加 `vars_mode`）；附設定的包匯入出二選一對話框（`ImportWorkDialog`，帶劇透鎖勾選、預設勾），沒附設定的包照舊系統確認框；`probe` 多回 `vars_mode`。
- 8c 已知限制：可續寫桌的系統事件（登場、私設、狀態更新、點名）沿用演出包規則只留位置、清空內容，GM 續寫看不到這些舊系統訊息（角色設定與世界書另有完整版）。
- 8c 已知限制：匯出時 world.md、角色設定、世界書在短提交鎖外讀（同圖；回合在途已拒，只有匯出當下手動編輯會讀到新或舊版）。

## 7.1 作者裁決

- §2.5 上限數字照表〔作者裁決 2026-11-02〕。
- 變數模式桌不能附設定〔作者裁決 2026-11-02〕。
- 可續寫桌進桌「繼續」落作者匯出時所在幕〔作者裁決 2026-11-02〕（曾改裁第 1 章開頭，施工前查出目前幕必須是分支末端、停舊幕會破幕樹，作者改回）。
- 匯出時沒有封面：讓玩家挑一張、取消就用第一張背景〔作者裁決 2026-11-02〕。
- 拆 8a／8b／8c、拖 `.ttvn` 進大廳、重播桌只換幕不分岔：已照預設施工。

## 8 觀察

（施工與實測發現的範圍外問題寫這裡，一行一條。）

- 地點標籤照抄 GM 寫的時間文字，8a 測試桌 GM（claude-sonnet-5-5）寫成英文 `noon`／`afternoon`，中文桌顯示英文；屬 GM 用語漂移，不在本包。
- 8b 驗收：重播桌章字卡不顯示「整理中……」（主線小修，`ChapterCardView` 收 `replay`）。
- 匯入確認框的分級警示照包 9 原樣列英文鍵（`sexual`、`mpreg` 等），中文介面讀者看到英文；屬包 9 分級顯示，不在本包。
