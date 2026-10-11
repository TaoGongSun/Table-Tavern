# 重構路徑其餘靜默略過：方案

交接檔：[refactor-silent-skips](../handoffs/archive/refactor-silent-skips.md)。

## 1. 原卡檔讀不到被當成「沒有介面」

**根因**：`import/interface.rs` `read_card_interfaces_raw` 對原卡讀檔、PNG 解碼、JSON 解析失敗一律 `continue`。重構入口 `runAiRefactor` 拿 `card_interfaces` 分流，唯一一張匯入卡壞掉時清單為空 → `none` → 免問直跑角色線。

**修法**：單張壞卡仍不擋整份清單，但讀不出時回一筆 `unsupported: "unreadable"`（scripts 空、mvu 假、opening 無、名字照角色 meta，世界層為空）。
- `read_raw_card(path) -> Result<Option<Value>, String>`：IO 錯誤、PNG 結構壞、base64／JSON 解不開回 `Err`；沒有 chara／ccv3 且基本結構完整（IHDR 起頭、chunk 不越界、有 IEND）＝一般圖片，回 `Ok(None)`；只剩 signature、在 chunk 邊界截斷等結構不完整算損毀。`card_io` 新增 `find_png_card` 區分這兩種（`decode_png_character` 行為不變）。
- 角色層：`<id>.png` 不一定是原卡（換角色圖、重構套用立繪都寫這檔），依序看 `.png`、`.import.json`，第一份讀得出卡片資料的就用；都沒有卡片資料＝非匯入卡，略過（同現況）；都沒讀出且至少一份損毀才 `unreadable`。
- 世界層：兩種副檔名各自一份原卡、各自成一筆（只有帶卡片資料的才會存下）；沒有卡片資料略過，損毀回 `unreadable`。
- 其他呼叫端（面板 `cardDrawable`／`card-shell-route.ts`、匯入提示、`chat_assembly.rs`、`policy.rs`、`message_vars/mode.rs`）只用 scripts／mvu 或先濾 `unsupported`，行為等同略過；`world_scan/book.rs` `world_card` 改取第一個非空名，世界層一份壞一份好時不丟名字。
- 前端改四態 `detectRefactorCardState`：**任一筆 `unreadable` 就擋下（最優先）**，其次 supported、unsupported、none——讀不出的卡可能是介面卡或 MVU 卡，判不準就不跑，避免 `mvu_source` 把它靜默算成非 MVU〔主線裁定，依作者裁決 罕見邊際狀況輕防〕。擋下時狀態列報 `refactorCardUnreadable`（帶卡名）／`refactorCardUnreadableUnnamed`（世界層），請玩家開新桌重新匯入再重構（匯入一律產生新 id，同桌重匯無效），不開確認框〔主線裁定，依作者裁決 罕見邊際狀況輕防〕。
- 已重構桌重跑照舊先確認清回、清回後才讀 `card_interfaces` 判擋：清回用收據保存的原檔（`refactor/reset.rs`），目前桌檔讀不出不代表清回後也讀不出。未重構桌照舊直接判擋。
- 契約 `card-view.md` 補讀檔層 `unreadable` 一句。

**測試**：Rust `interface.rs`——非法 JSON、截斷 PNG、讀檔失敗（路徑是資料夾）、世界層損毀都回 `unreadable`；JSON 匯入後換一般 PNG 仍讀到 `.import.json` 的介面；PNG 匯入後換一般圖照舊略過；壞卡與正常卡並存兩筆都在；組合：一般 PNG＋壞 JSON＝unreadable、有效 PNG 原卡＋壞 JSON 用 PNG、壞 PNG＋好 JSON 用 JSON。`card_io`：一般圖回 None，只剩 signature、拿掉 IEND、只剩 IHDR 都回錯。`world_scan` 世界層一份 unreadable 時仍取得原卡名。前端：四態優先序、只有／混合 unreadable 擋下且不初判不盤點、世界層無名文案、重構過桌先確認清回、清回後仍 unreadable 才擋、面板 `cardDrawable`／`candidateCards` 排除 unreadable。

## 2. 重構脈絡略過讀不到的既有角色

**根因**：`refactor_ai/context.rs` `assemble_card_context` 對 `list_characters` 列出的角色讀失敗就略過（只剩檔名與 id 不符、讀取前被動過、IO 錯誤），AI 不知道角色已存在，可能重拆一份。

**修法**：`read_character` 失敗回錯並帶角色名（`伊利亞: <錯誤>`）；7 處呼叫端本來就把錯誤送前端（初判開二選一附原因、其餘報狀態列／失敗清單）。

**測試**：角色檔改名成與 id 不符 → 回錯且以角色名開頭。

## 3. 取消重構失敗無回饋

**根因**：`cancelAiRefactor` 的 `void invoke("refactor_abort")`；只會在 IPC 層失敗，失敗時取消鈕永久 disabled、玩家看不出。

**修法**：`.catch` 時，限發起取消的那一桌、元件還在、同一段進度（`progressGen`，每開一段進度加一）才把 `cancelling` 還原並報狀態列；`cancelRef` 保持 true，不再發下一條。

**測試**：abort 失敗 → 取消鈕可按、狀態列報原因、可再按（abort 呼叫兩次）、後續展開不發；舊桌取消後換桌開新重構並取消，舊 abort 晚到失敗 → 新桌 `cancelling` 與狀態列不變；同桌開新一段後舊 abort 失敗不還原；卸載後失敗不動作。

## 已知限制

- 重構過的桌要先確認清回才知道原卡讀不讀得出；收據原檔也壞時，清回本身會失敗並報錯，桌不受影響〔主線裁定，依作者裁決 罕見邊際狀況輕防〕。

## 範圍外〔模型判斷·未裁決〕

- 換／刪角色圖會蓋掉匯入原卡 PNG：另案 [character-image-overwrites-card](../tasks/character-image-overwrites-card.md)。
- `list_characters` 本身略過解析不了的角色檔是全 app 共用契約，不改。
- `refactorUnsupportedCard` 文案只講雲端載入器、DRM（scrypt）卡也顯示同一句，不改。
- 匯入提示 `tellAboutInterface` 不提示 `unreadable`（剛匯入的檔不會讀不出）。
- 初判階段 abort 失敗時初判照常回來並開二選一（初判不看 `cancelRef`），玩家再選即可，罕見。
